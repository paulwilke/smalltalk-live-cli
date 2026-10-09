//! stlive – thin client for a persistent Pharo image running the StLive server.
//!
//! One request line (JSON) per connection, one response line back. All state (objects,
//! debug sessions) lives in the image, so every invocation is short-lived.
//!
//! Exit codes: 0 ok · 1 command failed / exception in image / failing tests · 2 usage
//!             3 image not running or unreachable · 4 protocol error · 5 timeout

mod init;
mod save;

use clap::{Parser, Subcommand};
use serde_json::{json, Map, Value};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const SERVE_ST: &str = "(Smalltalk globals at: #StLiveServer) startFromEnvironment.\n";

#[derive(Parser)]
#[command(name = "stlive", version, about = "Work inside a live Pharo image: eval, inspect, debug, fix, re-run.")]
struct Cli {
    /// Instance name (several images can run side by side)
    #[arg(long, short = 'i', global = true, env = "STLIVE_INSTANCE", default_value = "default")]
    instance: String,
    /// Directory holding instance state (port file, pid, log, config)
    #[arg(long, global = true, env = "STLIVE_DIR")]
    state_dir: Option<PathBuf>,
    /// Pretty-print JSON
    #[arg(long, global = true)]
    pretty: bool,
    /// Append one JSON line per call (time, command, duration, output size, ok, key result fields) to this file
    #[arg(long, global = true, env = "STLIVE_LOG")]
    log: Option<PathBuf>,
    /// Free-form marker written to the call log (scene, task, ...)
    #[arg(long, global = true, env = "STLIVE_TAG")]
    tag: Option<String>,
    /// Human-readable indented text instead of JSON (exit codes are unchanged)
    #[arg(long, global = true)]
    text: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// One-time setup: download Pharo 13 and build an image with the StLive server
    Init {
        /// Re-download and rebuild even if present
        #[arg(long)]
        force: bool,
    },
    /// List instances known in the state directory
    Instances,
    /// Start the image headless in the background
    Start {
        /// Pharo VM executable (remembered per instance)
        #[arg(long, env = "STLIVE_VM")]
        vm: Option<PathBuf>,
        /// Image prepared with the StLive package (remembered per instance)
        #[arg(long, env = "STLIVE_IMAGE")]
        image: Option<PathBuf>,
    },
    /// Stop the image (destructive: unsaved state and debug sessions are lost)
    Stop {
        /// Required confirmation
        #[arg(long)]
        force: bool,
    },
    /// Image identity and status
    Status,
    /// Check reachability
    Ping,
    /// Evaluate Smalltalk code ('-' reads stdin). Exceptions become debug sessions.
    Eval {
        code: String,
        /// Object reference used as `self`
        #[arg(long = "in")]
        receiver: Option<String>,
        /// Milliseconds before the running code is interrupted into a debug session
        #[arg(long)]
        timeout: Option<u64>,
        /// Also return the complete printString of the value as `text` (capped at 100000 chars)
        #[arg(long)]
        full: bool,
        /// On failure report the error but do not keep a suspended debug session
        #[arg(long)]
        no_session: bool,
    },
    /// Inspect objects by reference
    #[command(subcommand)]
    Obj(ObjCmd),
    /// Search the system
    #[command(subcommand)]
    Find(FindCmd),
    /// Classes
    #[command(subcommand)]
    Class(ClassCmd),
    /// Methods
    #[command(subcommand)]
    Method(MethodCmd),
    /// Recorded changes made through this CLI
    #[command(subcommand)]
    Changes(ChangesCmd),
    /// Tests
    #[command(subcommand)]
    Test(TestCmd),
    /// Packages
    #[command(subcommand)]
    Package(PackageCmd),
    /// Debug sessions
    #[command(subcommand)]
    Debug(DebugCmd),
    /// Image persistence, info and cloning
    #[command(subcommand)]
    Image(ImageCmd),
    /// Write changes made through this CLI back to Tonel files (minimal per-method diffs)
    Save {
        /// Package whose changes are written
        #[arg(long)]
        package: Option<String>,
        /// Write the changes of every package (one folder each)
        #[arg(long)]
        all: bool,
        /// Source directory containing one folder per package (e.g. the repo's src/)
        #[arg(long)]
        dir: PathBuf,
        /// Show what would be written
        #[arg(long)]
        dry_run: bool,
    },
    /// Load a Metacello baseline; reports the packages that appeared
    Load {
        /// Baseline name without the BaselineOf prefix, e.g. Mapless
        baseline: String,
        /// Repository, e.g. tonel:///abs/path/src or github://user/repo:master/src
        #[arg(long)]
        repository: String,
        /// Groups to load (repeatable); default group if none
        #[arg(long = "group")]
        groups: Vec<String>,
        #[arg(long)]
        timeout: Option<u64>,
    },
    /// Send a raw command: stlive raw debug.frames session=dbg-1 limit=3
    Raw { cmd: String, args: Vec<String> },
}

