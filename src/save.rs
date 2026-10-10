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

/// Selector of the method whose first source line (the pattern) is `pattern`.
fn selector_of_pattern(pattern: &str) -> String {
    let toks: Vec<&str> = pattern.split_whitespace().collect();
    match toks.first() {
        None => String::new(),
        Some(t) if t.ends_with(':') => toks.iter().step_by(2).cloned().collect::<String>(),
        Some(t) if toks.len() == 2 && !t.chars().next().unwrap().is_alphanumeric() && *t != "_" => t.to_string(),
        Some(t) => t.to_string(),
    }
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
        if let Some(next) = lines.get(li + 1) {
            if let Some((head, rest)) = next.split_once(" >> ") {
                let pattern = rest.trim_end().trim_end_matches('[').trim();
                blocks.push(Block { start: *off, end, class_header: head.to_string(), selector: selector_of_pattern(pattern) });
            }
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

fn render_class(c: &Value, symbol_style: bool, package: &str) -> String {
    let name = c["class_name"].as_str().unwrap_or("");
    let sup = c["superclass"].as_str().unwrap_or("Object");
    let list = |label: &str, items: Vec<String>| -> String {
        if items.is_empty() { return String::new(); }
        let body = items.iter().map(|v| format!("\t\t'{}'", v)).collect::<Vec<_>>().join(",\n");
        format!("\t#{} : [\n{}\n\t],\n", label, body)
    };
    let mut s = String::new();
    if let Some(comment) = c["comment"].as_str() {
        if !comment.is_empty() {
            s.push_str(&format!("\"\n{}\n\"\n", norm(comment).replace('"', "\"\"")));
        }
    }
    s.push_str("Class {\n");
    s.push_str(&format!("\t#name : {},\n\t#superclass : {},\n", sym(name, symbol_style), sym(sup, symbol_style)));
    s.push_str(&list("instVars", str_list(c, "instance_variables")));
    s.push_str(&list("classVars", str_list(c, "class_variables")));
    s.push_str(&list("classInstVars", str_list(c, "class_instance_variables")));
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

/// If the image's class has other instance/class variables than the file, re-render the definition block
/// (keeping name, superclass and package/category lines of the file); otherwise answer None.
fn patch_class_definition(text: &str, c: &Value, symbol_style: bool, package: &str) -> Option<String> {
    let start = text.find("Class {")?;
    let end = start + text[start..].find("\n}")? + 2;
    let block = &text[start..end];
    let wanted = [
        ("instVars", str_list(c, "instance_variables")),
        ("classVars", str_list(c, "class_variables")),
        ("classInstVars", str_list(c, "class_instance_variables")),
    ];
    if wanted.iter().all(|(k, v)| &listed(block, k) == v) {
        return None;
    }
    let keep = |prefix: &str| block.lines().find(|l| l.trim_start().starts_with(prefix)).map(|l| l.to_string());
    let mut out = String::from("Class {\n");
    for p in ["#name :", "#superclass :"] {
        if let Some(l) = keep(p) { out.push_str(&l); out.push('\n'); }
    }
    for (k, v) in &wanted {
        if !v.is_empty() {
            let body = v.iter().map(|x| format!("\t\t'{}'", x)).collect::<Vec<_>>().join(",\n");
            out.push_str(&format!("\t#{} : [\n{}\n\t],\n", k, body));
        }
    }
    let tail = keep("#package :").or_else(|| keep("#category :")).unwrap_or_else(|| {
        if symbol_style { format!("\t#category : {}", sym(package, true)) } else { format!("\t#package : '{}'", package) }
    });
    out.push_str(&tail);
    out.push_str("\n}");
    Some(format!("{}{}{}", &text[..start], out, &text[end..]))
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
        let path = pkg_dir.join(format!("{}.class.st", name));
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
        let file = pkg_dir.join(format!("{}.{}.st", class_name, if extension { "extension" } else { "class" }));
        let header = if side == "class" { format!("{} class", class_name) } else { class_name.clone() };
        let existed = load(&file, &mut contents);
        if !existed {
            let head = if extension {
                format!("Extension {{ #name : {} }}\n", sym(class_name, symbol_style))
            } else {
                return Err(format!(
                    "{} is not on disk (class {} has no file in {}). Create the class through `class create` first or check --dir.",
                    file.display(), class_name, pkg_dir.display()
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
        if lines.len() < 3 { continue; }
        let head = lines[1];
        let pattern = head.split_once(" >> ").map(|(_, r)| r.trim_end().trim_end_matches('[').trim()).unwrap_or("");
        // body: everything after the header line up to the closing "]" line
        let mut body: Vec<&str> = lines[2..].to_vec();
        while body.last().map(|l| l.trim().is_empty()).unwrap_or(false) { body.pop(); }
        if body.last().map(|l| l.trim() == "]").unwrap_or(false) { body.pop(); }
        let mut source = pattern.to_string();
        for l in body { source.push('\n'); source.push_str(l); }
        out.push(FileMethod { class_header: b.class_header.clone(), selector: b.selector.clone(), source: normalise(&source) });
    }
    out
}

/// Compare the methods the image has for a package with the Tonel files in `<dir>/<package>/`.
pub fn drift(image: &Value, package: &str, dir: &Path) -> Value {
    let pkg_dir = dir.join(package);
    let mut in_files: BTreeMap<String, String> = BTreeMap::new();
    let mut file_classes: Vec<String> = Vec::new();
    if let Ok(rd) = fs::read_dir(&pkg_dir) {
        for e in rd.flatten() {
            let p = e.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            if name.ends_with(".class.st") { file_classes.push(name.trim_end_matches(".class.st").to_string()); }
            if name.ends_with(".class.st") || name.ends_with(".extension.st") {
                if let Ok(t) = fs::read_to_string(&p) {
                    for m in parse_methods(&t) {
                        let (cls, side) = match m.class_header.strip_suffix(" class") { Some(c) => (c.to_string(), "class"), None => (m.class_header.clone(), "instance") };
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
    let image_classes: Vec<String> = image["classes"].as_array().map(|a| a.iter().filter_map(|c| c.as_str().map(String::from)).collect()).unwrap_or_default();
    let classes_missing_in_files: Vec<&String> = image_classes.iter().filter(|c| !file_classes.contains(c)).collect();
    let classes_missing_in_image: Vec<&String> = file_classes.iter().filter(|c| !image_classes.contains(c)).collect();
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
