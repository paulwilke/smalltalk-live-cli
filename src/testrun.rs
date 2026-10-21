//! `stlive test run`: target expansion and the fresh run (`--fresh`).
//!
//! A target is a TestCase class, a package, `Class>>selector`, or a glob pattern (`*`, `?`) over package and test
//! class names. Glob patterns are expanded here from the names the image lists (`test.names`); the image then runs
//! the exact names in one run, so the failure groups and debug sessions of the run stay together.
//!
//! `--fresh` runs the same targets in a temporary instance: it is started from the template image (the copy that
//! `stlive start` makes), the baseline is loaded (`load.baseline`), the tests run, and the instance is stopped and
//! its files deleted. `--keep` leaves it running; its name is in the output.

use crate::{call, emit_inner, exit_code_for, log_response, opt, platform, start_quiet, stop_quiet, try_call, Instance};
use serde_json::{json, Map, Value};
use signal_hook::consts::{SIGINT, SIGTERM};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Held by whichever thread finishes the process (a completed run, or an interrupt): the cleanup and the exit happen
/// under it, so the other thread waits until the process ends instead of cleaning up or printing a second time.
static FINISHING: Mutex<()> = Mutex::new(());

/// Options of a test run, as the command line gives them.
pub struct TestOpts {
    pub timeout: Option<u64>,
    pub keep_sessions: bool,
    pub discard_sessions: bool,
    pub skips_fail: bool,
}

/// `--fresh`: the baseline to load into the temporary instance.
pub struct FreshSpec {
    pub baseline: String,
    pub repository: String,
    pub groups: Vec<String>,
    pub keep: bool,
    pub load_timeout: Option<u64>,
}

/// A step that failed: exit code, error code, message, and more fields of the error object.
struct Fail {
    code: i32,
    error: &'static str,
    message: String,
    extra: Map<String, Value>,
}

impl Fail {
    fn new(code: i32, error: &'static str, message: String) -> Fail {
        Fail { code, error, message, extra: Map::new() }
    }
}

