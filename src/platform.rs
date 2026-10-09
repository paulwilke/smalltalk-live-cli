//! The few places where Unix and Windows differ: process control, links, home directory, Pharo download.
//! The Windows branches are written to the documented APIs but have not been run on a Windows machine yet.

use std::path::{Path, PathBuf};
use std::process::Command;

pub fn home() -> PathBuf {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    PathBuf::from(std::env::var(var).unwrap_or_else(|_| ".".into()))
}

/// Is a process with this id alive?
#[cfg(unix)]
pub fn pid_alive(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}
#[cfg(windows)]
pub fn pid_alive(pid: i32) -> bool {
    Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
        .unwrap_or(false)
}

/// Forcefully end a process.
#[cfg(unix)]
pub fn kill_pid(pid: i32) {
    unsafe { libc::kill(pid, libc::SIGKILL) };
}
#[cfg(windows)]
pub fn kill_pid(pid: i32) {
    let _ = Command::new("taskkill").args(["/PID", &pid.to_string(), "/F"]).output();
}

/// Make the child independent of this (short-lived) CLI process.
#[cfg(unix)]
pub fn detach(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    cmd.process_group(0);
}
#[cfg(windows)]
pub fn detach(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

/// Symlink `link` -> `target`; where symlinks need privileges (Windows) copy the file instead.
#[cfg(unix)]
pub fn link_or_copy(target: &Path, link: &Path) {
    let _ = std::os::unix::fs::symlink(target, link);
}
#[cfg(windows)]
pub fn link_or_copy(target: &Path, link: &Path) {
    let _ = std::fs::copy(target, link);
}

/// Process listening on a local TCP port (used by `attach` when --pid is not given).
#[cfg(unix)]
pub fn pid_listening_on(port: u16) -> Option<i32> {
    let out = Command::new("lsof").args(["-ti", &format!("tcp:{}", port), "-sTCP:LISTEN"]).output().ok()?;
    String::from_utf8_lossy(&out.stdout).lines().next()?.trim().parse().ok()
}
#[cfg(windows)]
pub fn pid_listening_on(port: u16) -> Option<i32> {
    let out = Command::new("netstat").args(["-ano", "-p", "tcp"]).output().ok()?;
    let needle = format!(":{} ", port);
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find(|l| l.contains("LISTENING") && l.contains(&needle))
        .and_then(|l| l.split_whitespace().last()?.parse().ok())
}

/// Candidate VM executables below the Pharo directory, in order.
pub fn vm_candidates() -> &'static [&'static str] {
    &[
        "pharo-vm/Pharo.app/Contents/MacOS/Pharo",
        "pharo-vm/pharo",
        "pharo-vm/bin/pharo",
        "pharo-vm/Pharo.exe",
        "pharo-vm/PharoConsole.exe",
    ]
}

fn run(cmd: &mut Command, what: &str) -> Result<(), String> {
    let status = cmd.status().map_err(|e| format!("{}: {}", what, e))?;
    if status.success() { Ok(()) } else { Err(format!("{} failed ({})", what, status)) }
}

/// Download and unpack the Pharo 13 VM and image into `dir`.
#[cfg(unix)]
pub fn download_pharo(dir: &Path, version: u32) -> Result<(), String> {
    let script = dir.join("get-pharo.sh");
    run(Command::new("curl").args(["-fsSL", "-o"]).arg(&script).arg(format!("https://get.pharo.org/64/{}0+vm", version)), "download of the get.pharo.org script (is curl installed?)")?;
    run(Command::new("bash").arg(&script).current_dir(dir), "Pharo download (needs curl and unzip)")
}
#[cfg(windows)]
pub fn download_pharo(dir: &Path, version: u32) -> Result<(), String> {
    // curl.exe and tar.exe ship with Windows 10+. File names follow files.pharo.org/get-files/130/.
    let items = [
        (format!("https://files.pharo.org/get-files/{}0/pharoImage-x86_64.zip", version), "image.zip"),
        (format!("https://files.pharo.org/get-files/{}0/pharo-vm-Windows-x86_64-stable.zip", version), "vm.zip"),
    ];
    for (url, name) in items {
        let zip = dir.join(name);
        run(Command::new("curl.exe").args(["-fsSL", "-o"]).arg(&zip).arg(&url), "download (needs curl.exe)")?;
        let target = if name == "vm.zip" { dir.join("pharo-vm") } else { dir.to_path_buf() };
        std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        run(Command::new("tar.exe").arg("-xf").arg(&zip).arg("-C").arg(&target), "unzip (needs tar.exe)")?;
    }
    Ok(())
}
