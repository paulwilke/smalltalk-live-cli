//! `stlive save`: write the methods/classes changed in the image back to Tonel files with minimal diffs.
//!
//! The image records every change made through the CLI (`changes.pending`). Here existing files are patched
//! method-by-method (a replaced method changes only its own block; new methods are appended), so
//! `git diff` shows exactly what was changed. Methods whose package differs from their class's package go to
//! `<Class>.extension.st`.

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Plan {
    pub report: Vec<Value>,
    pub marked: Vec<u64>,
}

fn norm(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n").trim_end().to_string()
}

fn is_ident(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// Style of existing files: `true` = old symbol style (`#name : #Foo`), `false` = new string style.
/// Looks at `<dir>/*/*.st` (any package folder next to the target).
fn detect_symbol_style(dir: &Path) -> bool {
    let mut folders: Vec<PathBuf> = vec![dir.to_path_buf()];
    if let Ok(rd) = fs::read_dir(dir) {
        folders.extend(rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
    }
    for folder in folders {
        if let Ok(rd) = fs::read_dir(&folder) {
            for e in rd.flatten() {
                if e.path().to_string_lossy().ends_with(".class.st") {
                    if let Ok(t) = fs::read_to_string(e.path()) {
                        if t.contains("#name : #") { return true; }
                        if t.contains("#name : '") { return false; }
                    }
                }
            }
        }
    }
    false
}

fn sym(s: &str, symbol_style: bool) -> String {
    if symbol_style {
        if is_ident(s) { format!("#{}", s) } else { format!("#'{}'", s) }
    } else {
        format!("'{}'", s)
    }
}

/// Selector of a message pattern: `add: x` -> `add:`, `at: i put: v` -> `at:put:`, `+ other` -> `+`, `one` -> `one`.
fn selector_of_pattern(pattern: &str) -> String {
    let toks: Vec<&str> = pattern.split_whitespace().collect();
    match toks.first() {
        None => String::new(),
        Some(t) if t.ends_with(':') => toks.iter().step_by(2).cloned().collect::<String>(),
        Some(t) => t.to_string(),
    }
}

/// A Tonel method header line: `Class >> pattern [` or `Class >> pattern [ body text` (body on the header line).
struct Header<'a> {
    class_header: &'a str,
    /// The message pattern: the text before the first `[` (a pattern never contains one), trimmed.
    pattern: &'a str,
    /// The text after that `[`, trimmed: the first body text when the body starts on the header line.
    tail: &'a str,
}

fn parse_header(line: &str) -> Option<Header<'_>> {
    let (class_header, rest) = line.trim_end().split_once(" >> ")?;
    // the body opens at the first `[`; the body text after it may contain more of them
    let (pattern, tail) = match rest.find('[') {
        Some(i) => (&rest[..i], &rest[i + 1..]),
        None => (rest, ""),
    };
    Some(Header { class_header, pattern: pattern.trim(), tail: tail.trim() })
}

/// The image's `canonicalSourceOf:` (smalltalk/src/StLive/StLiveCommands.class.st). `raw` is the method source
/// with the pattern first; `pattern` is the message pattern the image builds from the selector and argument names
/// (single spaces). A body written on the pattern's line moves below the pattern, indented by one tab.
fn canonical_source(pattern: &str, raw: &str) -> String {
    let rest = match raw.strip_prefix(pattern) {
        Some(r) => r,
        None => return raw.to_string(),
    };
    if rest.is_empty() || rest.starts_with('\n') {
        return raw.to_string();
    }
    format!("{}\n\t{}", pattern, rest.trim())
}

/// Source of a parsed Tonel method as the image's `canonicalSourceOf:` gives it: the pattern, then the text after
/// the header's `[` and the body lines (the closing `]` line excluded).
fn method_source(h: &Header, body: &[&str]) -> String {
    let pattern = h.pattern.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut raw = h.pattern.to_string();
    if !h.tail.is_empty() {
        raw.push(' ');
        raw.push_str(h.tail);
    }
    for l in body {
        raw.push('\n');
        raw.push_str(l);
    }
    canonical_source(&pattern, &raw)
}

struct Block {
    start: usize,
    end: usize,
    class_header: String, // e.g. "Foo class"
    selector: String,
}

/// Find method blocks: `{ #category ... }` line followed by `Class [class] >> pattern [`.
fn method_blocks(text: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut offset = 0usize;
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut starts: Vec<(usize, usize)> = Vec::new(); // (line index, byte offset)
    for (i, l) in lines.iter().enumerate() {
        if l.starts_with("{ #category") || l.starts_with("{ #") {
            starts.push((i, offset));
        }
        offset += l.len();
    }
    for (n, (li, off)) in starts.iter().enumerate() {
        let end = if n + 1 < starts.len() { starts[n + 1].1 } else { text.len() };
        if let Some(h) = lines.get(li + 1).and_then(|next| parse_header(next)) {
            blocks.push(Block { start: *off, end, class_header: h.class_header.to_string(), selector: selector_of_pattern(h.pattern) });
        }
    }
    blocks
}

fn render_method(class_header: &str, protocol: &str, source: &str, symbol_style: bool) -> String {
    let src = norm(source);
    let mut lines = src.lines();
    let pattern = lines.next().unwrap_or("").trim_end().to_string();
    let rest: Vec<&str> = lines.collect();
    let mut out = format!("{{ #category : {} }}\n{} >> {} [\n", sym(protocol, symbol_style), class_header, pattern);
    for l in rest {
        out.push_str(l);
        out.push('\n');
    }
    out.push_str("]\n");
    out
}

fn str_list(v: &Value, key: &str) -> Vec<String> {
    v[key].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
}

/// `true` when a recorded change concerns a trait (Tonel: `Name.trait.st`). Images older than 0.7 do not
/// send `class_kind`; then an existing `.trait.st` file decides.
fn is_trait(c: &Value, pkg_dir: &Path) -> bool {
    match c["class_kind"].as_str() {
        Some(k) => k == "trait",
        None => pkg_dir.join(format!("{}.trait.st", c["class_name"].as_str().unwrap_or(""))).exists(),
    }
}