fn from_try(e: (i32, &'static str, String)) -> Fail {
    Fail::new(e.0, e.1, e.2)
}

fn fail_response(f: &Fail) -> Value {
    let mut err = Map::new();
    err.insert("code".into(), json!(f.error));
    err.insert("message".into(), json!(f.message));
    for (k, v) in &f.extra {
        err.insert(k.clone(), v.clone());
    }
    json!({"ok": false, "error": Value::Object(err)})
}

/// Print a response, log it with `code` and exit.
fn exit_with(resp: &Value, code: i32, pretty: bool) -> ! {
    emit_inner(resp, pretty);
    log_response(resp, code);
    std::process::exit(code)
}

fn or_exit<T>(r: Result<T, Fail>, pretty: bool) -> T {
    r.unwrap_or_else(|f| exit_with(&fail_response(&f), f.code, pretty))
}

/// Socket budget of a test run: the timeout is per test, the whole run may take minutes.
fn test_read_timeout(timeout: Option<u64>) -> Duration {
    Duration::from_millis((timeout.unwrap_or(10_000) * 20).max(600_000) + 15_000)
}

fn test_args(targets: &[String], opts: &TestOpts) -> Value {
    let mut m = Map::new();
    m.insert("targets".into(), json!(targets));
    if let Some(t) = opts.timeout {
        m.insert("timeout".into(), json!(t));
    }
    if opts.keep_sessions {
        m.insert("keep_sessions".into(), json!(true));
    }
    if opts.discard_sessions {
        m.insert("discard_sessions".into(), json!(true));
    }
    Value::Object(m)
}

/// Merge keys into the `result` of a response (created when missing).
fn merge_result(resp: &mut Value, extra: Map<String, Value>) {
    if !resp["result"].is_object() {
        resp["result"] = json!({});
    }
    for (k, v) in extra {
        resp["result"][k] = v;
    }
}

/// The targets actually run, and the expansion of each glob pattern.
fn add_run_info(resp: &mut Value, targets: &[String], expanded: Map<String, Value>) {
    if resp["ok"] != json!(true) {
        return;
    }
    let mut extra = Map::new();
    extra.insert("targets".into(), json!(targets));
    if !expanded.is_empty() {
        extra.insert("expanded".into(), Value::Object(expanded));
    }
    merge_result(resp, extra);
}

/// `--skips-fail`: skipped tests count as failures for the exit code (see `exit_code_for`).
fn skips_note(resp: &mut Value, skips_fail: bool) {
    if skips_fail && resp["result"]["skipped"].as_u64().unwrap_or(0) > 0 {
        resp["result"]["skips_fail"] = json!(true);
        resp["result"]["note"] = json!("Skipped tests count as failures (--skips-fail): exit code 1.");
    }
}

// ---------------------------------------------------------------- targets

/// A target with `*` or `?` is a pattern; `Class>>selector` never is.
pub fn is_glob(target: &str) -> bool {
    !target.contains(">>") && target.chars().any(|c| c == '*' || c == '?')
}

/// Shell-style match of a name: `*` any run of characters, `?` exactly one. Case-insensitive, like `test list`.
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let n: Vec<char> = name.to_lowercase().chars().collect();
    let (mut pi, mut ni) = (0, 0);
    // the last '*' seen, and the name position it currently starts from
    let mut star: Option<usize> = None;
    let mut resume = 0;
    while ni < n.len() {
        if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            resume = ni;
            pi += 1;
        } else if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            pi += 1;
            ni += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            resume += 1;
            ni = resume;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

/// The names the image lists for glob expansion: packages with their number of test classes, and test class names.
#[derive(Debug, Default, PartialEq)]
pub struct TestNames {
    pub packages: Vec<(String, usize)>,
    pub classes: Vec<String>,
}

pub fn test_names_from(result: &Value) -> TestNames {
    let packages: Vec<(String, usize)> = result["packages"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|p| Some((p["name"].as_str()?.to_string(), p["test_classes"].as_u64().unwrap_or(0) as usize)))
                .collect()
        })
        .unwrap_or_default();
    let classes: Vec<String> = result["test_classes"]
        .as_array()
        .map(|list| list.iter().filter_map(|c| c.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    TestNames { packages, classes }
}

/// Expand the targets: literal targets stay as they are; a glob matches the packages that have test classes and
/// the test class names it matches. Each exact name appears once, in target order. A glob that matches nothing is
/// an error, carrying its pattern.
pub fn expand_targets(targets: &[String], names: &TestNames) -> Result<(Vec<String>, Map<String, Value>), String> {
    let mut out: Vec<String> = Vec::new();
    let mut expanded = Map::new();
    for target in targets {
        if !is_glob(target) {
            if !out.contains(target) {
                out.push(target.clone());
            }
            continue;
        }
        let mut matches: Vec<String> = names
            .packages
            .iter()
            .filter(|(name, tests)| *tests > 0 && glob_match(target, name))
            .map(|(name, _)| name.clone())
            .collect();
        matches.extend(names.classes.iter().filter(|c| glob_match(target, c)).cloned());
        if matches.is_empty() {
            return Err(target.clone());
        }
        for m in &matches {
            if !out.contains(m) {
                out.push(m.clone());
            }
        }
        expanded.insert(target.clone(), json!(matches));
    }
    Ok((out, expanded))
}

/// `--fresh` without targets: the packages the load added that end in -Tests.
pub fn default_targets(added: &[String]) -> Vec<String> {
    added.iter().filter(|p| p.ends_with("-Tests")).cloned().collect()
}

/// Why a baseline load did not come out clean: it failed, or left names undeclared (code references a global that
/// nothing defines), or reported warnings. `None` when it is clean.
pub fn load_problem(baseline: &str, resp: &Value) -> Option<String> {
    if resp["ok"] != json!(true) {
        let message = resp["error"]["message"].as_str().unwrap_or("unknown error");
        return Some(format!("Loading {} failed: {}", baseline, message));
    }
    let undeclared: Vec<Value> = resp["result"]["loaded"]["undeclared"].as_array().cloned().unwrap_or_default();
    if !undeclared.is_empty() {
        let count = resp["result"]["loaded"]["undeclared_count"].as_u64().unwrap_or(undeclared.len() as u64);
        let names: Vec<&str> = undeclared.iter().take(5).filter_map(|u| u["name"].as_str()).collect();
        let first = &undeclared[0];
        return Some(format!(
            "Loading {} left {} undeclared name(s): {}{} (first used by {})",
            baseline,
            count,
            names.join(", "),
            if count > names.len() as u64 { ", ..." } else { "" },
            first["methods"][0].as_str().unwrap_or("?")
        ));
    }
    let warnings: Vec<Value> = resp["result"]["loaded"]["warnings"].as_array().cloned().unwrap_or_default();
    match warnings.first() {
        None => None,
        Some(first) => Some(format!(
            "Loading {} reported {} warning(s), first: {}",
            baseline,
            warnings.len(),
            first["message"].as_str().unwrap_or("?")
        )),
    }
}

/// Names of all packages in the image, sorted.
fn package_names(inst: &Instance) -> Result<Vec<String>, Fail> {
    let resp = try_call(inst, "find.package", json!({"pattern": "*", "limit": 1_000_000}), Duration::from_secs(60)).map_err(from_try)?;
    if resp["ok"] != json!(true) {
        let message = resp["error"]["message"].as_str().unwrap_or("unknown error");
        return Err(Fail::new(1, "packages_unavailable", format!("Listing the packages failed: {}", message)));
    }
    let mut names: Vec<String> = resp["result"]["packages"]
        .as_array()
        .map(|list| list.iter().filter_map(|p| p["name"].as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    names.sort();
    Ok(names)
}

/// The exact targets to run. Globs are expanded over the image's names with one `test.names` call, made only when
/// there is a glob.
fn resolve(inst: &Instance, targets: &[String]) -> Result<(Vec<String>, Map<String, Value>), Fail> {
    if !targets.iter().any(|t| is_glob(t)) {
        let mut unique: Vec<String> = Vec::new();
        for t in targets {
            if !unique.contains(t) {
                unique.push(t.clone());
            }
        }
        return Ok((unique, Map::new()));
    }
    let resp = try_call(inst, "test.names", json!({}), Duration::from_secs(60)).map_err(from_try)?;
    if resp["ok"] != json!(true) {
        let message = resp["error"]["message"].as_str().unwrap_or("unknown error");
        return Err(Fail::new(1, "test_names_failed", format!("Listing the test names failed: {}", message)));
    }
    let names = test_names_from(&resp["result"]);
    expand_targets(targets, &names).map_err(|pattern| {
        let mut f = Fail::new(
            1,
            "no_tests_matched",
            format!("No package with test classes and no test class matches {}", pattern),
        );
        f.extra.insert("pattern".into(), json!(pattern));
        f
    })
}

// ---------------------------------------------------------------- commands

/// `stlive test run TARGET...` in the instance `inst`: one run of the expanded targets.
pub fn test_run(inst: &Instance, targets: Vec<String>, opts: TestOpts, pretty: bool) -> ! {
    let (targets, expanded) = or_exit(resolve(inst, &targets), pretty);
    let mut resp = call(inst, "test.run", test_args(&targets, &opts), test_read_timeout(opts.timeout), pretty);
    add_run_info(&mut resp, &targets, expanded);
    skips_note(&mut resp, opts.skips_fail);
    let code = exit_code_for(&resp);
    exit_with(&resp, code, pretty)
}

/// Stop and clean up (unless kept), then print the response and exit with `code`. Also the path of an interrupt
/// (see `watch_interrupts`), which stops the instance even when the run was to keep it.
fn end(inst: &Instance, keep: bool, code: i32, resp: &Value, pretty: bool) -> ! {
    let _finishing = FINISHING.lock().unwrap_or_else(|e| e.into_inner());
    if keep {
        mark(inst, "kept", json!(true));
    } else {
        stop_quiet(inst);
        remove_files(inst);
    }
    exit_with(resp, code, pretty)
}

/// SIGINT and SIGTERM during a fresh run: the instance is stopped and its files deleted by the same path as a finished
/// run, and the process exits with 130 (SIGINT) or 143 (SIGTERM). Installed for the whole fresh run, from before the
/// instance starts. A watcher thread polls the signal flags, because the main thread may be blocked in a socket read.
fn watch_interrupts(inst: &Instance, pretty: bool) -> Result<(), String> {
    let int = Arc::new(AtomicBool::new(false));
    let term = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGINT, Arc::clone(&int)).map_err(|e| e.to_string())?;
    signal_hook::flag::register(SIGTERM, Arc::clone(&term)).map_err(|e| e.to_string())?;
    let (dir, name) = (inst.dir.clone(), inst.name.clone());
    std::thread::spawn(move || loop {
        let (code, signal) = if int.load(Ordering::SeqCst) {
            (130, "SIGINT")
        } else if term.load(Ordering::SeqCst) {
            (143, "SIGTERM")
        } else {
            std::thread::sleep(Duration::from_millis(50));
            continue;
        };
        let inst = Instance { dir: dir.clone(), name: name.clone() };
        let mut f = Fail::new(
            code,
            "interrupted",
            format!("Interrupted by {}: the temporary instance {} was stopped and its files deleted", signal, inst.name),
        );
        f.extra.insert("signal".into(), json!(signal));
        end(&inst, false, code, &fail_response(&f), pretty);
    });
    Ok(())
}

/// The stlive process that started a temporary instance, from its name `fresh-<pid>-<ms>`.
pub fn owner_pid(name: &str) -> Option<i32> {
    name.strip_prefix("fresh-")?.split('-').next()?.parse().ok()
}

/// A temporary instance nobody can use any more: its image is dead, or its owner run is gone and the run did not keep
/// the instance (`--keep` marks it kept once the run completes; a kept instance lives until it is stopped).
pub fn orphaned(temporary: bool, kept: bool, instance_alive: bool, owner_alive: bool) -> bool {
    temporary && (!instance_alive || (!kept && !owner_alive))
}

/// `orphaned` for an instance on disk.
pub fn is_orphaned(inst: &Instance) -> bool {
    let cfg = read_config(inst);
    let owner_alive = owner_pid(&inst.name).map(platform::pid_alive).unwrap_or(false);
    orphaned(cfg["temporary"] == json!(true), cfg["kept"] == json!(true), inst.alive(), owner_alive)
}

fn fresh_end(inst: &Instance, info: Map<String, Value>, keep: bool, code: i32, mut resp: Value, pretty: bool) -> ! {
    merge_result(&mut resp, info);
    end(inst, keep, code, &resp, pretty)
}

fn fresh_or_end<T>(r: Result<T, Fail>, inst: &Instance, keep: bool, info: &Map<String, Value>, pretty: bool) -> T {
    r.unwrap_or_else(|f| fresh_end(inst, info.clone(), keep, f.code, fail_response(&f), pretty))
}

/// Deletes the files of a stopped instance (`NAME.image`, `NAME.changes`, `NAME.config.json`, ...). The sources file
/// that sits next to the images is shared by all instances and stays.
pub fn remove_files(inst: &Instance) {
    let prefix = format!("{}.", inst.name);
    if let Ok(entries) = std::fs::read_dir(&inst.dir) {
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().starts_with(&prefix) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

fn read_config(inst: &Instance) -> Value {
    std::fs::read_to_string(inst.path("config.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(json!({}))
}

/// Sets a key in the instance's config. `temporary` makes `stop` delete its files as well (a fresh run's instance).
fn mark(inst: &Instance, key: &str, value: Value) {
    let mut cfg = read_config(inst);
    cfg[key] = value;
    let _ = std::fs::write(inst.path("config.json"), cfg.to_string());
}

fn mark_temporary(inst: &Instance) {
    mark(inst, "temporary", json!(true));
}

/// Whether the instance was started by `test run --fresh`.
pub fn is_temporary(inst: &Instance) -> bool {
    read_config(inst)["temporary"] == json!(true)
}

/// `stlive test run --fresh`: a temporary instance from the template, the baseline loaded, the targets run, then the
/// instance stopped and deleted. The load is reported under `load`. A load that fails or reports warnings exits 6
/// before any test runs. With `keep`, the instance stays running and is named in the output.
pub fn fresh_test_run(base: &Instance, spec: FreshSpec, targets: Vec<String>, opts: TestOpts, pretty: bool) -> ! {
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let name = format!("fresh-{}-{}", std::process::id(), ms);
    let inst = Instance { dir: base.dir.clone(), name: name.clone() };
    let keep = spec.keep;
    let mut info = Map::new();
    info.insert("fresh".into(), json!(true));
    info.insert("kept".into(), json!(keep));
    if keep {
        info.insert("instance".into(), json!(name));
        info.insert(
            "next".into(),
            json!(format!("Debug the failing tests with: stlive -i {n} debug list. Stop the instance with: stlive -i {n} stop --force", n = name)),
        );
    }

    let _ = std::fs::create_dir_all(&inst.dir);
    if let Err(e) = watch_interrupts(&inst, pretty) {
        eprintln!("stlive: cannot catch interrupts ({}); an interrupted run may leave {} running", e, name);
    }
    if let Err(e) = start_quiet(&inst, None, None, false) {
        let f = from_try(e);
        end(&inst, false, f.code, &fail_response(&f), pretty);
    }
    mark_temporary(&inst);

    // without targets, the packages the load adds are found by comparing the package lists before and after
    let before = if targets.is_empty() { Some(fresh_or_end(package_names(&inst), &inst, keep, &info, pretty)) } else { None };

    let mut load_args = Map::new();
    load_args.insert("name".into(), json!(spec.baseline));
    load_args.insert("repository".into(), json!(spec.repository));
    load_args.insert("groups".into(), json!(spec.groups));
    opt(&mut load_args, "timeout", spec.load_timeout);
    let load_timeout = spec.load_timeout.unwrap_or(600_000);
    let load_resp = fresh_or_end(
        try_call(&inst, "load.baseline", Value::Object(load_args), Duration::from_millis(load_timeout + 15_000)).map_err(from_try),
        &inst,
        keep,
        &info,
        pretty,
    );
    info.insert("load".into(), load_resp["result"]["loaded"].clone());
    if let Some(problem) = load_problem(&spec.baseline, &load_resp) {
        let mut f = Fail::new(6, "load_failed", problem);
        if !load_resp["error"].is_null() {
            f.extra.insert("load_error".into(), load_resp["error"].clone());
        }
        fresh_end(&inst, info, keep, 6, fail_response(&f), pretty);
    }

    let (targets, expanded) = match before {
        Some(before) => {
            let after = fresh_or_end(package_names(&inst), &inst, keep, &info, pretty);
            let added: Vec<String> = after.into_iter().filter(|p| !before.contains(p)).collect();
            let found = default_targets(&added);
            if found.is_empty() {
                let f = Fail::new(1, "no_tests_matched", "The load added no package ending in -Tests; give the targets to run".into());
                fresh_end(&inst, info, keep, 1, fail_response(&f), pretty);
            }
            (found, Map::new())
        }
        None => fresh_or_end(resolve(&inst, &targets), &inst, keep, &info, pretty),
    };

    let mut resp = fresh_or_end(
        try_call(&inst, "test.run", test_args(&targets, &opts), test_read_timeout(opts.timeout)).map_err(from_try),
        &inst,
        keep,
        &info,
        pretty,
    );
    add_run_info(&mut resp, &targets, expanded);
    skips_note(&mut resp, opts.skips_fail);
    merge_result(&mut resp, info);
    let code = exit_code_for(&resp);
    end(&inst, keep, code, &resp, pretty)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> TestNames {
        TestNames {
            packages: vec![
                ("Orbit-Core".into(), 0),
                ("Orbit-Core-Tests".into(), 4),
                ("Orbit-Web-Tests".into(), 2),
                ("Other-Tests".into(), 1),
            ],
            classes: vec!["OrbitStrayTest".into(), "Unrelated".into()],
        }
    }

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn glob_matches_names_case_insensitively() {
        assert!(glob_match("Orbit-*-Tests", "Orbit-Core-Tests"));
        assert!(glob_match("orbit-*-tests", "Orbit-Web-Tests"));
        assert!(!glob_match("Orbit-*-Tests", "Orbit-Core"));
        assert!(glob_match("Orbit-?ore-Tests", "Orbit-Core-Tests"));
        assert!(!glob_match("Orbit-?-Tests", "Orbit-Core-Tests"));
        assert!(glob_match("*Test", "OrbitStrayTest"));
        assert!(glob_match("*", ""));
        assert!(glob_match("a*b*c", "aXXbYYc"));
        assert!(!glob_match("a*b*c", "aXXbYY"));
        assert!(glob_match("*-*-*", "a-b-c-d"));
        assert!(glob_match("Foo", "Foo"));
        assert!(!glob_match("Foo", "Foo2"));
        assert!(!glob_match("?", ""));
    }

    #[test]
    fn only_patterns_without_selector_are_globs() {
        assert!(is_glob("Orbit-*-Tests"));
        assert!(is_glob("Foo?"));
        assert!(!is_glob("Plain-Tests"));
        assert!(!is_glob("Foo>>test*"));
    }

    #[test]
    fn expansion_keeps_target_order_and_each_name_once() {
        let (targets, expanded) = expand_targets(&strings(&["Orbit-*-Tests", "Other-Tests", "Orbit-Web-Tests"]), &names()).unwrap();
        assert_eq!(targets, strings(&["Orbit-Core-Tests", "Orbit-Web-Tests", "Other-Tests"]));
        assert_eq!(expanded["Orbit-*-Tests"], json!(["Orbit-Core-Tests", "Orbit-Web-Tests"]));
        assert!(!expanded.contains_key("Other-Tests"));
    }

    #[test]
    fn a_pattern_also_matches_test_class_names() {
        let (targets, _) = expand_targets(&strings(&["Orbit*"]), &names()).unwrap();
        // the package without test classes is skipped; the stray test class matches by name
        assert_eq!(targets, strings(&["Orbit-Core-Tests", "Orbit-Web-Tests", "OrbitStrayTest"]));
    }

    #[test]
    fn literal_targets_pass_through_without_a_name_list() {
        let (targets, expanded) = expand_targets(&strings(&["Foo>>bar", "Plain"]), &TestNames::default()).unwrap();
        assert_eq!(targets, strings(&["Foo>>bar", "Plain"]));
        assert!(expanded.is_empty());
    }

    #[test]
    fn a_pattern_matching_nothing_is_an_error_with_the_pattern() {
        assert_eq!(expand_targets(&strings(&["Nope-*"]), &names()).unwrap_err(), "Nope-*");
        // a matching package without test classes does not count
        assert_eq!(expand_targets(&strings(&["Orbit-Cor?"]), &names()).unwrap_err(), "Orbit-Cor?");
    }

    #[test]
    fn default_targets_are_the_added_test_packages() {
        let added = strings(&["Orbit-Core", "Orbit-Core-Tests", "Orbit-Web-Tests", "Other-Tests-Support"]);
        assert_eq!(default_targets(&added), strings(&["Orbit-Core-Tests", "Orbit-Web-Tests"]));
        assert!(default_targets(&strings(&["Orbit"])).is_empty());
    }

    #[test]
    fn names_are_read_from_the_image_answer() {
        let answer = json!({
            "packages": [{"name": "A-Tests", "test_classes": 2}, {"name": "A", "test_classes": 0}],
            "test_classes": ["ATest"]
        });
        let n = test_names_from(&answer);
        assert_eq!(n.packages, vec![("A-Tests".to_string(), 2), ("A".to_string(), 0)]);
        assert_eq!(n.classes, strings(&["ATest"]));
    }

    #[test]
    fn a_clean_load_is_no_problem() {
        let clean = json!({"ok": true, "result": {"loaded": {"warnings": [], "undeclared": []}}});
        assert_eq!(load_problem("Orbit", &clean), None);
        let warned = json!({"ok": true, "result": {"loaded": {"warnings": [{"message": "Undeclared variable: x", "count": 1}]}}});
        assert_eq!(
            load_problem("Orbit", &warned).unwrap(),
            "Loading Orbit reported 1 warning(s), first: Undeclared variable: x"
        );
        let failed = json!({"ok": false, "error": {"code": "exception", "message": "boom"}});
        assert_eq!(load_problem("Orbit", &failed).unwrap(), "Loading Orbit failed: boom");
    }

    #[test]
    fn an_undeclared_name_is_a_load_problem_that_names_it() {
        let undeclared = json!({"ok": true, "result": {"loaded": {"warnings": [], "undeclared": [
            {"name": "NoSuchThingXyz", "methods": ["Thing>>answer"], "method_count": 1}], "undeclared_count": 1}}});
        assert_eq!(
            load_problem("Orbit", &undeclared).unwrap(),
            "Loading Orbit left 1 undeclared name(s): NoSuchThingXyz (first used by Thing>>answer)"
        );
        let empty = json!({"ok": true, "result": {"loaded": {"warnings": [], "undeclared": [], "undeclared_count": 0}}});
        assert_eq!(load_problem("Orbit", &empty), None);
    }

    #[test]
    fn undeclared_names_in_a_load_problem_are_at_most_five() {
        let names: Vec<Value> = (1..=6).map(|i| json!({"name": format!("N{}", i), "methods": ["A>>b"], "method_count": 1})).collect();
        let resp = json!({"ok": true, "result": {"loaded": {"undeclared": names, "undeclared_count": 6}}});
        assert_eq!(
            load_problem("Orbit", &resp).unwrap(),
            "Loading Orbit left 6 undeclared name(s): N1, N2, N3, N4, N5, ... (first used by A>>b)"
        );
    }

    #[test]
    fn the_owner_of_a_temporary_instance_is_the_pid_in_its_name() {
        assert_eq!(owner_pid("fresh-4242-1700000000000"), Some(4242));
        assert_eq!(owner_pid("default"), None);
        assert_eq!(owner_pid("fresh-x-1"), None);
        assert_eq!(owner_pid("fresh-"), None);
    }

    #[test]
    fn a_temporary_instance_is_orphaned_when_nobody_can_use_it() {
        // its run is going on
        assert!(!orphaned(true, false, true, true));
        // its run is gone and did not keep it
        assert!(orphaned(true, false, true, false));
        // its run is gone, but the run kept it on purpose (until `stop`)
        assert!(!orphaned(true, true, true, false));
        // its image is dead, kept or not
        assert!(orphaned(true, true, false, true));
        assert!(orphaned(true, false, false, true));
        // an instance that was not started by a fresh run is never an orphan
        assert!(!orphaned(false, false, false, false));
    }

    #[test]
    fn test_run_arguments_carry_the_targets() {
        let opts = TestOpts { timeout: None, keep_sessions: false, discard_sessions: false, skips_fail: false };
        assert_eq!(test_args(&strings(&["A-Tests"]), &opts), json!({"targets": ["A-Tests"]}));
        let opts = TestOpts { timeout: Some(500), keep_sessions: true, discard_sessions: false, skips_fail: true };
        assert_eq!(test_args(&strings(&["A"]), &opts), json!({"targets": ["A"], "timeout": 500, "keep_sessions": true}));
    }

    #[test]
    fn the_socket_budget_covers_a_large_run() {
        assert_eq!(test_read_timeout(None), Duration::from_millis(615_000));
        assert_eq!(test_read_timeout(Some(60_000)), Duration::from_millis(1_215_000));
    }
}
