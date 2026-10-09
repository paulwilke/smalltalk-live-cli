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
(Smalltalk globals at: #StLiveServer) installedHash: (Smalltalk os environment at: 'STLIVE_SOURCES_HASH' ifAbsent: [ nil ]).
(src parent / 'system-packages.txt') ensureDelete.
(src parent / 'system-packages.txt') writeStreamDo: [ :out |
	PackageOrganizer default packages do: [ :pk | out nextPutAll: pk name; lf ] ].
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
    ("StLive/StLiveTranscriptProxy.class.st", include_str!("../smalltalk/src/StLive/StLiveTranscriptProxy.class.st")),
];

/// Startup script run by every `start`: upgrades the server inside the image if it was built from other sources, then starts it.
pub const SERVE_ST: &str = r#"| env hash current |
env := Smalltalk os environment.
hash := env at: 'STLIVE_SOURCES_HASH' ifAbsent: [ nil ].
current := (Smalltalk globals at: #StLiveServer ifAbsent: [ nil ]) ifNotNil: [ :c | (c respondsTo: #installedHash) ifTrue: [ c installedHash ] ].
(hash notNil and: [ hash ~= current and: [ (env at: 'STLIVE_SRC' ifAbsent: [ nil ]) notNil ] ]) ifTrue: [
	[ Metacello new baseline: 'StLive'; repository: 'tonel://' , (env at: 'STLIVE_SRC'); load.
	(Smalltalk globals at: #StLiveServer) installedHash: hash ]
		on: Error
		do: [ :e | Stdio stdout nextPutAll: 'stlive: could not upgrade the server in this image: ' , e messageText asString; lf; flush ] ].
(Smalltalk globals at: #StLiveServer) startFromEnvironment.
"#;

/// FNV-1a hash (hex) of everything that ends up inside the image: the Smalltalk sources and the scripts.
pub fn sources_hash() -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut feed = |s: &str| {
        for b in s.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    };
    for (name, content) in SOURCES {
        feed(name);
        feed(content);
    }
    feed(PREPARE_ST);
    feed(SERVE_ST);
    format!("{:016x}", h)
}

pub fn home_dir() -> PathBuf {
    if let Ok(h) = std::env::var("STLIVE_HOME") {
        return PathBuf::from(h);
    }
    crate::platform::home().join(".stlive")
}

fn find_vm(pharo_dir: &Path) -> Option<PathBuf> {
    crate::platform::vm_candidates()
        .iter()
        .map(|p| pharo_dir.join(p))
        .find(|p| p.exists())
}

fn run(cmd: &mut Command, what: &str) -> Result<(), String> {
    let status = cmd.status().map_err(|e| format!("{}: {}", what, e))?;
    if status.success() { Ok(()) } else { Err(format!("{} failed ({})", what, status)) }
}

/// Write the embedded Smalltalk sources below `<home>/src` (used by init and attach).
pub fn write_sources(home: &Path) -> Result<PathBuf, String> {
    let src = home.join("src");
    for (rel, content) in SOURCES {
        let path = src.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&path, content).map_err(|e| e.to_string())?;
    }
    Ok(src)
}

/// Names of the packages of a pristine image (written by `init`), for `attach`.
pub fn system_packages(home: &Path) -> Option<Vec<String>> {
    let t = std::fs::read_to_string(home.join("system-packages.txt")).ok()?;
    Some(t.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
}

pub fn init(home: &Path, force: bool, local: Option<(PathBuf, PathBuf)>, version: u32) -> Result<Value, String> {
    let pharo_dir = version_dir(home, version);
    std::fs::create_dir_all(&pharo_dir).map_err(|e| e.to_string())?;

    // 1. Pharo 13 VM + base image: a local Pharo (--vm/--image) or get.pharo.org
    if let Some((lvm, limg)) = &local {
        std::fs::copy(limg, pharo_dir.join("Pharo.image")).map_err(|e| format!("{}: {}", limg.display(), e))?;
        let _ = std::fs::copy(limg.with_extension("changes"), pharo_dir.join("Pharo.changes"));
        // the sources file must sit next to the image
        if let Ok(rd) = std::fs::read_dir(limg.parent().unwrap()) {
            for e in rd.flatten() {
                if e.path().extension().map(|x| x == "sources").unwrap_or(false) {
                    let link = pharo_dir.join(e.file_name());
                    if !link.exists() { crate::platform::link_or_copy(&e.path(), &link); }
                }
            }
        }
        let _ = lvm;
    } else if force || !pharo_dir.join("Pharo.image").exists() || find_vm(&pharo_dir).is_none() {
        eprintln!("stlive: downloading Pharo {} (VM + image, ~100 MB) from get.pharo.org ...", version);
        crate::platform::download_pharo(&pharo_dir, version)?;
    }
    let vm = match &local {
        Some((lvm, _)) => std::fs::canonicalize(lvm).map_err(|e| format!("{}: {}", lvm.display(), e))?,
        None => find_vm(&pharo_dir).ok_or("Pharo VM not found after download")?,
    };

    // 2. Sources
    let src = write_sources(home)?;
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
            .env("STLIVE_SOURCES_HASH", sources_hash())
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
        "image preparation",
    )?;

    // 4. Remember
    let cfg = json!({"vm": vm, "image": image});
    std::fs::write(version_config(home, version), cfg.to_string()).map_err(|e| e.to_string())?;
    Ok(json!({"home": home, "vm": vm, "template_image": image, "next": "stlive start"}))
}

/// Global template (vm + image) written by `init`, if present.
pub const DEFAULT_PHARO: u32 = 13;

fn version_dir(home: &Path, version: u32) -> PathBuf {
    if version == DEFAULT_PHARO { home.join("pharo") } else { home.join(format!("pharo{}", version)) }
}

fn version_config(home: &Path, version: u32) -> PathBuf {
    if version == DEFAULT_PHARO { home.join("config.json") } else { home.join(format!("config-{}.json", version)) }
}

pub fn template(home: &Path, version: u32) -> Option<(PathBuf, PathBuf)> {
    let t = std::fs::read_to_string(version_config(home, version)).ok()?;
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
                    crate::platform::link_or_copy(&e.path(), &link);
                }
            }
        }
    }
    Ok(dest)
}