/// Tonel header of the class side: `Foo class`, or `TFoo classSide` for a trait.
fn class_side_header(name: &str, is_a_trait: bool) -> String {
    if is_a_trait { format!("{} classSide", name) } else { format!("{} class", name) }
}

/// Class name and side ("instance" / "class") of a Tonel method header such as `Foo class` or `TFoo classSide`.
fn split_header(header: &str) -> (String, &'static str) {
    if let Some(c) = header.strip_suffix(" classSide").or_else(|| header.strip_suffix(" class")) {
        (c.to_string(), "class")
    } else {
        (header.to_string(), "instance")
    }
}

fn var_list(label: &str, items: &[String]) -> String {
    if items.is_empty() { return String::new(); }
    let body = items.iter().map(|v| format!("\t\t'{}'", v)).collect::<Vec<_>>().join(",\n");
    format!("\t#{} : [\n{}\n\t],\n", label, body)
}

/// Tonel comment block of a class (a double quote is written as two); empty for an empty comment.
fn comment_block(comment: &str) -> String {
    let text = norm(comment);
    if text.is_empty() { return String::new(); }
    format!("\"\n{}\n\"\n", text.replace('"', "\"\""))
}

/// The comment a leading comment block holds, as the image would have it (quotes unescaped, normalised).
/// Empty when the text has no comment block.
fn comment_of_block(block: &str) -> String {
    let t = block.trim();
    if t.len() < 2 || !t.starts_with('"') || !t.ends_with('"') { return String::new(); }
    norm(&t[1..t.len() - 1].replace("\"\"", "\"")).trim().to_string()
}

fn render_class(c: &Value, symbol_style: bool, package: &str) -> String {
    let name = c["class_name"].as_str().unwrap_or("");
    let is_a_trait = c["class_kind"].as_str() == Some("trait");
    let mut s = String::new();
    if let Some(comment) = c["comment"].as_str() {
        s.push_str(&comment_block(comment));
    }
    if is_a_trait {
        s.push_str("Trait {\n");
        s.push_str(&format!("\t#name : {},\n", sym(name, symbol_style)));
    } else {
        let sup = c["superclass"].as_str().unwrap_or("Object");
        s.push_str("Class {\n");
        s.push_str(&format!("\t#name : {},\n\t#superclass : {},\n", sym(name, symbol_style), sym(sup, symbol_style)));
    }
    if let Some(t) = c["traits"].as_str().filter(|t| !t.is_empty()) {
        s.push_str(&format!("\t#traits : '{}',\n", t));
    }
    if let Some(t) = c["class_traits"].as_str().filter(|t| !t.is_empty()) {
        s.push_str(&format!("\t#classTraits : '{}',\n", t));
    }
    s.push_str(&var_list("instVars", &str_list(c, "instance_variables")));
    s.push_str(&var_list("classVars", &str_list(c, "class_variables")));
    s.push_str(&var_list("classInstVars", &str_list(c, "class_instance_variables")));
    if symbol_style {
        s.push_str(&format!("\t#category : {}\n}}\n", sym(package, true)));
    } else {
        s.push_str(&format!("\t#package : '{}'\n}}\n", package));
    }
    s
}

/// Variable names listed under `#key : [ ... ]` in a class definition block.
fn listed(block: &str, key: &str) -> Vec<String> {
    let marker = format!("#{} : [", key);
    match block.find(&marker) {
        None => vec![],
        Some(i) => {
            let rest = &block[i + marker.len()..];
            let end = rest.find(']').unwrap_or(rest.len());
            rest[..end].split('\'').enumerate().filter(|(n, _)| n % 2 == 1).map(|(_, v)| v.to_string()).collect()
        }
    }
}

/// Byte offset just after the leading comment block of a Tonel class file (0 when there is none).
/// Inside the comment a double quote is written as two, so the block ends at the first lone quote.
fn comment_end(text: &str) -> usize {
    let body = text.trim_start();
    if !body.starts_with('"') { return 0; }
    let bytes = body.as_bytes();
    let mut i = 1;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            if bytes.get(i + 1) == Some(&b'"') {
                i += 2;
                continue;
            }
            return text.len() - body.len() + i + 1;
        }
        i += 1;
    }
    0
}

/// The `Class {` / `Trait {` definition block of a Tonel file: (start, end) byte offsets.
fn definition_span(text: &str) -> Option<(usize, usize)> {
    let from = comment_end(text);
    let start = from + ["Class {", "Trait {"].iter().filter_map(|m| text[from..].find(m)).min()?;
    let end = start + text[start..].find("\n}")? + 2;
    Some((start, end))
}