#[derive(Subcommand)]
enum ObjCmd {
    /// Summary with instance variables (paginated)
    Show { r#ref: String, #[arg(long)] offset: Option<u64>, #[arg(long)] limit: Option<u64>, /// Also show internals of system collections
        #[arg(long)] raw: bool },
    /// Elements of a collection (paginated)
    Items { r#ref: String, #[arg(long)] offset: Option<u64>, #[arg(long)] limit: Option<u64> },
    /// Full text of a string (or printString of any object), paginated by characters
    Text { r#ref: String, #[arg(long)] offset: Option<u64>, #[arg(long)] limit: Option<u64> },
    /// One instance variable
    Var { r#ref: String, name: String },
    /// Compare two objects (identity, equality, class)
    Compare { a: String, b: String },
    /// Objects that point to this one (registry and tool frames filtered out)
    Referrers { r#ref: String, #[arg(long)] limit: Option<u64> },
    /// Reachable graph: nodes with identity, edges with labels, cycles (back edges) and shared nodes
    Graph { r#ref: String, #[arg(long)] depth: Option<u64>, #[arg(long)] max_nodes: Option<u64> },
    /// Forget a reference (or --all)
    Release { r#ref: Option<String>, #[arg(long)] all: bool },
}

#[derive(Subcommand)]
enum FindCmd {
    Class { pattern: String, #[arg(long)] limit: Option<u64> },
    Package { pattern: String, #[arg(long)] limit: Option<u64> },
    Implementors { selector: String, #[arg(long)] limit: Option<u64> },
    Senders { selector: String, #[arg(long)] limit: Option<u64> },
}

#[derive(Subcommand)]
enum ClassCmd {
    /// Hierarchy, variables, methods ('Foo' or 'Foo class')
    Show { name: String, #[arg(long)] offset: Option<u64>, #[arg(long)] limit: Option<u64> },
    /// Create (or redefine) a class
    Create {
        name: String,
        #[arg(long)] superclass: Option<String>,
        /// Space separated instance variable names
        #[arg(long)] ivars: Option<String>,
        #[arg(long)] package: Option<String>,
        /// Instance variables on the class side (space separated)
        #[arg(long)] class_ivars: Option<String>,
        /// Class variables (space separated)
        #[arg(long)] class_vars: Option<String>,
        /// Class comment
        #[arg(long)] comment: Option<String>,
    },
}

#[derive(Subcommand)]
enum MethodCmd {
    /// Source of Class>>selector
    Show { target: String },
    /// Compile a method into a class (source from --source, --file or stdin)
    Compile {
        class: String,
        /// Method source; '-' reads stdin (same as omitting it)
        source_arg: Option<String>,
        #[arg(long)] source: Option<String>,
        /// Read the source from a file ('-' = stdin)
        #[arg(long)] file: Option<PathBuf>,
        #[arg(long)] protocol: Option<String>,
    },
    /// Remove a method (destructive)
    Remove { class: String, selector: String, #[arg(long)] force: bool },
}

#[derive(Subcommand)]
enum ChangesCmd {
    List { #[arg(long)] offset: Option<u64>, #[arg(long)] limit: Option<u64> },
    Show { index: u64 },
}

#[derive(Subcommand)]
enum PackageCmd {
    /// List packages (substring or glob like 'Mapless*'; default all)
    List { pattern: Option<String>, #[arg(long)] limit: Option<u64> },
}

#[derive(Subcommand)]
enum TestCmd {
    /// List test classes (substring or glob on class or package name)
    List { pattern: Option<String>, #[arg(long)] limit: Option<u64> },
    /// Run a TestCase class, 'Class>>testSelector', or a package (also repeats single tests)
    Run {
        target: String,
        /// Milliseconds per test
        #[arg(long)]
        timeout: Option<u64>,
        /// Keep every failing test's debug session (default: one per failure group, the rest are ended)
        #[arg(long)]
        keep_sessions: bool,
        /// End all sessions of this run, including the representative one per group
        #[arg(long)]
        discard_sessions: bool,
    },
}

#[derive(Subcommand)]
enum DebugCmd {
    /// Sessions (ended ones only with --all)
    List { #[arg(long)] all: bool },
    /// Compact description of a session
    Inspect { session: String },
    /// A page of frames (default 5)
    Frames { session: String, #[arg(long)] offset: Option<u64>, #[arg(long)] limit: Option<u64> },
    /// One frame in detail (source, receiver, variables)
    Frame { session: String, index: Option<u64>, #[arg(long)] frame: Option<u64>, /// Omit the method source
        #[arg(long)] no_source: bool },
    /// Arguments and temporaries of a frame
    Locals { session: String, frame_pos: Option<u64>, #[arg(long)] frame: Option<u64> },
    /// Receiver of a frame as an inspectable object
    Receiver { session: String, frame_pos: Option<u64>, #[arg(long)] frame: Option<u64> },
    /// Evaluate code in a frame (temporaries visible; --frame N, default 0 = top frame)
    Eval { session: String, code: String, #[arg(long, default_value_t = 0)] frame: u64, #[arg(long)] timeout: Option<u64>, #[arg(long)] full: bool, #[arg(long)] no_session: bool },
    /// CONTINUE the suspended execution (does not re-run; does not prove a fix)
    Resume { session: String, /// Smalltalk expression whose value the failed send answers
        #[arg(long)] value: Option<String>, #[arg(long)] timeout: Option<u64> },
    /// Unwind to a frame and RESTART it with the current method code
    Restart { session: String, frame_pos: Option<u64>, #[arg(long)] frame: Option<u64>, #[arg(long)] timeout: Option<u64> },
    /// Make a frame return a value and continue
    Return { session: String, value: String, frame_pos: Option<u64>, #[arg(long)] frame: Option<u64>, #[arg(long)] timeout: Option<u64> },
    /// RE-RUN the original operation from scratch with current code
    Rerun { session: String },
    /// Terminate a session (runs unwind blocks) or --all
    Terminate { session: Option<String>, #[arg(long)] all: bool },
}

#[derive(Subcommand)]
enum ImageCmd {
    /// Snapshot the image to disk
    Save,
    /// Version, image file, loaded non-system packages, unsaved changes, sessions
    Info,
    /// Save this instance's image and register a copy as a new instance (shares VM and sources file)
    Clone {
        /// Name of the new instance
        name: String,
        /// Start it right away
        #[arg(long)]
        start: bool,
    },
}

// ---------------------------------------------------------------- state handling

struct Instance {
    dir: PathBuf,
    name: String,
}

impl Instance {
    fn path(&self, ext: &str) -> PathBuf {
        self.dir.join(format!("{}.{}", self.name, ext))
    }
    fn port_info(&self) -> Option<Value> {
        let text = std::fs::read_to_string(self.path("port.json")).ok()?;
        serde_json::from_str(&text).ok()
    }
    fn pid(&self) -> Option<i32> {
        std::fs::read_to_string(self.path("pid")).ok()?.trim().parse().ok()
    }
    fn alive(&self) -> bool {
        self.pid().map(|p| unsafe { libc::kill(p, 0) == 0 }).unwrap_or(false)
    }
}

fn resolve_state_dir(explicit: &Option<PathBuf>, create: bool) -> PathBuf {
    if let Some(d) = explicit {
        return d.clone();
    }
    let mut cur = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        let candidate = cur.join(".stlive");
        if candidate.is_dir() {
            return candidate;
        }
        if !cur.pop() {
            break;
        }
    }
    let here = std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(".stlive");
    if create {
        let _ = std::fs::create_dir_all(&here);
    }
    here
}

// ---------------------------------------------------------------- output

static TEXT_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn emit(value: &Value, pretty: bool) {
    if TEXT_MODE.load(std::sync::atomic::Ordering::Relaxed) {
        let mut out = String::new();
        render_text(value, 0, &mut out);
        print!("{}", out);
        return;
    }
    let s = if pretty { serde_json::to_string_pretty(value) } else { serde_json::to_string(value) };
    println!("{}", s.unwrap());
}

fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::Null => Some("null".into()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

/// Indented key/value rendering for humans; multi-line strings are shown as blocks.
fn render_text(v: &Value, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match v {
        Value::Object(m) => {
            for (k, val) in m {
                match scalar(val) {
                    Some(s) if !s.contains('\n') => out.push_str(&format!("{}{}: {}\n", pad, k, s)),
                    Some(s) => {
                        out.push_str(&format!("{}{}: |\n", pad, k));
                        for l in s.lines() { out.push_str(&format!("{}  {}\n", pad, l)); }
                    }
                    None => {
                        out.push_str(&format!("{}{}:\n", pad, k));
                        render_text(val, indent + 1, out);
                    }
                }
            }
        }
        Value::Array(a) => {
            if a.iter().all(|x| scalar(x).is_some()) && !a.is_empty() {
                out.push_str(&format!("{}[{}]\n", pad, a.iter().filter_map(scalar).collect::<Vec<_>>().join(", ")));
            } else {
                for item in a {
                    match item {
                        Value::Object(_) | Value::Array(_) => {
                            let mut inner = String::new();
                            render_text(item, indent + 1, &mut inner);
                            let mut first = true;
                            for l in inner.lines() {
                                if first { out.push_str(&format!("{}- {}\n", pad, l.trim_start())); first = false; }
                                else { out.push_str(&format!("{}\n", l)); }
                            }
                        }
                        other => out.push_str(&format!("{}- {}\n", pad, scalar(other).unwrap_or_default())),
                    }
                }
            }
        }
        other => out.push_str(&format!("{}{}\n", pad, scalar(other).unwrap_or_default())),
    }
}

fn die(code: i32, err_code: &str, message: String, pretty: bool) -> ! {
    emit(&json!({"ok": false, "error": {"code": err_code, "message": message}}), pretty);
    std::process::exit(code)
}

// ---------------------------------------------------------------- transport

fn call(inst: &Instance, cmd: &str, args: Value, read_timeout: Duration, pretty: bool) -> Value {
    let info = match inst.port_info() {
        Some(i) => i,
        None => die(3, "not_running", format!("Instance '{}' is not running (no port file in {}). Start it with: stlive start", inst.name, inst.dir.display()), pretty),
    };
    let port = info["port"].as_u64().unwrap_or(0) as u16;
    let mut stream = match TcpStream::connect_timeout(&format!("127.0.0.1:{}", port).parse().unwrap(), Duration::from_secs(3)) {
        Ok(s) => s,
        Err(e) => die(3, "unreachable", format!("Cannot reach instance '{}' on 127.0.0.1:{} ({}). It may have exited; run: stlive start", inst.name, port, e), pretty),
    };
    let _ = stream.set_read_timeout(Some(read_timeout));
    let request = json!({"cmd": cmd, "args": args});
    if let Err(e) = stream.write_all(format!("{}\n", request).as_bytes()) {
        die(3, "connection_lost", format!("Write failed: {}", e), pretty);
    }
    let mut buf = Vec::new();
    match stream.read_to_end(&mut buf) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
            die(5, "timeout", format!("No response within {:?}. The image may be busy; the request may still be running.", read_timeout), pretty)
        }
        Err(e) => die(3, "connection_lost", format!("Connection lost while waiting for the response: {}", e), pretty),
    }
    if buf.is_empty() {
        die(4, "empty_response", "The image closed the connection without a response".into(), pretty);
    }
    match serde_json::from_slice(&buf) {
        Ok(v) => v,
        Err(e) => die(4, "bad_response", format!("Response is not valid JSON: {}", e), pretty),
    }
}

/// UTC timestamp like 2026-10-09T13:05:07.123Z (no external crate needed).
fn iso_now() -> String {
    let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let (secs, ms) = (d.as_secs() as i64, d.subsec_millis());
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z", year, month, day, rem / 3600, rem % 3600 / 60, rem % 60, ms)
}

/// One JSON line per call; metadata only (no arguments, no source code).
fn write_log(path: &Path, tag: Option<&str>, instance: &str, cmd: &str, dur: Duration, resp: &Value, exit: i32) {
    let r = &resp["result"];
    let e = &resp["error"];
    let pick = |keys: &[&str]| -> Value {
        for k in keys {
            for v in [&r[*k], &e[*k]] {
                if !v.is_null() { return v.clone(); }
            }
        }
        Value::Null
    };
    let line = json!({
        "ts": iso_now(),
        "tag": tag,
        "instance": instance,
        "command": cmd,
        "duration_ms": dur.as_millis() as u64,
        "output_chars": serde_json::to_string(resp).map(|s| s.chars().count()).unwrap_or(0),
        "ok": resp["ok"] == json!(true),
        "exit_code": exit,
        "class": if !r["value"]["class"].is_null() { r["value"]["class"].clone() } else { pick(&["class"]) },
        "selector": pick(&["selector"]),
        "session": pick(&["session"]),
        "mode": pick(&["mode"]),
        "error_code": if resp["ok"] == json!(true) { Value::Null } else { pick(&["code", "type"]) },
    });
    if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{}", line);
    }
}

fn exit_code_for(resp: &Value) -> i32 {
    if resp["ok"] != json!(true) {
        return 1;
    }
    // A test run with failures is a failed run for shell scripts.
    if resp["result"]["failed"].as_u64().unwrap_or(0) > 0 {
        return 1;
    }
    0
}

fn read_stdin() -> String {
    let mut s = String::new();
    let _ = std::io::stdin().read_to_string(&mut s);
    s
}

fn opt(map: &mut Map<String, Value>, key: &str, v: Option<impl Into<Value>>) {
    if let Some(v) = v {
        map.insert(key.into(), v.into());
    }
}

fn parse_kv(items: &[String]) -> Value {
    let mut m = Map::new();
    for kv in items {
        if let Some((k, v)) = kv.split_once('=') {
            m.insert(k.to_string(), serde_json::from_str(v).unwrap_or_else(|_| Value::String(v.to_string())));
        }
    }
    Value::Object(m)
}

// ---------------------------------------------------------------- start / stop

fn list_instances(inst: &Instance, pretty: bool) {
    let mut names: Vec<String> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&inst.dir) {
        for e in rd.flatten() {
            if let Some(n) = e.file_name().to_string_lossy().strip_suffix(".config.json").map(|s| s.to_string()) {
                names.push(n);
            }
        }
    }
    names.sort();
    let list: Vec<Value> = names
        .iter()
        .map(|n| {
            let i = Instance { dir: inst.dir.clone(), name: n.clone() };
            let info = i.port_info();
            json!({"name": n, "running": i.alive() && info.is_some(), "port": info.as_ref().map(|x| x["port"].clone()), "pid": i.pid()})
        })
        .collect();
    emit(&json!({"ok": true, "result": {"state_dir": inst.dir, "instances": list}}), pretty);
}

fn start(inst: &Instance, vm: Option<PathBuf>, image: Option<PathBuf>, pretty: bool) {
    if inst.alive() && inst.port_info().is_some() {
        let r = call(inst, "ping", json!({}), Duration::from_secs(3), pretty);
        if r["ok"] == json!(true) {
            emit(&json!({"ok": true, "result": {"already_running": true, "port": inst.port_info().unwrap()["port"]}}), pretty);
            return;
        }
    }
    let cfg_path = inst.path("config.json");
    let saved: Value = std::fs::read_to_string(&cfg_path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(json!({}));
    let vm = vm.or_else(|| saved["vm"].as_str().map(PathBuf::from));
    let image = image.or_else(|| saved["image"].as_str().map(PathBuf::from));
    let (vm, image) = match (vm, image) {
        (Some(v), Some(i)) => (v, i),
        (v, i) => match init::template(&init::home_dir()) {
            Some((tvm, timg)) => {
                let img = match i {
                    Some(i) => i,
                    None => init::instance_image(&timg, &inst.dir, &inst.name).unwrap_or_else(|e| die(3, "image_copy_failed", e, pretty)),
                };
                (v.unwrap_or(tvm), img)
            }
            None => die(2, "not_initialized", "No Pharo set up yet. Run `stlive init` once (downloads Pharo 13 and builds the image), or pass --vm and --image.".into(), pretty),
        },
    };
    let abs = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let (vm, image) = (abs(&vm), abs(&image));
    let _ = std::fs::create_dir_all(&inst.dir);
    let _ = std::fs::write(&cfg_path, json!({"vm": vm, "image": image}).to_string());
    let serve = inst.path("serve.st");
    let _ = std::fs::write(&serve, SERVE_ST);
    let _ = std::fs::remove_file(inst.path("port.json"));
    let log = std::fs::File::create(inst.path("log")).expect("cannot create log file");
    use std::os::unix::process::CommandExt;
    let child = Command::new(&vm)
        .arg("--headless")
        .arg(&image)
        .arg("st")
        .arg(&serve)
        .arg("--no-quit")
        .env("STLIVE_PORTFILE", inst.path("port.json"))
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .process_group(0)
        .spawn();
    let child = match child {
        Ok(c) => c,
        Err(e) => die(3, "spawn_failed", format!("Cannot start {}: {}", vm.display(), e), pretty),
    };
    let _ = std::fs::write(inst.path("pid"), child.id().to_string());
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        if let Some(info) = inst.port_info() {
            emit(&json!({"ok": true, "result": {"started": true, "pid": child.id(), "port": info["port"], "epoch": info["epoch"], "pharo": info["pharo"], "log": inst.path("log")}}), pretty);
            return;
        }
        if !inst.alive() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    die(3, "start_failed", format!("Image did not publish a port in time; see {}", inst.path("log").display()), pretty);
}

fn stop(inst: &Instance, force: bool, pretty: bool) {
    if !force {
        die(2, "confirmation_required", "Stopping discards all unsaved image state and debug sessions. Re-run with --force (use `stlive image save` first to keep state).".into(), pretty);
    }
    let _ = call(inst, "image.stop", json!({"force": true}), Duration::from_secs(5), pretty);
    let deadline = Instant::now() + Duration::from_secs(10);
    while inst.alive() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    if inst.alive() {
        if let Some(p) = inst.pid() {
            unsafe { libc::kill(p, libc::SIGKILL) };
        }
    }
    let _ = std::fs::remove_file(inst.path("port.json"));
    let _ = std::fs::remove_file(inst.path("pid"));
    emit(&json!({"ok": true, "result": {"stopped": true}}), pretty);
}

// ---------------------------------------------------------------- save / clone / diff

fn add_diff(resp: &mut Value) {
    let r = &mut resp["result"];
    if let (Some(old), Some(new)) = (r["old_source"].as_str().map(str::to_string), r["new_source"].as_str().map(str::to_string)) {
        let norm = |s: &str| s.replace("\r\n", "\n").replace('\r', "\n");
        let (o, n) = (norm(&old), norm(&new));
        let diff = similar::TextDiff::from_lines(&o, &n);
        r["diff"] = Value::String(diff.unified_diff().context_radius(2).header("before", "after").to_string());
    } else if r["old_source"].is_null() && r["new_source"].is_string() {
        r["diff"] = Value::String("(new method)".into());
    }
}

fn save_changes(inst: &Instance, package: Option<String>, all: bool, dir: &Path, dry_run: bool, pretty: bool) {
    if package.is_none() && !all {
        die(2, "missing_argument", "Give --package <Name> or --all. `stlive changes list` shows unsaved_by_package.".into(), pretty);
    }
    let args = match &package { Some(p) if !all => json!({"package": p}), _ => json!({}) };
    let resp = call(inst, "changes.pending", args, Duration::from_secs(30), pretty);
    if resp["ok"] != json!(true) {
        emit(&resp, pretty);
        std::process::exit(1);
    }
    let changes: Vec<Value> = resp["result"]["changes"].as_array().cloned().unwrap_or_default();
    if changes.is_empty() {
        emit(&json!({"ok": true, "result": {"files": [], "note": "No unsaved changes"}}), pretty);
        return;
    }
    let mut by_pkg: std::collections::BTreeMap<String, Vec<Value>> = Default::default();
    let mut skipped = Vec::new();
    for c in changes {
        match c["package"].as_str() {
            Some(p) => by_pkg.entry(p.to_string()).or_default().push(c),
            None => skipped.push(c["index"].clone()),
        }
    }
    let mut files = Vec::new();
    let mut marked: Vec<u64> = Vec::new();
    for (pkg, list) in &by_pkg {
        match save::plan(list, pkg, dir, dry_run) {
            Ok(plan) => {
                files.extend(plan.report);
                marked.extend(plan.marked);
            }
            Err(e) => die(1, "save_failed", format!("package {}: {}", pkg, e), pretty),
        }
    }
    if !dry_run {
        let _ = call(inst, "changes.mark_saved", json!({"indexes": marked}), Duration::from_secs(30), pretty);
    }
    emit(&json!({"ok": true, "result": {"dry_run": dry_run, "packages": by_pkg.keys().collect::<Vec<_>>(), "changes": marked.len(), "files": files, "skipped_without_package": skipped}}), pretty);
}

fn clone_image(inst: &Instance, name: &str, start_it: bool, pretty: bool) {
    let cfg: Value = std::fs::read_to_string(inst.path("config.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(json!({}));
    let (vm, image) = match (cfg["vm"].as_str(), cfg["image"].as_str()) {
        (Some(v), Some(i)) => (PathBuf::from(v), PathBuf::from(i)),
        _ => die(2, "missing_config", format!("Instance '{}' has no recorded VM/image; start it first", inst.name), pretty),
    };
    let saved = call(inst, "image.save", json!({}), Duration::from_secs(120), pretty);
    if saved["ok"] != json!(true) {
        emit(&saved, pretty);
        std::process::exit(1);
    }
    let dir = image.parent().unwrap().to_path_buf();
    let new_image = dir.join(format!("{}.image", name));
    let new_changes = dir.join(format!("{}.changes", name));
    if new_image.exists() {
        die(1, "exists", format!("{} already exists", new_image.display()), pretty);
    }
    if let Err(e) = std::fs::copy(&image, &new_image).and_then(|_| std::fs::copy(image.with_extension("changes"), &new_changes)) {
        die(1, "copy_failed", e.to_string(), pretty);
    }
    let clone_inst = Instance { dir: inst.dir.clone(), name: name.to_string() };
    let _ = std::fs::write(clone_inst.path("config.json"), json!({"vm": vm, "image": new_image}).to_string());
    if start_it {
        return start(&clone_inst, Some(vm), Some(new_image), pretty);
    }
    emit(&json!({"ok": true, "result": {"instance": name, "image": new_image, "start": format!("stlive -i {} start", name)}}), pretty);
}

// ---------------------------------------------------------------- main

fn main() {
    let cli = Cli::parse();
    let pretty = cli.pretty;
    TEXT_MODE.store(cli.text, std::sync::atomic::Ordering::Relaxed);
    let creating = matches!(cli.cmd, Cmd::Start { .. });
    let inst = Instance { dir: resolve_state_dir(&cli.state_dir, creating), name: cli.instance.clone() };

    let (cmd, args, timeout_ms): (String, Value, Option<u64>) = match cli.cmd {
        Cmd::Init { force } => {
            let home = init::home_dir();
            match init::init(&home, force) {
                Ok(r) => return emit(&json!({"ok": true, "result": r}), pretty),
                Err(e) => die(1, "init_failed", e, pretty),
            }
        }
        Cmd::Instances => return list_instances(&inst, pretty),
        Cmd::Start { vm, image } => return start(&inst, vm, image, pretty),
        Cmd::Stop { force } => return stop(&inst, force, pretty),
        Cmd::Status => ("status".into(), json!({}), None),
        Cmd::Ping => ("ping".into(), json!({}), None),
        Cmd::Eval { code, receiver, timeout, full, no_session } => {
            let code = if code == "-" { read_stdin() } else { code };
            let mut m = Map::new();
            m.insert("code".into(), code.into());
            if full { m.insert("full".into(), true.into()); }
            if no_session { m.insert("no_session".into(), true.into()); }
            opt(&mut m, "in", receiver);
            opt(&mut m, "timeout", timeout);
            ("eval".into(), Value::Object(m), timeout)
        }
        Cmd::Obj(o) => match o {
            ObjCmd::Show { r#ref, offset, limit, raw } => {
                let mut m = Map::new();
                m.insert("ref".into(), r#ref.into());
                opt(&mut m, "offset", offset);
                opt(&mut m, "limit", limit);
                if raw { m.insert("raw".into(), true.into()); }
                ("obj.show".into(), Value::Object(m), None)
            }
            ObjCmd::Items { r#ref, offset, limit } => {
                let mut m = Map::new();
                m.insert("ref".into(), r#ref.into());
                opt(&mut m, "offset", offset);
                opt(&mut m, "limit", limit);
                ("obj.items".into(), Value::Object(m), None)
            }
            ObjCmd::Text { r#ref, offset, limit } => {
                let mut m = Map::new();
                m.insert("ref".into(), r#ref.into());
                opt(&mut m, "offset", offset);
                opt(&mut m, "limit", limit);
                ("obj.text".into(), Value::Object(m), None)
            }
            ObjCmd::Var { r#ref, name } => ("obj.var".into(), json!({"ref": r#ref, "name": name}), None),
            ObjCmd::Compare { a, b } => ("obj.compare".into(), json!({"a": a, "b": b}), None),
            ObjCmd::Referrers { r#ref, limit } => {
                let mut m = Map::new();
                m.insert("ref".into(), r#ref.into());
                opt(&mut m, "limit", limit);
                ("obj.referrers".into(), Value::Object(m), None)
            }
            ObjCmd::Graph { r#ref, depth, max_nodes } => {
                let mut m = Map::new();
                m.insert("ref".into(), r#ref.into());
                opt(&mut m, "depth", depth);
                opt(&mut m, "max_nodes", max_nodes);
                ("obj.graph".into(), Value::Object(m), None)
            }
            ObjCmd::Release { r#ref, all } => {
                let mut m = Map::new();
                opt(&mut m, "ref", r#ref);
                if all { m.insert("all".into(), true.into()); }
                ("obj.release".into(), Value::Object(m), None)
            }
        },
        Cmd::Find(f) => {
            let (name, key, v, limit) = match f {
                FindCmd::Class { pattern, limit } => ("find.class", "pattern", pattern, limit),
                FindCmd::Package { pattern, limit } => ("find.package", "pattern", pattern, limit),
                FindCmd::Implementors { selector, limit } => ("find.implementors", "selector", selector, limit),
                FindCmd::Senders { selector, limit } => ("find.senders", "selector", selector, limit),
            };
            let mut m = Map::new();
            m.insert(key.into(), v.into());
            opt(&mut m, "limit", limit);
            (name.into(), Value::Object(m), None)
        }
        Cmd::Class(c) => match c {
            ClassCmd::Show { name, offset, limit } => {
                let mut m = Map::new();
                m.insert("name".into(), name.into());
                opt(&mut m, "offset", offset);
                opt(&mut m, "limit", limit);
                ("class.show".into(), Value::Object(m), None)
            }
            ClassCmd::Create { name, superclass, ivars, package, class_ivars, class_vars, comment } => {
                let mut m = Map::new();
                m.insert("name".into(), name.into());
                opt(&mut m, "class_ivars", class_ivars);
                opt(&mut m, "class_vars", class_vars);
                opt(&mut m, "comment", comment);
                opt(&mut m, "superclass", superclass);
                opt(&mut m, "ivars", ivars);
                opt(&mut m, "package", package);
                ("class.create".into(), Value::Object(m), None)
            }
        },
        Cmd::Method(mc) => match mc {
            MethodCmd::Show { target } => ("method.show".into(), json!({"target": target}), None),
            MethodCmd::Compile { class, source_arg, source, file, protocol } => {
                let source = source.or(source_arg);
                let source = match (source, file) {
                    (Some(s), _) if s != "-" => s,
                    (Some(_), _) => read_stdin(),
                    (None, Some(f)) if f.as_os_str() == "-" => read_stdin(),
                    (None, Some(f)) => std::fs::read_to_string(&f).unwrap_or_else(|e| die(2, "bad_file", format!("{}: {}", f.display(), e), pretty)),
                    (None, None) => read_stdin(),
                };
                let mut m = Map::new();
                m.insert("class".into(), class.into());
                m.insert("source".into(), source.into());
                opt(&mut m, "protocol", protocol);
                ("method.compile".into(), Value::Object(m), None)
            }
            MethodCmd::Remove { class, selector, force } => ("method.remove".into(), json!({"class": class, "selector": selector, "force": force}), None),
        },
        Cmd::Changes(c) => match c {
            ChangesCmd::List { offset, limit } => {
                let mut m = Map::new();
                opt(&mut m, "offset", offset);
                opt(&mut m, "limit", limit);
                ("changes.list".into(), Value::Object(m), None)
            }
            ChangesCmd::Show { index } => ("changes.show".into(), json!({"index": index}), None),
        },
        Cmd::Test(TestCmd::List { pattern, limit }) => {
            let mut m = Map::new();
            opt(&mut m, "pattern", pattern);
            opt(&mut m, "limit", limit);
            ("test.list".into(), Value::Object(m), None)
        }
        Cmd::Package(PackageCmd::List { pattern, limit }) => {
            let mut m = Map::new();
            m.insert("pattern".into(), pattern.unwrap_or_else(|| "*".into()).into());
            opt(&mut m, "limit", limit);
            ("find.package".into(), Value::Object(m), None)
        }
        Cmd::Test(TestCmd::Run { target, timeout, keep_sessions, discard_sessions }) => {
            let mut m = Map::new();
            m.insert("target".into(), target.into());
            opt(&mut m, "timeout", timeout);
            if keep_sessions { m.insert("keep_sessions".into(), true.into()); }
            if discard_sessions { m.insert("discard_sessions".into(), true.into()); }
            // the timeout is per test; allow a generous overall budget for the connection
            ("test.run".into(), Value::Object(m), Some(timeout.unwrap_or(10_000) * 20))
        }
        Cmd::Debug(d) => match d {
            DebugCmd::List { all } => ("debug.list".into(), json!({"all": all}), None),
            DebugCmd::Inspect { session } => ("debug.inspect".into(), json!({"session": session}), None),
            DebugCmd::Frames { session, offset, limit } => {
                let mut m = Map::new();
                m.insert("session".into(), session.into());
                opt(&mut m, "offset", offset);
                opt(&mut m, "limit", limit);
                ("debug.frames".into(), Value::Object(m), None)
            }
            DebugCmd::Frame { session, index, frame, no_source } => ("debug.frame".into(), json!({"session": session, "frame": index.or(frame).unwrap_or(0), "no_source": no_source}), None),
            DebugCmd::Locals { session, frame_pos, frame } => ("debug.locals".into(), json!({"session": session, "frame": frame_pos.or(frame).unwrap_or(0)}), None),
            DebugCmd::Receiver { session, frame_pos, frame } => ("debug.receiver".into(), json!({"session": session, "frame": frame_pos.or(frame).unwrap_or(0)}), None),
            DebugCmd::Eval { session, code, frame, timeout, full, no_session } => {
                let code = if code == "-" { read_stdin() } else { code };
                let mut m = Map::new();
                m.insert("session".into(), session.into());
                m.insert("code".into(), code.into());
                m.insert("frame".into(), frame.into());
                if full { m.insert("full".into(), true.into()); }
                if no_session { m.insert("no_session".into(), true.into()); }
                opt(&mut m, "timeout", timeout);
                ("debug.eval".into(), Value::Object(m), timeout)
            }
            DebugCmd::Resume { session, value, timeout } => {
                let mut m = Map::new();
                m.insert("session".into(), session.into());
                opt(&mut m, "value", value);
                opt(&mut m, "timeout", timeout);
                ("debug.resume".into(), Value::Object(m), timeout)
            }
            DebugCmd::Restart { session, frame_pos, frame, timeout } => {
                let mut m = Map::new();
                m.insert("session".into(), session.into());
                m.insert("frame".into(), frame_pos.or(frame).unwrap_or(0).into());
                opt(&mut m, "timeout", timeout);
                ("debug.restart".into(), Value::Object(m), timeout)
            }
            DebugCmd::Return { session, value, frame_pos, frame, timeout } => {
                let mut m = Map::new();
                m.insert("session".into(), session.into());
                m.insert("value".into(), value.into());
                m.insert("frame".into(), frame_pos.or(frame).unwrap_or(0).into());
                opt(&mut m, "timeout", timeout);
                ("debug.return".into(), Value::Object(m), timeout)
            }
            DebugCmd::Rerun { session } => ("debug.rerun".into(), json!({"session": session}), None),
            DebugCmd::Terminate { session, all } => {
                let mut m = Map::new();
                opt(&mut m, "session", session);
                if all { m.insert("all".into(), true.into()); }
                ("debug.terminate".into(), Value::Object(m), None)
            }
        },
        Cmd::Image(ImageCmd::Save) => ("image.save".into(), json!({}), None),
        Cmd::Image(ImageCmd::Info) => ("image.info".into(), json!({}), None),
        Cmd::Image(ImageCmd::Clone { name, start: do_start }) => return clone_image(&inst, &name, do_start, pretty),
        Cmd::Save { package, all, dir, dry_run } => return save_changes(&inst, package, all, &dir, dry_run, pretty),
        Cmd::Load { baseline, repository, groups, timeout } => {
            let mut m = Map::new();
            m.insert("name".into(), baseline.into());
            m.insert("repository".into(), repository.into());
            m.insert("groups".into(), json!(groups));
            opt(&mut m, "timeout", timeout);
            ("load.baseline".into(), Value::Object(m), Some(timeout.unwrap_or(600_000)))
        }
        Cmd::Raw { cmd, args } => (cmd, parse_kv(&args), None),
    };

    // The image enforces the evaluation timeout (default 30 s) and answers with a debug session;
    // the socket timeout only guards against a wedged image.
    let read_timeout = Duration::from_millis(timeout_ms.unwrap_or(30_000) + 15_000);
    let started = Instant::now();
    let mut resp = call(&inst, &cmd, args, read_timeout, pretty);
    if cmd == "changes.show" {
        add_diff(&mut resp);
    }
    emit(&resp, pretty);
    let code = exit_code_for(&resp);
    if let Some(path) = &cli.log {
        write_log(path, cli.tag.as_deref(), &inst.name, &cmd, started.elapsed(), &resp, code);
    }
    std::process::exit(code);
}
