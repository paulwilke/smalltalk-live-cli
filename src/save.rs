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

fn render_class(c: &Value, symbol_style: bool, package: &str) -> String {
    let name = c["class_name"].as_str().unwrap_or("");
    let sup = c["superclass"].as_str().unwrap_or("Object");
    let ivars: Vec<&str> = c["instance_variables"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect()).unwrap_or_default();
    let mut s = String::from("Class {\n");
    s.push_str(&format!("\t#name : {},\n\t#superclass : {},\n", sym(name, symbol_style), sym(sup, symbol_style)));
    if !ivars.is_empty() {
        s.push_str("\t#instVars : [\n");
        s.push_str(&ivars.iter().map(|v| format!("\t\t'{}'", v)).collect::<Vec<_>>().join(",\n"));
        s.push_str("\n\t],\n");
    }
    if symbol_style {
        s.push_str(&format!("\t#category : {}\n}}\n", sym(package, true)));
    } else {
        s.push_str(&format!("\t#package : '{}'\n}}\n", package));
    }
    s
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
            "class-create" => class_defs.push(c),
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
            continue; // never rewrite an existing class definition automatically
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