/// If the image's class has other instance/class variables than the file, re-render the variable lists of
/// the definition block and keep every other entry (name, superclass, traits, classTraits, category, package)
/// as it is in the file. A changed class comment replaces the leading comment block (added when the file has
/// none, removed when the image's comment is empty). Nothing changed: None. An image that sends no comment
/// (older images, unreadable comment) leaves the file's comment alone.
fn patch_class_definition(text: &str, c: &Value, symbol_style: bool, package: &str) -> Option<String> {
    let (start, end) = definition_span(text)?;
    let block = &text[start..end];
    let wanted = [
        ("instVars", str_list(c, "instance_variables")),
        ("classVars", str_list(c, "class_variables")),
        ("classInstVars", str_list(c, "class_instance_variables")),
    ];
    let vars_changed = wanted.iter().any(|(k, v)| &listed(block, k) != v);
    let prefix = &text[..start];
    let comment_changed = c["comment"].as_str().map_or(false, |image| norm(image).trim() != comment_of_block(prefix));
    if !vars_changed && !comment_changed {
        return None;
    }
    let new_prefix = if comment_changed { comment_block(c["comment"].as_str().unwrap_or("")) } else { prefix.to_string() };
    if !vars_changed {
        return Some(format!("{}{}{}", new_prefix, block, &text[end..]));
    }
    // entries: `#key : value` possibly spanning several lines (arrays)
    let mut entries: Vec<(String, String)> = Vec::new();
    let mut lines = block.lines();
    let opener = lines.next().unwrap_or("Class {").to_string();
    for l in lines {
        if l.trim() == "}" { break; }
        if let Some(key) = l.trim_start().strip_prefix('#').and_then(|r| r.split_whitespace().next()) {
            entries.push((key.to_string(), l.to_string()));
        } else if let Some(last) = entries.last_mut() {
            last.1.push('\n');
            last.1.push_str(l);
        }
    }
    let is_list = |k: &str| wanted.iter().any(|(w, _)| *w == k);
    let mut out: Vec<String> = Vec::new();
    let mut inserted = false;
    let lists = || wanted.iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| var_list(k, v).trim_end_matches('\n').trim_end_matches(',').to_string()).collect::<Vec<_>>();
    for (k, raw) in &entries {
        if is_list(k) { continue; }
        if !inserted && (k == "category" || k == "package") {
            out.extend(lists());
            inserted = true;
        }
        out.push(raw.trim_end().trim_end_matches(',').to_string());
    }
    if !inserted {
        out.extend(lists());
    }
    if !entries.iter().any(|(k, _)| k == "package" || k == "category") {
        out.push(if symbol_style { format!("\t#category : {}", sym(package, true)) } else { format!("\t#package : '{}'", package) });
    }
    let rendered = format!("{}\n{}\n}}", opener, out.join(",\n"));
    Some(format!("{}{}{}", new_prefix, rendered, &text[end..]))
}

#[derive(Default)]
struct FileStats {
    created: bool,
    added: Vec<String>,
    replaced: Vec<String>,
    removed: Vec<String>,
}

pub fn plan(changes: &[Value], package: &str, dir: &Path, dry_run: bool) -> Result<Plan, String> {
    let pkg_dir = dir.join(package);
    let symbol_style = detect_symbol_style(dir);
    let mut marked = Vec::new();

    // Coalesce: last write wins per (class, side, selector); class definitions kept in order.
    let mut class_defs: Vec<&Value> = Vec::new();
    let mut methods: BTreeMap<(String, String, String), &Value> = BTreeMap::new();
    for c in changes {
        marked.push(c["index"].as_u64().unwrap_or(0));
        match c["kind"].as_str().unwrap_or("") {
            "class-create" => {
                class_defs.retain(|x| x["class_name"] != c["class_name"]);
                class_defs.push(c)
            }
            "method-compile" | "method-remove" => {
                let key = (
                    c["class_name"].as_str().unwrap_or("").to_string(),
                    c["side"].as_str().unwrap_or("instance").to_string(),
                    c["selector"].as_str().unwrap_or("").to_string(),
                );
                methods.insert(key, c);
            }
            _ => {}
        }
    }

    let mut stats: BTreeMap<PathBuf, FileStats> = BTreeMap::new();
    let mut contents: BTreeMap<PathBuf, String> = BTreeMap::new();

    let load = |path: &PathBuf, contents: &mut BTreeMap<PathBuf, String>| -> bool {
        if contents.contains_key(path) {
            return contents.get(path).map(|_| true).unwrap();
        }
        match fs::read_to_string(path) {
            Ok(t) => { contents.insert(path.clone(), t); true }
            Err(_) => false,
        }
    };

    for c in &class_defs {
        let name = c["class_name"].as_str().unwrap_or("");
        let path = pkg_dir.join(format!("{}.{}.st", name, if is_trait(c, &pkg_dir) { "trait" } else { "class" }));
        if path.exists() {
            // existing file: only update the variable lists of the class definition if they changed
            let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
            if let Some(updated) = patch_class_definition(&text, c, symbol_style, package) {
                contents.insert(path.clone(), updated);
                stats.entry(path).or_default().replaced.push(format!("{} (class definition)", name));
            }
            continue;
        }
        contents.insert(path.clone(), render_class(c, symbol_style, package));
        stats.entry(path).or_default().created = true;
    }

    for ((class_name, side, selector), c) in &methods {
        let extension = c["class_package"].as_str() != Some(package);
        let is_a_trait = is_trait(c, &pkg_dir);
        let kind = if extension { "extension" } else if is_a_trait { "trait" } else { "class" };
        let file = pkg_dir.join(format!("{}.{}.st", class_name, kind));
        let header = if side == "class" { class_side_header(class_name, is_a_trait) } else { class_name.clone() };
        let existed = load(&file, &mut contents);
        if !existed {
            let head = if extension {
                format!("Extension {{ #name : {} }}\n", sym(class_name, symbol_style))
            } else {
                return Err(format!(
                    "{} is not on disk ({} {} has no file in {}). Create it through `class create` first or check --dir.",
                    file.display(), kind, class_name, pkg_dir.display()
                ));
            };
            contents.insert(file.clone(), head);
            stats.entry(file.clone()).or_default().created = true;
        }
        let text = contents.get(&file).unwrap().clone();
        let blocks = method_blocks(&text);
        let found = blocks.iter().find(|b| b.class_header == header && &b.selector == selector);
        let label = format!("{}>>{}", header, selector);
        let st = stats.entry(file.clone()).or_default();
        let new_text = if c["kind"] == "method-remove" {
            match found {
                Some(b) => {
                    st.removed.push(label);
                    let mut t = text.clone();
                    t.replace_range(b.start..b.end, "");
                    t
                }
                None => text,
            }
        } else {
            let protocol = c["protocol"].as_str().unwrap_or("as yet unclassified");
            let block = render_method(&header, protocol, c["new_source"].as_str().unwrap_or(""), symbol_style);
            match found {
                Some(b) => {
                    st.replaced.push(label);
                    // keep original separation: block ends where the next one starts
                    let tail_gap = if b.end < text.len() { "\n" } else { "" };
                    let mut t = text.clone();
                    t.replace_range(b.start..b.end, &format!("{}{}", block, tail_gap));
                    t
                }
                None => {
                    st.added.push(label);
                    let mut t = text.trim_end().to_string();
                    t.push_str("\n\n");
                    t.push_str(&block);
                    t
                }
            }
        };
        contents.insert(file, new_text);
    }

    let mut report = Vec::new();
    for (path, text) in &contents {
        let st = stats.get(path);
        if !dry_run {
            if let Some(parent) = path.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
            let pkg_file = pkg_dir.join("package.st");
            if !pkg_file.exists() {
                let name = if symbol_style { sym(package, true) } else { sym(package, false) };
                fs::write(&pkg_file, format!("Package {{ #name : {} }}\n", name)).map_err(|e| e.to_string())?;
            }
            let mut out = text.trim_end().to_string();
            out.push('\n');
            fs::write(path, out).map_err(|e| format!("{}: {}", path.display(), e))?;
        }
        report.push(json!({
            "file": path.display().to_string(),
            "created": st.map(|s| s.created).unwrap_or(false),
            "added": st.map(|s| s.added.clone()).unwrap_or_default(),
            "replaced": st.map(|s| s.replaced.clone()).unwrap_or_default(),
            "removed": st.map(|s| s.removed.clone()).unwrap_or_default(),
        }));
    }
    Ok(Plan { report, marked })
}

