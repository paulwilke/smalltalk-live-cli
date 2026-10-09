//! `stlive init`: download Pharo 13 (VM + image) and build an image containing the StLive server.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const PREPARE_ST: &str = r#"| src |
src := (Smalltalk os environment at: 'STLIVE_SRC') asFileReference.
Metacello new
	baseline: 'StLive';
	repository: 'tonel://' , src fullName;
	load.
(Smalltalk globals at: #StLiveServer) recordSystemPackages.
Smalltalk snapshot: true andQuit: true.
"#;

/// (relative path, content) of the Smalltalk sources compiled into the binary.
const SOURCES: &[(&str, &str)] = &[
    ("BaselineOfStLive/package.st", include_str!("../smalltalk/src/BaselineOfStLive/package.st")),
    ("BaselineOfStLive/BaselineOfStLive.class.st", include_str!("../smalltalk/src/BaselineOfStLive/BaselineOfStLive.class.st")),
    ("StLive/package.st", include_str!("../smalltalk/src/StLive/package.st")),
    ("StLive/StLiveCommands.class.st", include_str!("../smalltalk/src/StLive/StLiveCommands.class.st")),
    ("StLive/StLiveDescriber.class.st", include_str!("../smalltalk/src/StLive/StLiveDescriber.class.st")),
    ("StLive/StLiveErrorHandler.class.st", include_str!("../smalltalk/src/StLive/StLiveErrorHandler.class.st")),
    ("StLive/StLiveFailure.class.st", include_str!("../smalltalk/src/StLive/StLiveFailure.class.st")),
    ("StLive/StLiveJob.class.st", include_str!("../smalltalk/src/StLive/StLiveJob.class.st")),
    ("StLive/StLiveServer.class.st", include_str!("../smalltalk/src/StLive/StLiveServer.class.st")),
    ("StLive/StLiveSession.class.st", include_str!("../smalltalk/src/StLive/StLiveSession.class.st")),
    ("StLive/StLiveTranscriptTee.class.st", include_str!("../smalltalk/src/StLive/StLiveTranscriptTee.class.st")),
];

pub fn home_dir() -> PathBuf {
    if let Ok(h) = std::env::var("STLIVE_HOME") {
        return PathBuf::from(h);
    }
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into())).join(".stlive")
}

fn find_vm(pharo_dir: &Path) -> Option<PathBuf> {
    ["pharo-vm/Pharo.app/Contents/MacOS/Pharo", "pharo-vm/pharo", "pharo-vm/bin/pharo"]
        .iter()
        .map(|p| pharo_dir.join(p))
        .find(|p| p.exists())
}

fn run(cmd: &mut Command, what: &str) -> Result<(), String> {
    let status = cmd.status().map_err(|e| format!("{}: {}", what, e))?;
    if status.success() { Ok(()) } else { Err(format!("{} failed ({})", what, status)) }
}

pub fn init(home: &Path, force: bool) -> Result<Value, String> {
    let pharo_dir = home.join("pharo");
    std::fs::create_dir_all(&pharo_dir).map_err(|e| e.to_string())?;

    // 1. Pharo 13 VM + base image from get.pharo.org
    if force || !pharo_dir.join("Pharo.image").exists() || find_vm(&pharo_dir).is_none() {
        eprintln!("stlive: downloading Pharo 13 (VM + image, ~100 MB) from get.pharo.org ...");
        let script = pharo_dir.join("get-pharo.sh");
        run(Command::new("curl").args(["-fsSL", "-o"]).arg(&script).arg("https://get.pharo.org/64/130+vm"), "download of get.pharo.org script (is curl installed?)")?;
        run(Command::new("bash").arg(&script).current_dir(&pharo_dir), "Pharo download (needs curl and unzip)")?;
    }
    let vm = find_vm(&pharo_dir).ok_or("Pharo VM not found after download")?;

    // 2. Sources
    let src = home.join("src");
    for (rel, content) in SOURCES {
        let path = src.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&path, content).map_err(|e| e.to_string())?;
    }
    std::fs::write(home.join("prepare.st"), PREPARE_ST).map_err(|e| e.to_string())?;

    // 3. Image = base image + StLive package
    let image = pharo_dir.join("stlive.image");
    std::fs::copy(pharo_dir.join("Pharo.image"), &image).map_err(|e| e.to_string())?;
    let _ = std::fs::copy(pharo_dir.join("Pharo.changes"), pharo_dir.join("stlive.changes"));
    eprintln!("stlive: loading the StLive package into the image ...");
    run(
        Command::new(&vm)
            .arg("--headless")
            .arg(&image)
            .arg("st")
            .arg(home.join("prepare.st"))
            .env("STLIVE_SRC", &src)
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
        "image preparation",
    )?;

    // 4. Remember
    let cfg = json!({"vm": vm, "image": image});
    std::fs::write(home.join("config.json"), cfg.to_string()).map_err(|e| e.to_string())?;
    Ok(json!({"home": home, "vm": vm, "template_image": image, "next": "stlive start"}))
}

/// Global template (vm + image) written by `init`, if present.
pub fn template(home: &Path) -> Option<(PathBuf, PathBuf)> {
    let t = std::fs::read_to_string(home.join("config.json")).ok()?;
    let v: Value = serde_json::from_str(&t).ok()?;
    Some((PathBuf::from(v["vm"].as_str()?), PathBuf::from(v["image"].as_str()?)))
}

/// Give an instance its own copy of the template image (so `image save` never touches the template).
pub fn instance_image(template_image: &Path, dest_dir: &Path, name: &str) -> Result<PathBuf, String> {
    let dest = dest_dir.join(format!("{}.image", name));
    if dest.exists() {
        return Ok(dest);
    }
    std::fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;
    std::fs::copy(template_image, &dest).map_err(|e| e.to_string())?;
    let _ = std::fs::copy(template_image.with_extension("changes"), dest.with_extension("changes"));
    // the sources file must sit next to the image
    if let Ok(rd) = std::fs::read_dir(template_image.parent().unwrap()) {
        for e in rd.flatten() {
            if e.path().extension().map(|x| x == "sources").unwrap_or(false) {
                let link = dest_dir.join(e.file_name());
                if !link.exists() {
                    let _ = std::os::unix::fs::symlink(e.path(), link);
                }
            }
        }
    }
    Ok(dest)
}
