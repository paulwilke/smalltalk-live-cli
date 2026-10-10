//! stlive – thin client for a persistent Pharo image running the StLive server.
//!
//! One request line (JSON) per connection, one response line back. All state (objects,
//! debug sessions) lives in the image, so every invocation is short-lived.
//!
//! Exit codes: 0 ok · 1 command failed / exception in image / failing tests · 2 usage
//!             3 image not running or unreachable · 4 protocol error · 5 timeout

mod init;
mod platform;
mod save;

use clap::{Parser, Subcommand};
use serde_json::{json, Map, Value};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};


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
        /// Use this local Pharo VM instead of downloading (needs --image)
        #[arg(long, requires = "image")]
        vm: Option<PathBuf>,
        /// Use this pristine Pharo image as the base instead of downloading (needs --vm)
        #[arg(long, requires = "vm")]
        image: Option<PathBuf>,
        /// Pharo major version to download and build (default 13; 14 for Spec-Gtk)
        #[arg(long, default_value_t = 13)]
        pharo: u32,
    },
    /// Load the StLive server into an image that is ALREADY running (another stlive/pharoctl server on --port)
    Attach {
        /// Port of the running server (its state file: <dir>/<instance>.port.json)
        #[arg(long)]
        port: u16,
        /// Process id of that image (found automatically if omitted (lsof or netstat))
        #[arg(long)]
        pid: Option<i32>,
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
        /// Start Pharo WITH its window (Morphic/Spec/Bloc development) instead of headless; everything else is the same
        #[arg(long)]
        gui: bool,
        /// Pharo 14/Spec-Gtk style: start with `--worker --headless` (the VM stays alive on its own, no `--no-quit`)
        #[arg(long, conflicts_with = "gui")]
        gtk: bool,
        /// Extra VM option placed before the image (repeatable), e.g. --vm-arg=--worker
        #[arg(long = "vm-arg", allow_hyphen_values = true)]
        vm_args: Vec<String>,
        /// Use the template made by `init --pharo N` (default: the Pharo 13 one or what this instance already uses)
        #[arg(long)]
        pharo: Option<u32>,
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
        /// Leave the statement text out of error frames
        #[arg(long)]
        no_source: bool,
        /// Run in the GUI image's UI process (safe for Spec/Morphic changes; needs `start --gui`)
        #[arg(long)]
        ui: bool,
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
    /// Windows of a GUI image (start it with `start --gui`): list, screenshot, press buttons
    #[command(subcommand)]
    Ui(UiCmd),
    /// Open a URL in a chromeless app window (Chrome/Edge --app), e.g. the web UI of your application
    OpenUi {
        url: String,
        /// Browser executable or app name (default: Chrome on macOS, Edge on Windows, first Chrome/Chromium found on Linux)
        #[arg(long)]
        browser: Option<String>,
    },
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
        /// After writing, compare image and files and fail if they still differ
        #[arg(long)]
        verify: bool,
    },
    /// One-shot headless run of a program from Tonel sources in a FRESH image; stdout carries only the program's result
    Run {
        /// Tonel source directory to load (one folder per package; repeatable)
        #[arg(long)]
        load: Vec<PathBuf>,
        /// Only these packages, in this order (default: every package folder)
        #[arg(long = "package")]
        packages: Vec<String>,
        /// Smalltalk file evaluated after loading (e.g. to load a Metacello dependency)
        #[arg(long)]
        prepare: Option<PathBuf>,
        /// Expression to evaluate; its value is printed (Strings raw, others printString)
        #[arg(long)]
        eval: Option<String>,
        /// Smalltalk file to evaluate instead of --eval
        #[arg(long)]
        file: Option<PathBuf>,
        /// Seconds before the run is killed (exit code 124)
        #[arg(long, default_value_t = 300)]
        timeout: u64,
        /// Keep the work directory (image, logs) for inspection
        #[arg(long)]
        keep: bool,
        /// Arguments for the program: read them with `StLiveRun arguments`
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Compare the image with the Tonel files of a package: methods or classes that exist only on one side, or differ
    Drift {
        /// Package to compare (or --all for every package folder in --dir that is loaded in the image)
        #[arg(long)]
        package: Option<String>,
        #[arg(long)]
        all: bool,
        /// Source directory with one folder per package
        #[arg(long)]
        dir: PathBuf,
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
        /// Compile in the UI process (open Spec/Morphic windows are rebuilt safely; needs `start --gui`)
        #[arg(long)] ui: bool,
    },
    /// Remove a method (destructive)
    Remove { class: String, selector: String, #[arg(long)] force: bool },
}

#[derive(Subcommand)]
enum ChangesCmd {
    /// Recorded changes (also those made by eval or the IDE); --since N lists entries after index N
    List { #[arg(long)] offset: Option<u64>, #[arg(long)] limit: Option<u64>, #[arg(long)] since: Option<u64> },
    /// Stream new changes as JSON lines until interrupted (polls every --interval ms)
    Watch { #[arg(long, default_value_t = 500)] interval: u64 },
    Show { index: u64 },
}

#[derive(Subcommand)]
enum UiCmd {
    /// Open windows with title, presenter class, bounds and an object reference
    Windows,
    /// PNG of the whole world or one window (index from `ui windows` or part of the title)
    Screenshot {
        #[arg(long)]
        window: Option<String>,
        /// Output file (default: <state dir>/screenshots/<instance>-<time>.png)
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Press a button by its label (or the presenter variable name) inside the UI process
    Press {
        target: String,
        #[arg(long)]
        window: Option<String>,
        #[arg(long)]
        timeout: Option<u64>,
    },
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
    Eval { session: String, code: String, #[arg(long, default_value_t = 0)] frame: u64, #[arg(long)] timeout: Option<u64>, #[arg(long)] full: bool, #[arg(long)] no_session: bool, #[arg(long)] ui: bool },
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
    /// Save a copy of the image for delivery WITHOUT the stlive server (no open evaluation port), then stop this instance
    Export {
        /// Target image file, e.g. dist/MyApp.image (.changes and the sources file are written/copied next to it)
        out: PathBuf,
        /// Keep the stlive server in the exported image
        #[arg(long)]
        keep_server: bool,
        /// Required: the instance ends after exporting
        #[arg(long)]
        force: bool,
    },
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
        self.pid().map(platform::pid_alive).unwrap_or(false)
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

/// Talk to an instance; `Err((exit code, error code, message))` instead of exiting.
fn try_call(inst: &Instance, cmd: &str, args: Value, read_timeout: Duration) -> Result<Value, (i32, &'static str, String)> {
    let info = inst.port_info().ok_or_else(|| (3, "not_running", format!("Instance '{}' is not running (no port file in {}). Start it with: stlive start", inst.name, inst.dir.display())))?;
    let port = info["port"].as_u64().unwrap_or(0) as u16;
    let mut stream = TcpStream::connect_timeout(&format!("127.0.0.1:{}", port).parse().unwrap(), Duration::from_secs(3))
        .map_err(|e| (3, "unreachable", format!("Cannot reach instance '{}' on 127.0.0.1:{} ({}). It may have exited; run: stlive start", inst.name, port, e)))?;
    let _ = stream.set_read_timeout(Some(read_timeout));
    let request = json!({"cmd": cmd, "args": args});
    stream.write_all(format!("{}\n", request).as_bytes()).map_err(|e| (3, "connection_lost", format!("Write failed: {}", e)))?;
    let mut buf = Vec::new();
    match stream.read_to_end(&mut buf) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
            return Err((5, "timeout", format!("No response within {:?}. The image may be busy; the request may still be running.", read_timeout)))
        }
        Err(e) => return Err((3, "connection_lost", format!("Connection lost while waiting for the response: {}", e))),
    }
    if buf.is_empty() {
        return Err((4, "empty_response", "The image closed the connection without a response".into()));
    }
    serde_json::from_slice(&buf).map_err(|e| (4, "bad_response", format!("Response is not valid JSON: {}", e)))
}

fn call(inst: &Instance, cmd: &str, args: Value, read_timeout: Duration, pretty: bool) -> Value {
    match try_call(inst, cmd, args, read_timeout) {
        Ok(v) => v,
        Err((code, err, msg)) => die(code, err, msg, pretty),
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

/// A short, single-line description of a call for the log (first 60 characters of code / the main argument).
fn summarize(cmd: &str, args: &Value) -> String {
    let pick = ["code", "target", "name", "class", "session", "ref", "pattern", "selector", "package"]
        .iter()
        .find_map(|k| args[*k].as_str().map(|v| (*k, v.to_string())));
    let text = match (cmd, pick) {
        ("method.compile", _) => {
            let first = args["source"].as_str().unwrap_or("").lines().next().unwrap_or("").trim().to_string();
            format!("{} >> {}", args["class"].as_str().unwrap_or("?"), first)
        }
        (_, Some((_, v))) => v,
        _ => String::new(),
    };
    let one_line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() > 60 { format!("{}...", one_line.chars().take(60).collect::<String>()) } else { one_line }
}

/// One JSON line per call: metadata and a 60-character summary of the call (no full code).
fn write_log(path: &Path, tag: Option<&str>, instance: &str, cmd: &str, summary: &str, dur: Duration, resp: &Value, exit: i32) {
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
        "summary": summary,
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
    let mut line = line;
    if cmd == "test.run" && resp["ok"] == json!(true) {
        let groups = r["groups"].as_array().cloned().unwrap_or_default();
        line["ran"] = r["ran"].clone();
        line["passed"] = r["passed"].clone();
        line["failed"] = r["failed"].clone();
        line["sessions"] = json!(groups.iter().flat_map(|g| g["sessions"].as_array().cloned().unwrap_or_default()).collect::<Vec<_>>());
        line["groups"] = json!(groups.iter().map(|g| json!({"count": g["count"], "type": g["type"], "message": g["message"].as_str().map(|m| m.chars().take(60).collect::<String>()), "frame": g["first_application_frame"]["label"]})).collect::<Vec<_>>());
    }
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

fn start(inst: &Instance, vm: Option<PathBuf>, image: Option<PathBuf>, mode: Option<&str>, vm_args: Vec<String>, pharo: Option<u32>, pretty: bool) {
    if inst.alive() && inst.port_info().is_some() {
        let r = call(inst, "ping", json!({}), Duration::from_secs(3), pretty);
        if r["ok"] == json!(true) {
            emit(&json!({"ok": true, "result": {"already_running": true, "port": inst.port_info().unwrap()["port"]}}), pretty);
            return;
        }
    }
    let cfg_path = inst.path("config.json");
    let saved: Value = std::fs::read_to_string(&cfg_path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(json!({}));
    // --pharo N picks that version's template and ignores what this instance remembered.
    let pharo_explicit = pharo.is_some();
    let pharo = pharo.or_else(|| saved["pharo"].as_u64().map(|v| v as u32));
    let version = pharo.unwrap_or(init::DEFAULT_PHARO);
    let vm = vm.or_else(|| if pharo_explicit { None } else { saved["vm"].as_str().map(PathBuf::from) });
    let image = image.or_else(|| if pharo_explicit { None } else { saved["image"].as_str().map(PathBuf::from) });
    // Pharo 14+ cannot be kept alive with `st … --no-quit`; its worker mode (`--worker --headless`) is the way.
    let mode = mode.map(String::from).or_else(|| saved["mode"].as_str().map(String::from)).unwrap_or_else(|| if version >= 14 { "worker".into() } else { "headless".into() });
    let mode = mode.as_str();
    let image_name = if version == init::DEFAULT_PHARO { inst.name.clone() } else { format!("{}.p{}", inst.name, version) };
    let (vm, image) = match (vm, image) {
        (Some(v), Some(i)) => (v, i),
        (v, i) => match init::template(&init::home_dir(), version) {
            Some((tvm, timg)) => {
                let img = match i {
                    Some(i) => i,
                    None => init::instance_image(&timg, &inst.dir, &image_name).unwrap_or_else(|e| die(3, "image_copy_failed", e, pretty)),
                };
                (v.unwrap_or(tvm), img)
            }
            None => die(2, "not_initialized", "No Pharo set up yet. Run `stlive init` once (downloads Pharo and builds the image; `--pharo 14` for another version), or pass --vm and --image.".into(), pretty),
        },
    };
    // A deleted instance image is rebuilt from the template.
    if !image.exists() && image.parent() == Some(inst.dir.as_path()) {
        if let Some((_, timg)) = init::template(&init::home_dir(), version) {
            let _ = init::instance_image(&timg, &inst.dir, &image_name);
        }
    }
    let abs = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let (vm, image) = (abs(&vm), abs(&image));
    let _ = std::fs::create_dir_all(&inst.dir);
    // The state directory (images, logs, Pharo's pharo-local/ombu-sessions) never belongs in version control.
    if !inst.dir.join(".gitignore").exists() {
        let _ = std::fs::write(inst.dir.join(".gitignore"), "# created by stlive: local state, never commit\n*\n");
    }
    let _ = std::fs::write(&cfg_path, json!({"vm": vm, "image": image, "pharo": version, "mode": mode}).to_string());
    let serve = inst.path("serve.st");
    let _ = std::fs::write(&serve, init::serve_st());
    let src_dir = init::write_sources(&init::home_dir()).ok();
    let _ = std::fs::remove_file(inst.path("port.json"));
    let log = std::fs::File::create(inst.path("log")).expect("cannot create log file");
    let mut cmd = Command::new(&vm);
    // headless: --headless … --no-quit | gui: window, same script | worker (Pharo 14 Spec-Gtk): --worker --headless, the worker keeps the VM alive
    let mut mode_args: Vec<String> = match mode {
        "gui" => vec![],
        "worker" => vec!["--worker".into(), "--headless".into()],
        _ => vec!["--headless".into()],
    };
    mode_args.extend(vm_args);
    cmd
        .args(&mode_args)
        .arg(&image)
        .arg("st")
        .arg(&serve)
    ;
    if mode != "worker" {
        cmd.arg("--no-quit");
    }
    cmd
        .env("STLIVE_PORTFILE", inst.path("port.json"))
        .env("STLIVE_SOURCES_HASH", init::sources_hash())
        .envs(src_dir.iter().map(|d| ("STLIVE_SRC", d.clone())))
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
    ;
    platform::detach(&mut cmd);
    let child = cmd.spawn();
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
    let _ = try_call(inst, "image.stop", json!({"force": true}), Duration::from_secs(5)); // a dying image may not answer; fall through to the kill below
    let deadline = Instant::now() + Duration::from_secs(10);
    while inst.alive() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    if inst.alive() {
        if let Some(p) = inst.pid() {
            platform::kill_pid(p);
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


fn watch_changes(inst: &Instance, interval: u64, pretty: bool) {
    let mut seen: u64 = 0;
    loop {
        let r = call(inst, "changes.list", json!({"since": seen, "limit": 200}), Duration::from_secs(30), pretty);
        if r["ok"] != json!(true) {
            emit(&r, pretty);
            std::process::exit(1);
        }
        for c in r["result"]["changes"].as_array().cloned().unwrap_or_default() {
            seen = seen.max(c["index"].as_u64().unwrap_or(seen));
            emit(&c, false);
        }
        use std::io::Write as _;
        let _ = std::io::stdout().flush();
        std::thread::sleep(Duration::from_millis(interval));
    }
}

/// Load the StLive server into a running image through its existing JSON-line server.
fn attach(inst: &Instance, old_port: u16, pid: Option<i32>, pretty: bool) {
    let home = init::home_dir();
    let packages = match init::system_packages(&home) {
        Some(p) => p,
        None => die(2, "not_initialized", "attach needs the framework package list written by `stlive init` (run it once; it can use your local Pharo with --vm/--image)".into(), pretty),
    };
    let src = match init::write_sources(&home) {
        Ok(s) => s,
        Err(e) => die(1, "write_failed", e, pretty),
    };
    let portfile = inst.path("port.json");
    let _ = std::fs::create_dir_all(&inst.dir);
    let list = packages.iter().map(|p| format!("'{}'", p)).collect::<Vec<_>>().join(" ");
    let code = format!(
        "Metacello new baseline: 'StLive'; repository: 'tonel://{src}'; load.\n\
         (Smalltalk globals at: #StLiveServer) recordSystemPackagesFrom: #({list}).\n\
         (Smalltalk globals at: #StLiveServer) installedHash: '{hash}'.\n\
         (Smalltalk globals at: #StLiveServer) current startPortFile: '{pf}'.",
        src = src.display(),
        list = list,
        hash = init::sources_hash(),
        pf = portfile.display()
    );
    let tmp = Instance { dir: inst.dir.clone(), name: "attach-old".into() };
    let _ = std::fs::write(tmp.path("port.json"), json!({"port": old_port}).to_string());
    let r = call(&tmp, "eval", json!({"code": code, "timeout": 600000}), Duration::from_secs(660), pretty);
    let _ = std::fs::remove_file(tmp.path("port.json"));
    if r["ok"] != json!(true) {
        emit(&r, pretty);
        std::process::exit(1);
    }
    let pid = pid.or_else(|| {
        platform::pid_listening_on(old_port)
    });
    if let Some(p) = pid {
        let _ = std::fs::write(inst.path("pid"), p.to_string());
    }
    let info = inst.port_info().unwrap_or(json!({}));
    emit(&json!({"ok": true, "result": {"attached": true, "instance": inst.name, "port": info["port"], "epoch": info["epoch"], "pid": pid, "note": "The old server keeps running in the same image on its own port; use `stlive` from now on."}}), pretty);
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
        return start(&clone_inst, Some(vm), Some(new_image), None, vec![], None, pretty);
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
        Cmd::Attach { port, pid } => return attach(&inst, port, pid, pretty),
        Cmd::OpenUi { url, browser } => return open_ui(&url, browser, pretty),
        Cmd::Ui(u) => match u {
            UiCmd::Windows => ("ui.windows".into(), json!({}), None),
            UiCmd::Screenshot { window, out } => {
                let path = out.unwrap_or_else(|| {
                    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
                    inst.dir.join("screenshots").join(format!("{}-{}.png", inst.name, ms))
                });
                let path = if path.is_absolute() { path } else { std::env::current_dir().unwrap_or_default().join(path) };
                let mut m = Map::new();
                m.insert("path".into(), path.display().to_string().into());
                opt(&mut m, "window", window);
                ("ui.screenshot".into(), Value::Object(m), None)
            }
            UiCmd::Press { target, window, timeout } => {
                let mut m = Map::new();
                m.insert("target".into(), target.into());
                opt(&mut m, "window", window);
                opt(&mut m, "timeout", timeout);
                ("ui.press".into(), Value::Object(m), timeout)
            }
        },
        Cmd::Init { force, vm, image, pharo } => {
            let home = init::home_dir();
            match init::init(&home, force, vm.zip(image), pharo) {
                Ok(r) => return emit(&json!({"ok": true, "result": r}), pretty),
                Err(e) => die(1, "init_failed", e, pretty),
            }
        }
        Cmd::Instances => return list_instances(&inst, pretty),
        Cmd::Start { vm, image, gui, gtk, vm_args, pharo } => return start(&inst, vm, image, if gui { Some("gui") } else if gtk { Some("worker") } else { None }, vm_args, pharo, pretty),
        Cmd::Stop { force } => return stop(&inst, force, pretty),
        Cmd::Status => ("status".into(), json!({}), None),
        Cmd::Ping => ("ping".into(), json!({}), None),
        Cmd::Eval { code, receiver, timeout, full, no_session, no_source, ui } => {
            let code = if code == "-" { read_stdin() } else { code };
            let mut m = Map::new();
            m.insert("code".into(), code.into());
            if full { m.insert("full".into(), true.into()); }
            if no_session { m.insert("no_session".into(), true.into()); }
            if no_source { m.insert("no_source".into(), true.into()); }
            if ui { m.insert("ui".into(), true.into()); }
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
            MethodCmd::Compile { class, source_arg, source, file, protocol, ui } => {
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
                if ui { m.insert("ui".into(), true.into()); }
                ("method.compile".into(), Value::Object(m), None)
            }
            MethodCmd::Remove { class, selector, force } => ("method.remove".into(), json!({"class": class, "selector": selector, "force": force}), None),
        },
        Cmd::Changes(c) => match c {
            ChangesCmd::Watch { interval } => return watch_changes(&inst, interval, pretty),
            ChangesCmd::List { offset, limit, since } => {
                let mut m = Map::new();
                opt(&mut m, "since", since);
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
            DebugCmd::Eval { session, code, frame, timeout, full, no_session, ui } => {
                let code = if code == "-" { read_stdin() } else { code };
                let mut m = Map::new();
                m.insert("session".into(), session.into());
                m.insert("code".into(), code.into());
                m.insert("frame".into(), frame.into());
                if full { m.insert("full".into(), true.into()); }
                if no_session { m.insert("no_session".into(), true.into()); }
                if ui { m.insert("ui".into(), true.into()); }
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
        Cmd::Image(ImageCmd::Export { out, keep_server, force }) => return export_image(&inst, &out, keep_server, force, pretty),
        Cmd::Image(ImageCmd::Clone { name, start: do_start }) => return clone_image(&inst, &name, do_start, pretty),
        Cmd::Save { package, all, dir, dry_run, verify } => return save_changes(&inst, package, all, &dir, dry_run, verify, pretty),
        Cmd::Run { load, packages, prepare, eval, file, timeout, keep, args } => return run_program(&inst, load, packages, prepare, eval, file, timeout, keep, args, pretty),
        Cmd::Drift { package, all, dir } => return drift_cmd(&inst, package, all, &dir, pretty),
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
    let summary = summarize(&cmd, &args);
    let mut resp = call(&inst, &cmd, args, read_timeout, pretty);
    if cmd == "changes.show" {
        add_diff(&mut resp);
    }
    emit(&resp, pretty);
    let code = exit_code_for(&resp);
    if let Some(path) = &cli.log {
        write_log(path, cli.tag.as_deref(), &inst.name, &cmd, &summary, started.elapsed(), &resp, code);
    }
    std::process::exit(code);
}

/// Open `url` in a chromeless application window (Chrome/Edge `--app=`), the web-UI way of "developing a window".
fn open_ui(url: &str, browser: Option<String>, pretty: bool) {
    let app = format!("--app={}", url);
    let status = if cfg!(target_os = "macos") {
        let name = browser.unwrap_or_else(|| "Google Chrome".into());
        Command::new("open").args(["-na", &name, "--args", &app]).status()
    } else if cfg!(target_os = "windows") {
        let exe = browser.unwrap_or_else(|| "msedge".into());
        Command::new("cmd").args(["/C", "start", "", &exe, &app]).status()
    } else {
        let candidates = match browser {
            Some(b) => vec![b],
            None => ["google-chrome", "chromium", "chromium-browser", "microsoft-edge"].iter().map(|s| s.to_string()).collect(),
        };
        let mut last = Err(std::io::Error::new(std::io::ErrorKind::NotFound, "no Chrome/Chromium/Edge found; pass --browser"));
        for c in candidates {
            last = Command::new(&c).arg(&app).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map(|_| std::process::ExitStatus::default());
            if last.is_ok() { break; }
        }
        last
    };
    match status {
        Ok(s) if s.success() => emit(&json!({"ok": true, "result": {"opened": url}}), pretty),
        Ok(s) => die(1, "open_failed", format!("browser launcher exited with {}", s), pretty),
        Err(e) => die(1, "open_failed", e.to_string(), pretty),
    }
}

/// Export the running image under a new name for delivery (server removed), stop the instance and complete the file set.
fn export_image(inst: &Instance, out: &Path, keep_server: bool, force: bool, pretty: bool) {
    if !force {
        die(2, "confirmation_required", "Export saves the image under the new name and ends this instance; add --force.".into(), pretty);
    }
    let abs = if out.is_absolute() { out.to_path_buf() } else { std::env::current_dir().unwrap_or_default().join(out) };
    let abs = if abs.extension().map(|e| e == "image").unwrap_or(false) { abs } else { abs.with_extension("image") };
    if let Some(parent) = abs.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let r = call(inst, "image.export", json!({"path": abs.display().to_string(), "keep_server": keep_server, "force": true}), Duration::from_secs(60), pretty);
    if r["ok"] != json!(true) {
        emit(&r, pretty);
        std::process::exit(1);
    }
    // the image saves itself and quits
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline && inst.alive() {
        std::thread::sleep(Duration::from_millis(200));
    }
    if inst.alive() {
        if let Some(p) = inst.pid() { platform::kill_pid(p); }
    }
    let _ = std::fs::remove_file(inst.path("port.json"));
    let _ = std::fs::remove_file(inst.path("pid"));
    // the sources file must sit next to the delivered image
    let cfg: Value = std::fs::read_to_string(inst.path("config.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(json!({}));
    let mut sources = None;
    if let Some(src_dir) = cfg["image"].as_str().and_then(|i| Path::new(i).parent().map(|p| p.to_path_buf())) {
        if let Ok(rd) = std::fs::read_dir(&src_dir) {
            for e in rd.flatten() {
                if e.path().extension().map(|x| x == "sources").unwrap_or(false) {
                    let dest = abs.parent().unwrap().join(e.file_name());
                    if std::fs::copy(e.path(), &dest).is_ok() { sources = Some(dest); }
                }
            }
        }
    }
    let ok = abs.exists();
    emit(&json!({"ok": ok, "result": {"image": abs, "changes": abs.with_extension("changes"), "sources": sources, "server_removed": !keep_server,
        "note": "Start it with your VM and your own startup script; this image has no stlive port."}}), pretty);
    if !ok { std::process::exit(1); }
}

/// Package folders (with Tonel files) found below `dir`.
fn package_folders(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_dir())
                .filter(|e| std::fs::read_dir(e.path()).map(|r| r.flatten().any(|f| f.file_name().to_string_lossy().ends_with(".st"))).unwrap_or(false))
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// Drift report per package: what the image has that the files lack and vice versa.
fn verify_packages(inst: &Instance, packages: &[String], dir: &Path, pretty: bool) -> Vec<Value> {
    packages
        .iter()
        .map(|p| {
            let r = call(inst, "package.methods", json!({"package": p}), Duration::from_secs(120), pretty);
            if r["ok"] == json!(true) {
                save::drift(&r["result"], p, dir)
            } else if r["error"]["code"] == json!("unknown_package") {
                json!({"package": p, "ok": true, "note": "folder exists but the package is not loaded in the image; nothing to compare"})
            } else {
                json!({"package": p, "ok": false, "error": r["error"]})
            }
        })
        .collect()
}

fn drift_cmd(inst: &Instance, package: Option<String>, all: bool, dir: &Path, pretty: bool) {
    let packages = match (package, all) {
        (Some(p), false) => vec![p],
        (None, true) => package_folders(dir),
        _ => die(2, "missing_argument", "Give --package <Name> or --all.".into(), pretty),
    };
    let reports = verify_packages(inst, &packages, dir, pretty);
    let ok = reports.iter().all(|r| r["ok"] == json!(true));
    emit(&json!({"ok": ok, "result": {"drift": !ok, "packages": reports}}), pretty);
    if !ok { std::process::exit(1); }
}

fn save_changes(inst: &Instance, package: Option<String>, all: bool, dir: &Path, dry_run: bool, verify: bool, pretty: bool) {
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
    let mut by_pkg: std::collections::BTreeMap<String, Vec<Value>> = Default::default();
    let mut warnings: Vec<Value> = Vec::new();
    for c in changes {
        match c["package"].as_str() {
            Some(p) => by_pkg.entry(p.to_string()).or_default().push(c),
            None => warnings.push(json!({
                "change": c["index"], "class": c["class"], "selector": c["selector"],
                "problem": "the change has no package (class created without one?), so it was NOT written; give the class a package and compile again"})),
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
    if !dry_run && !marked.is_empty() {
        let _ = call(inst, "changes.mark_saved", json!({"indexes": marked}), Duration::from_secs(30), pretty);
    }
    let mut result = json!({"dry_run": dry_run, "packages": by_pkg.keys().collect::<Vec<_>>(), "changes": marked.len(), "files": files});
    if marked.is_empty() && warnings.is_empty() {
        result["note"] = json!("No unsaved changes recorded. Use `stlive drift` to compare the image with the files.");
    }
    let mut ok = warnings.is_empty();
    if !warnings.is_empty() {
        result["warnings"] = json!(warnings);
    }
    if verify && !dry_run {
        let mut pkgs: Vec<String> = by_pkg.keys().cloned().collect();
        match (&package, all) {
            (Some(p), false) => { if !pkgs.contains(p) { pkgs.push(p.clone()); } }
            _ => { for p in package_folders(dir) { if !pkgs.contains(&p) { pkgs.push(p); } } }
        }
        let reports = verify_packages(inst, &pkgs, dir, pretty);
        if reports.iter().any(|r| r["ok"] != json!(true)) { ok = false; }
        result["verify"] = json!(reports);
    }
    emit(&json!({"ok": ok, "result": result}), pretty);
    if !ok { std::process::exit(1); }
}

/// `stlive run`: fresh image, load Tonel sources, evaluate, pass through stdout/stderr/exit code.
#[allow(clippy::too_many_arguments)]
fn run_program(inst: &Instance, load: Vec<PathBuf>, packages: Vec<String>, prepare: Option<PathBuf>, eval: Option<String>, file: Option<PathBuf>, timeout: u64, keep: bool, args: Vec<String>, pretty: bool) {
    if eval.is_none() && file.is_none() {
        die(2, "missing_argument", "Give --eval '<expression>' or --file <script.st>.".into(), pretty);
    }
    let (vm, template) = match init::template(&init::home_dir(), init::DEFAULT_PHARO) {
        Some(t) => t,
        None => die(2, "not_initialized", "Run `stlive init` once first.".into(), pretty),
    };
    let abs = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let dir = inst.dir.join(format!("run-{}-{}", std::process::id(), ms));
    let _ = std::fs::create_dir_all(&inst.dir);
    if !inst.dir.join(".gitignore").exists() {
        let _ = std::fs::write(inst.dir.join(".gitignore"), "# created by stlive: local state, never commit\n*\n");
    }
    let image = match init::instance_image(&template, &dir, "run") {
        Ok(i) => i,
        Err(e) => die(3, "image_copy_failed", e, pretty),
    };
    let script = dir.join("run.st");
    let _ = std::fs::write(&script, init::run_st());
    let src_dir = init::write_sources(&init::home_dir()).ok();
    let log = std::fs::File::create(dir.join("vm.log")).expect("cannot create log file");
    let load_abs: Vec<String> = load.iter().map(|p| abs(p).display().to_string()).collect();
    let mut cmd = Command::new(&vm);
    cmd.arg("--headless")
        .arg(&image)
        .arg("st")
        .arg(&script)
        .arg("--quit")
        .env("STLIVE_RUN_DIR", &dir)
        .env("STLIVE_RUN_ARGS", serde_json::to_string(&args).unwrap_or_default())
        .env("STLIVE_RUN_LOAD", serde_json::to_string(&load_abs).unwrap_or_default())
        .env("STLIVE_RUN_PACKAGES", serde_json::to_string(&packages).unwrap_or_default())
        .env("STLIVE_SOURCES_HASH", init::sources_hash())
        .envs(src_dir.iter().map(|d| ("STLIVE_SRC", d.clone())))
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log);
    if let Some(p) = &prepare { cmd.env("STLIVE_RUN_PREPARE", abs(p)); }
    if let Some(f) = &file { cmd.env("STLIVE_RUN_FILE", abs(f)); }
    if let Some(e) = &eval { cmd.env("STLIVE_RUN_EXPR", e); }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => die(3, "spawn_failed", format!("Cannot start {}: {}", vm.display(), e), pretty),
    };
    let deadline = Instant::now() + Duration::from_secs(timeout);
    let mut timed_out = false;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(_) => break,
        }
        if Instant::now() > deadline {
            timed_out = true;
            let _ = child.kill();
            let _ = child.wait();
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let out = std::fs::read(dir.join("out.txt")).unwrap_or_default();
    let err = std::fs::read(dir.join("err.txt")).unwrap_or_default();
    let code_file: Option<i32> = std::fs::read_to_string(dir.join("exit.txt")).ok().and_then(|t| t.trim().parse().ok());
    {
        let mut so = std::io::stdout();
        let _ = so.write_all(&out);
        let _ = so.flush();
        let mut se = std::io::stderr();
        let _ = se.write_all(&err);
        if code_file.is_none() && !timed_out {
            // the image did not get to write its result: show why
            let vmlog = std::fs::read_to_string(dir.join("vm.log")).unwrap_or_default();
            let tail: Vec<&str> = vmlog.lines().filter(|l| !l.trim().is_empty()).collect();
            let _ = writeln!(se, "stlive run: the image ended without a result; last lines of {}:", dir.join("vm.log").display());
            for l in tail.iter().rev().take(12).rev() { let _ = writeln!(se, "  {}", l); }
        }
        if timed_out { let _ = writeln!(se, "stlive run: timed out after {} s", timeout); }
        let _ = se.flush();
    }
    let code = if timed_out { 124 } else { code_file.unwrap_or(70) };
    if !keep && (code_file.is_some() || timed_out) {
        let _ = std::fs::remove_dir_all(&dir);
    } else if keep || code_file.is_none() {
        eprintln!("stlive run: work directory kept: {}", dir.display());
    }
    std::process::exit(code);
}