/// One method as written in a Tonel file: class header ("Foo" / "Foo class"), selector, normalised source.
pub struct FileMethod {
    pub class_header: String,
    pub selector: String,
    pub source: String,
}

fn normalise(src: &str) -> String {
    let lines: Vec<String> = norm(src).lines().map(|l| l.trim_end().to_string()).collect();
    lines.join("\n").trim_end().to_string()
}

/// All methods of a Tonel `.class.st` / `.extension.st` text.
pub fn parse_methods(text: &str) -> Vec<FileMethod> {
    let mut out = Vec::new();
    for b in method_blocks(text) {
        let block = &text[b.start..b.end];
        let lines: Vec<&str> = block.lines().collect();
        if lines.len() < 2 { continue; }
        let mut h = match parse_header(lines[1]) {
            Some(h) => h,
            None => continue,
        };
        // body: everything after the header line up to the closing "]" line
        let mut body: Vec<&str> = lines[2..].to_vec();
        while body.last().map(|l| l.trim().is_empty()).unwrap_or(false) { body.pop(); }
        if body.is_empty() {
            // a method on one line, `Foo >> pattern [ body ]`: its closing bracket is the LAST `]` of the header line.
            // The line is complete only when nothing follows it (the next block or the end of the file), so a
            // header line that ends in `]` with body lines below it keeps that `]`.
            match h.tail.strip_suffix(']') {
                Some(t) => h.tail = t.trim_end(),
                None => continue,
            }
        } else if body.last().map(|l| l.trim() == "]").unwrap_or(false) {
            body.pop();
        }
        out.push(FileMethod { class_header: b.class_header.clone(), selector: b.selector.clone(), source: normalise(&method_source(&h, &body)) });
    }
    out
}

/// Compare the methods the image has for a package with the Tonel files in `<dir>/<package>/`.
/// Classes and traits (`.class.st` / `.trait.st`) are both compared; trait class-side methods appear as
/// `TFoo classSide >> sel` in the files and as side `class` in the image.
pub fn drift(image: &Value, package: &str, dir: &Path) -> Value {
    let pkg_dir = dir.join(package);
    let mut in_files: BTreeMap<String, String> = BTreeMap::new();
    let mut file_classes: Vec<String> = Vec::new();
    let mut file_traits: Vec<String> = Vec::new();
    if let Ok(rd) = fs::read_dir(&pkg_dir) {
        for e in rd.flatten() {
            let p = e.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            if name.ends_with(".class.st") { file_classes.push(name.trim_end_matches(".class.st").to_string()); }
            if name.ends_with(".trait.st") { file_traits.push(name.trim_end_matches(".trait.st").to_string()); }
            if name.ends_with(".class.st") || name.ends_with(".trait.st") || name.ends_with(".extension.st") {
                if let Ok(t) = fs::read_to_string(&p) {
                    for m in parse_methods(&t) {
                        let (cls, side) = split_header(&m.class_header);
                        in_files.insert(format!("{}|{}|{}", cls, side, m.selector), m.source);
                    }
                }
            }
        }
    }
    let mut in_image: BTreeMap<String, String> = BTreeMap::new();
    for m in image["methods"].as_array().cloned().unwrap_or_default() {
        in_image.insert(format!("{}|{}|{}", m["class"].as_str().unwrap_or(""), m["side"].as_str().unwrap_or("instance"), m["selector"].as_str().unwrap_or("")), m["source"].as_str().unwrap_or("").to_string());
    }
    let label = |k: &str| { let p: Vec<&str> = k.split('|').collect(); if p[1] == "class" { format!("{} class>>{}", p[0], p[2]) } else { format!("{}>>{}", p[0], p[2]) } };
    let missing_in_files: Vec<String> = in_image.keys().filter(|k| !in_files.contains_key(*k)).map(|k| label(k)).collect();
    let missing_in_image: Vec<String> = in_files.keys().filter(|k| !in_image.contains_key(*k)).map(|k| label(k)).collect();
    let different: Vec<String> = in_image.iter().filter(|(k, v)| in_files.get(*k).map(|f| &normalise(f) != &normalise(v)).unwrap_or(false)).map(|(k, _)| label(k)).collect();
    let names = |key: &str| -> Vec<String> { image[key].as_array().map(|a| a.iter().filter_map(|c| c.as_str().map(String::from)).collect()).unwrap_or_default() };
    // images before 0.7 list traits under `classes`; the files decide whether a name is a class or a trait
    let mut image_classes: Vec<String> = names("classes");
    image_classes.extend(names("traits"));
    let mut file_behaviors: Vec<String> = file_classes.clone();
    file_behaviors.extend(file_traits.iter().cloned());
    let classes_missing_in_files: Vec<&String> = image_classes.iter().filter(|c| !file_behaviors.contains(c)).collect();
    let classes_missing_in_image: Vec<&String> = file_behaviors.iter().filter(|c| !image_classes.contains(c)).collect();
    let ok = missing_in_files.is_empty() && missing_in_image.is_empty() && different.is_empty() && classes_missing_in_files.is_empty() && classes_missing_in_image.is_empty();
    json!({
        "package": package,
        "ok": ok,
        "methods_in_image": in_image.len(),
        "methods_in_files": in_files.len(),
        "missing_in_files": missing_in_files,
        "missing_in_image": missing_in_image,
        "different": different,
        "classes_missing_in_files": classes_missing_in_files,
        "classes_missing_in_image": classes_missing_in_image,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("stlive-save-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("Pkg")).unwrap();
        d
    }

    const TRAIT_FILE: &str = "Trait {\n\t#name : 'TFoo',\n\t#instVars : [\n\t\t'tv'\n\t],\n\t#package : 'Pkg'\n}\n\n{ #category : 'x' }\nTFoo classSide >> make [\n\t^ self new\n]\n\n{ #category : 'x' }\nTFoo >> hello [\n\t^ 1\n]\n";

    #[test]
    fn parses_trait_class_side_headers() {
        let ms = parse_methods(TRAIT_FILE);
        assert_eq!(ms.len(), 2);
        assert_eq!(split_header(&ms[0].class_header), ("TFoo".to_string(), "class"));
        assert_eq!(ms[0].selector, "make");
        assert_eq!(split_header(&ms[1].class_header), ("TFoo".to_string(), "instance"));
        assert_eq!(split_header("Foo class"), ("Foo".to_string(), "class"));
    }

    #[test]
    fn saves_trait_methods_into_the_trait_file() {
        let dir = tmp("trait-methods");
        fs::write(dir.join("Pkg/TFoo.trait.st"), TRAIT_FILE).unwrap();
        let changes = vec![
            json!({"index": 1, "kind": "method-compile", "class_name": "TFoo", "class_kind": "trait", "class_package": "Pkg", "package": "Pkg", "side": "instance", "selector": "hello", "protocol": "x", "new_source": "hello\n\t^ 2"}),
            json!({"index": 2, "kind": "method-compile", "class_name": "TFoo", "class_kind": "trait", "class_package": "Pkg", "package": "Pkg", "side": "class", "selector": "other", "protocol": "x", "new_source": "other\n\t^ 3"}),
        ];
        let p = plan(&changes, "Pkg", &dir, false).unwrap();
        assert_eq!(p.marked, vec![1, 2]);
        let text = fs::read_to_string(dir.join("Pkg/TFoo.trait.st")).unwrap();
        assert!(text.contains("TFoo >> hello [\n\t^ 2\n]"), "{}", text);
        assert!(text.contains("TFoo classSide >> other [\n\t^ 3\n]"), "{}", text);
        assert!(!dir.join("Pkg/TFoo.class.st").exists());
        // an old image sends no class_kind: the existing .trait.st file decides
        let old = vec![json!({"index": 3, "kind": "method-compile", "class_name": "TFoo", "class_package": "Pkg", "package": "Pkg", "side": "class", "selector": "make", "protocol": "x", "new_source": "make\n\t^ self basicNew"})];
        plan(&old, "Pkg", &dir, false).unwrap();
        let text = fs::read_to_string(dir.join("Pkg/TFoo.trait.st")).unwrap();
        assert!(text.contains("TFoo classSide >> make [\n\t^ self basicNew\n]"), "{}", text);
        assert_eq!(text.matches(" >> make [").count(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn creates_trait_definitions_and_classes_using_traits() {
        let dir = tmp("trait-create");
        let changes = vec![
            json!({"index": 1, "kind": "class-create", "class_name": "TBar", "class_kind": "trait", "package": "Pkg", "class_package": "Pkg", "superclass": null, "instance_variables": ["x"], "class_variables": [], "class_instance_variables": []}),
            json!({"index": 2, "kind": "class-create", "class_name": "Bar", "class_kind": "class", "package": "Pkg", "class_package": "Pkg", "superclass": "Object", "traits": "TBar", "class_traits": "TBar classTrait", "instance_variables": ["a"], "class_variables": [], "class_instance_variables": []}),
        ];
        plan(&changes, "Pkg", &dir, false).unwrap();
        let t = fs::read_to_string(dir.join("Pkg/TBar.trait.st")).unwrap();
        assert_eq!(t, "Trait {\n\t#name : 'TBar',\n\t#instVars : [\n\t\t'x'\n\t],\n\t#package : 'Pkg'\n}\n");
        let c = fs::read_to_string(dir.join("Pkg/Bar.class.st")).unwrap();
        assert_eq!(c, "Class {\n\t#name : 'Bar',\n\t#superclass : 'Object',\n\t#traits : 'TBar',\n\t#classTraits : 'TBar classTrait',\n\t#instVars : [\n\t\t'a'\n\t],\n\t#package : 'Pkg'\n}\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn patching_variables_keeps_traits_and_category_lines() {
        let file = "\"\nA comment.\n\"\nClass {\n\t#name : 'Bar',\n\t#superclass : 'Object',\n\t#traits : 'TBar',\n\t#classTraits : 'TBar classTrait',\n\t#instVars : [\n\t\t'a'\n\t],\n\t#category : 'Pkg',\n\t#package : 'Pkg'\n}\n\n{ #category : 'x' }\nBar >> one [\n\t^ 1\n]\n";
        let c = json!({"class_name": "Bar", "instance_variables": ["a", "b"], "class_variables": ["Registry"], "class_instance_variables": []});
        let out = patch_class_definition(file, &c, false, "Pkg").unwrap();
        let expected = "\"\nA comment.\n\"\nClass {\n\t#name : 'Bar',\n\t#superclass : 'Object',\n\t#traits : 'TBar',\n\t#classTraits : 'TBar classTrait',\n\t#instVars : [\n\t\t'a',\n\t\t'b'\n\t],\n\t#classVars : [\n\t\t'Registry'\n\t],\n\t#category : 'Pkg',\n\t#package : 'Pkg'\n}\n\n{ #category : 'x' }\nBar >> one [\n\t^ 1\n]\n";
        assert_eq!(out, expected);
        // unchanged variables: nothing to patch
        let same = json!({"class_name": "Bar", "instance_variables": ["a"], "class_variables": [], "class_instance_variables": []});
        assert!(patch_class_definition(file, &same, false, "Pkg").is_none());
        // a trait definition is patched the same way
        let t = json!({"class_name": "TFoo", "instance_variables": ["tv", "tw"], "class_variables": [], "class_instance_variables": []});
        let out = patch_class_definition(TRAIT_FILE, &t, false, "Pkg").unwrap();
        assert!(out.starts_with("Trait {\n\t#name : 'TFoo',\n\t#instVars : [\n\t\t'tv',\n\t\t'tw'\n\t],\n\t#package : 'Pkg'\n}\n"), "{}", out);
    }

    #[test]
    fn class_comment_changes_replace_the_leading_comment_block() {
        let file = "\"\nOld comment.\n\"\nClass {\n\t#name : 'Bar',\n\t#superclass : 'Object',\n\t#instVars : [\n\t\t'a'\n\t],\n\t#package : 'Pkg'\n}\n\n{ #category : 'x' }\nBar >> one [\n\t^ 1\n]\n";
        let with_comment = |comment: Value| {
            let mut c = json!({"class_name": "Bar", "instance_variables": ["a"], "class_variables": [], "class_instance_variables": []});
            c["comment"] = comment;
            c
        };
        // (a) a changed comment replaces only the comment block
        let out = patch_class_definition(file, &with_comment(json!("New comment.")), false, "Pkg").unwrap();
        assert_eq!(out, file.replacen("Old comment.", "New comment.", 1));
        // the same comment (line endings do not matter) changes nothing
        assert!(patch_class_definition(file, &with_comment(json!("Old comment.\r\n")), false, "Pkg").is_none());
        // a change that sends no comment leaves the file's comment alone
        assert!(patch_class_definition(file, &with_comment(Value::Null), false, "Pkg").is_none());
        // (d) an empty image comment removes the comment block
        let out = patch_class_definition(file, &with_comment(json!("")), false, "Pkg").unwrap();
        assert_eq!(out, file.replacen("\"\nOld comment.\n\"\n", "", 1));
        // (b) a file without a comment gets one
        let bare = file.replacen("\"\nOld comment.\n\"\n", "", 1);
        let out = patch_class_definition(&bare, &with_comment(json!("A comment.")), false, "Pkg").unwrap();
        assert_eq!(out, format!("\"\nA comment.\n\"\n{}", bare));
        // (c) a double quote is written as two and reads back as the same comment
        let quoted = patch_class_definition(file, &with_comment(json!("Say \"hi\".")), false, "Pkg").unwrap();
        assert!(quoted.starts_with("\"\nSay \"\"hi\"\".\n\"\nClass {"), "{}", quoted);
        assert!(patch_class_definition(&quoted, &with_comment(json!("Say \"hi\".")), false, "Pkg").is_none());
    }

    #[test]
    fn saving_a_changed_class_comment_patches_the_existing_file() {
        let dir = tmp("class-comment");
        fs::write(dir.join("Pkg/Bar.class.st"), "\"\nOld.\n\"\nClass {\n\t#name : 'Bar',\n\t#superclass : 'Object',\n\t#instVars : [\n\t\t'a'\n\t],\n\t#package : 'Pkg'\n}\n").unwrap();
        let changes = vec![json!({"index": 1, "kind": "class-create", "class_name": "Bar", "class_kind": "class", "package": "Pkg", "class_package": "Pkg", "superclass": "Object", "instance_variables": ["a"], "class_variables": [], "class_instance_variables": [], "comment": "New \"text\"."})];
        let p = plan(&changes, "Pkg", &dir, false).unwrap();
        assert_eq!(p.marked, vec![1]);
        let text = fs::read_to_string(dir.join("Pkg/Bar.class.st")).unwrap();
        assert_eq!(text, "\"\nNew \"\"text\"\".\n\"\nClass {\n\t#name : 'Bar',\n\t#superclass : 'Object',\n\t#instVars : [\n\t\t'a'\n\t],\n\t#package : 'Pkg'\n}\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn drift_compares_traits_and_their_class_side() {
        let dir = tmp("trait-drift");
        fs::write(dir.join("Pkg/TFoo.trait.st"), TRAIT_FILE).unwrap();
        fs::write(dir.join("Pkg/Bar.class.st"), "Class {\n\t#name : 'Bar',\n\t#superclass : 'Object',\n\t#traits : 'TFoo',\n\t#package : 'Pkg'\n}\n\n{ #category : 'x' }\nBar >> own [\n\t^ 1\n]\n").unwrap();
        // what a 0.7 image answers: only locally defined methods, traits listed separately
        let image = json!({"classes": ["Bar"], "traits": ["TFoo"], "methods": [
            {"class": "TFoo", "side": "class", "selector": "make", "source": "make\n\t^ self new"},
            {"class": "TFoo", "side": "instance", "selector": "hello", "source": "hello\n\t^ 1"},
            {"class": "Bar", "side": "instance", "selector": "own", "source": "own\n\t^ 1"}]});
        let r = drift(&image, "Pkg", &dir);
        assert_eq!(r["ok"], json!(true), "{}", r);
        // an older image lists the trait under classes: still no drift
        let old = json!({"classes": ["Bar", "TFoo"], "methods": image["methods"].clone()});
        assert_eq!(drift(&old, "Pkg", &dir)["ok"], json!(true));
        // a trait method the image reports under the using class is real drift (that is what old images did)
        let mut leaked = image.clone();
        leaked["methods"].as_array_mut().unwrap().push(json!({"class": "Bar", "side": "instance", "selector": "hello", "source": "hello\n\t^ 1"}));
        let r = drift(&leaked, "Pkg", &dir);
        assert_eq!(r["missing_in_files"], json!(["Bar>>hello"]));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Methods whose body starts on the header line: unary, keyword (one and two keywords), binary, a pragma,
    /// and a body with a `[` of its own on the header line.
    const HEADER_LINE_FILE: &str = "{ #category : 'x' }\nDemoA >> one [ ^ 1\n]\n\n{ #category : 'x' }\nDemoA >> add: x [ ^ x + 1\n]\n\n{ #category : 'x' }\nDemoA >> at: i put: v [ ^ v\n]\n\n{ #category : 'x' }\nDemoA >> + other [ ^ other\n]\n\n{ #category : 'x' }\nDemoA >> three [ ^ [ 3 ] value\n]\n\n{ #category : 'baselines' }\nBaselineOfDemo >> baseline: spec [ <baseline>\n\tspec for: #common do: [\n\t\tspec package: 'Demo' ]\n]\n";

    #[test]
    fn header_line_bodies_parse_like_the_image_canonical_source() {
        let ms = parse_methods(HEADER_LINE_FILE);
        let got: Vec<(&str, &str)> = ms.iter().map(|m| (m.selector.as_str(), m.source.as_str())).collect();
        assert_eq!(got, vec![
            ("one", "one\n\t^ 1"),
            ("add:", "add: x\n\t^ x + 1"),
            ("at:put:", "at: i put: v\n\t^ v"),
            ("+", "+ other\n\t^ other"),
            ("three", "three\n\t^ [ 3 ] value"),
            ("baseline:", "baseline: spec\n\t<baseline>\n\tspec for: #common do: [\n\t\tspec package: 'Demo' ]"),
        ]);
        assert_eq!(ms[4].class_header, "DemoA");
        assert_eq!(ms[5].class_header, "BaselineOfDemo");
        // the standard layout (header alone on its line) gives the same canonical text as before
        let standard = "{ #category : 'baselines' }\nBaselineOfDemo >> baseline: spec [\n\t<baseline>\n\tspec for: #common do: [\n\t\tspec package: 'Demo' ]\n]\n";
        assert_eq!(parse_methods(standard)[0].source, "baseline: spec\n\t<baseline>\n\tspec for: #common do: [\n\t\tspec package: 'Demo' ]");
        // the selector is taken from the pattern alone, never from body text
        assert_eq!(method_blocks(HEADER_LINE_FILE).iter().map(|b| b.selector.as_str()).collect::<Vec<_>>(), vec!["one", "add:", "at:put:", "+", "three", "baseline:"]);
    }

    #[test]
    fn drift_is_clean_for_header_line_bodies() {
        let dir = tmp("header-line-drift");
        fs::write(dir.join("Pkg/BaselineOfDemo.class.st"), "Class {\n\t#name : 'BaselineOfDemo',\n\t#superclass : 'Object',\n\t#package : 'Pkg'\n}\n\n{ #category : 'baselines' }\nBaselineOfDemo >> baseline: spec [ <baseline>\n\tspec for: #common do: [\n\t\tspec package: 'Demo' ]\n]\n\n{ #category : 'x' }\nBaselineOfDemo >> one [ ^ 1\n]\n").unwrap();
        // the image's normalizedSourceOf: for the same methods
        let image = json!({"classes": ["BaselineOfDemo"], "methods": [
            {"class": "BaselineOfDemo", "side": "instance", "selector": "baseline:", "source": "baseline: spec\n\t<baseline>\n\tspec for: #common do: [\n\t\tspec package: 'Demo' ]"},
            {"class": "BaselineOfDemo", "side": "instance", "selector": "one", "source": "one\n\t^ 1"}]});
        let r = drift(&image, "Pkg", &dir);
        assert_eq!(r["ok"], json!(true), "{}", r);
        assert_eq!(r["missing_in_files"], json!([]));
        assert_eq!(r["different"], json!([]));
        // a real change in the body is still reported
        let mut changed = image.clone();
        changed["methods"][1]["source"] = json!("one\n\t^ 2");
        assert_eq!(drift(&changed, "Pkg", &dir)["different"], json!(["BaselineOfDemo>>one"]));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn saving_replaces_header_line_methods_in_place() {
        let dir = tmp("header-line-save");
        let file = "{ #category : 'x' }\nDemoA >> add: x [ ^ x + 1\n]\n\n{ #category : 'baselines' }\nDemoA >> baseline: spec [ <baseline>\n\tspec for: #common do: [\n\t\tspec package: 'Demo' ]\n]\n";
        fs::write(dir.join("Pkg/DemoA.class.st"), file).unwrap();
        let changes = vec![
            json!({"index": 1, "kind": "method-compile", "class_name": "DemoA", "class_package": "Pkg", "package": "Pkg", "side": "instance", "selector": "add:", "protocol": "x", "new_source": "add: x\n\t^ x + 2"}),
            json!({"index": 2, "kind": "method-compile", "class_name": "DemoA", "class_package": "Pkg", "package": "Pkg", "side": "instance", "selector": "baseline:", "protocol": "baselines", "new_source": "baseline: spec\n\t<baseline>\n\tspec for: #common do: [\n\t\tspec package: 'Other' ]"}),
        ];
        let p = plan(&changes, "Pkg", &dir, false).unwrap();
        assert_eq!(p.report[0]["added"], json!([]));
        assert_eq!(p.report[0]["replaced"], json!(["DemoA>>add:", "DemoA>>baseline:"]));
        let text = fs::read_to_string(dir.join("Pkg/DemoA.class.st")).unwrap();
        assert_eq!(text, "{ #category : 'x' }\nDemoA >> add: x [\n\t^ x + 2\n]\n\n{ #category : 'baselines' }\nDemoA >> baseline: spec [\n\t<baseline>\n\tspec for: #common do: [\n\t\tspec package: 'Other' ]\n]\n");
        assert_eq!(text.matches(" >> baseline:").count(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Methods written entirely on one line: `Foo >> pattern [ body ]`, the closing bracket being the last `]` of it.
    /// Unary, keyword, binary, and bodies with brackets of their own (`[ :e | e ] ]`, `[ 3 ] value`).
    const ONE_LINE_FILE: &str = "{ #category : 'x' }\nDemoB >> one [ ^ 1 ]\n\n{ #category : 'x' }\nDemoB >> add: x [ ^ x + 1 ]\n\n{ #category : 'x' }\nDemoB >> + other [ ^ other ]\n\n{ #category : 'x' }\nDemoB >> nested [ ^ #(1 2) collect: [ :e | e ] ]\n\n{ #category : 'x' }\nDemoB >> three [ ^ [ 3 ] value ]\n";

    #[test]
    fn one_line_methods_parse_like_the_image_canonical_source() {
        let ms = parse_methods(ONE_LINE_FILE);
        let got: Vec<(&str, &str)> = ms.iter().map(|m| (m.selector.as_str(), m.source.as_str())).collect();
        assert_eq!(got, vec![
            ("one", "one\n\t^ 1"),
            ("add:", "add: x\n\t^ x + 1"),
            ("+", "+ other\n\t^ other"),
            ("nested", "nested\n\t^ #(1 2) collect: [ :e | e ]"),
            ("three", "three\n\t^ [ 3 ] value"),
        ]);
        // the header line ends in `]` but body lines follow it: that `]` belongs to the body, the method closes below
        let open = "{ #category : 'x' }\nDemoB >> x [ ^ #(1 2) collect: [ :e | e ]\n\t+ 1\n]\n";
        assert_eq!(parse_methods(open)[0].source, "x\n\t^ #(1 2) collect: [ :e | e ]\n\t+ 1");
        // a blank line between the one-liner and the next block changes nothing
        let spaced = "{ #category : 'x' }\nDemoB >> one [ ^ 1 ]\n\n\n{ #category : 'x' }\nDemoB >> two [ ^ 2 ]\n";
        assert_eq!(parse_methods(spaced).iter().map(|m| m.source.clone()).collect::<Vec<_>>(), vec!["one\n\t^ 1", "two\n\t^ 2"]);
    }

    #[test]
    fn drift_is_clean_for_one_line_methods() {
        let dir = tmp("one-line-drift");
        fs::write(dir.join("Pkg/DemoB.class.st"), format!("Class {{\n\t#name : 'DemoB',\n\t#superclass : 'Object',\n\t#package : 'Pkg'\n}}\n\n{}", ONE_LINE_FILE)).unwrap();
        // the image's normalizedSourceOf: for the same methods
        let image = json!({"classes": ["DemoB"], "methods": [
            {"class": "DemoB", "side": "instance", "selector": "one", "source": "one\n\t^ 1"},
            {"class": "DemoB", "side": "instance", "selector": "add:", "source": "add: x\n\t^ x + 1"},
            {"class": "DemoB", "side": "instance", "selector": "+", "source": "+ other\n\t^ other"},
            {"class": "DemoB", "side": "instance", "selector": "nested", "source": "nested\n\t^ #(1 2) collect: [ :e | e ]"},
            {"class": "DemoB", "side": "instance", "selector": "three", "source": "three\n\t^ [ 3 ] value"}]});
        let r = drift(&image, "Pkg", &dir);
        assert_eq!(r["ok"], json!(true), "{}", r);
        assert_eq!(r["methods_in_files"], json!(5));
        // a changed one-liner is real drift
        let mut changed = image.clone();
        changed["methods"][0]["source"] = json!("one\n\t^ 2");
        assert_eq!(drift(&changed, "Pkg", &dir)["different"], json!(["DemoB>>one"]));
        // a one-liner the files lack is reported as missing in files
        fs::write(dir.join("Pkg/DemoB.class.st"), format!("Class {{\n\t#name : 'DemoB',\n\t#superclass : 'Object',\n\t#package : 'Pkg'\n}}\n\n{}", ONE_LINE_FILE.replace("DemoB >> one [ ^ 1 ]\n", ""))).unwrap();
        assert_eq!(drift(&image, "Pkg", &dir)["missing_in_files"], json!(["DemoB>>one"]));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn saving_replaces_one_line_methods_in_place() {
        let dir = tmp("one-line-save");
        fs::write(dir.join("Pkg/DemoB.class.st"), ONE_LINE_FILE).unwrap();
        let changes = vec![
            json!({"index": 1, "kind": "method-compile", "class_name": "DemoB", "class_package": "Pkg", "package": "Pkg", "side": "instance", "selector": "one", "protocol": "x", "new_source": "one\n\t^ 2"}),
            json!({"index": 2, "kind": "method-remove", "class_name": "DemoB", "class_package": "Pkg", "package": "Pkg", "side": "instance", "selector": "nested"}),
        ];
        let p = plan(&changes, "Pkg", &dir, false).unwrap();
        assert_eq!(p.report[0]["replaced"], json!(["DemoB>>one"]));
        assert_eq!(p.report[0]["removed"], json!(["DemoB>>nested"]));
        let text = fs::read_to_string(dir.join("Pkg/DemoB.class.st")).unwrap();
        assert_eq!(text, "{ #category : 'x' }\nDemoB >> one [\n\t^ 2\n]\n\n{ #category : 'x' }\nDemoB >> add: x [ ^ x + 1 ]\n\n{ #category : 'x' }\nDemoB >> + other [ ^ other ]\n\n{ #category : 'x' }\nDemoB >> three [ ^ [ 3 ] value ]\n");
        // the other one-liners are untouched and still drift-free against the image
        let image = json!({"classes": ["DemoB"], "methods": [
            {"class": "DemoB", "side": "instance", "selector": "one", "source": "one\n\t^ 2"},
            {"class": "DemoB", "side": "instance", "selector": "add:", "source": "add: x\n\t^ x + 1"},
            {"class": "DemoB", "side": "instance", "selector": "+", "source": "+ other\n\t^ other"},
            {"class": "DemoB", "side": "instance", "selector": "three", "source": "three\n\t^ [ 3 ] value"}]});
        assert_eq!(drift(&image, "Pkg", &dir)["ok"], json!(true));
        let _ = fs::remove_dir_all(&dir);
    }
}
