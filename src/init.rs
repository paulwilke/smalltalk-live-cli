//! `stlive init`: download Pharo 13 (VM + image) and build an image containing the StLive server.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Stage 1 of `init` (needs network, runs once and is cached as neojson.image): JSON for applications. NeoJSON is MIT licensed (Sven Van Caekenberghe) and fetched from upstream, never bundled.
const NEOJSON_ST: &str = r#"[ Metacello new
	baseline: 'NeoJSON';
	repository: 'github://svenvc/NeoJSON:v18/repository';
	load: #( 'core' ) ]
	on: Error
	do: [ :e | Stdio stdout nextPutAll: 'stlive: NeoJSON not loaded: ' , e messageText asString; lf; flush ].
Smalltalk snapshot: true andQuit: true.
"#;

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
    ("StLive/StLiveBackgroundErrorHandler.class.st", include_str!("../smalltalk/src/StLive/StLiveBackgroundErrorHandler.class.st")),
    ("StLive/StLiveCommands.class.st", include_str!("../smalltalk/src/StLive/StLiveCommands.class.st")),
    ("StLive/StLiveDescriber.class.st", include_str!("../smalltalk/src/StLive/StLiveDescriber.class.st")),
    ("StLive/StLiveErrorHandler.class.st", include_str!("../smalltalk/src/StLive/StLiveErrorHandler.class.st")),
    ("StLive/StLiveFailure.class.st", include_str!("../smalltalk/src/StLive/StLiveFailure.class.st")),
    ("StLive/StLiveJob.class.st", include_str!("../smalltalk/src/StLive/StLiveJob.class.st")),
    ("StLive/StLiveServer.class.st", include_str!("../smalltalk/src/StLive/StLiveServer.class.st")),
    ("StLive/StLiveSession.class.st", include_str!("../smalltalk/src/StLive/StLiveSession.class.st")),
    ("StLive/StLiveRun.class.st", include_str!("../smalltalk/src/StLive/StLiveRun.class.st")),
    ("StLive/StLiveTranscriptTee.class.st", include_str!("../smalltalk/src/StLive/StLiveTranscriptTee.class.st")),
    ("StLive/StLiveTranscriptProxy.class.st", include_str!("../smalltalk/src/StLive/StLiveTranscriptProxy.class.st")),
    // the StLive-Tests package: loaded only by the Tests group (`stlive load StLive --group Tests`), never by init
    ("StLive-Tests/package.st", include_str!("../smalltalk/src/StLive-Tests/package.st")),
    ("StLive-Tests/StLiveTestCase.class.st", include_str!("../smalltalk/src/StLive-Tests/StLiveTestCase.class.st")),
    ("StLive-Tests/StLiveBaselinePackagesTest.class.st", include_str!("../smalltalk/src/StLive-Tests/StLiveBaselinePackagesTest.class.st")),
    ("StLive-Tests/StLiveBaselineShapeTest.class.st", include_str!("../smalltalk/src/StLive-Tests/StLiveBaselineShapeTest.class.st")),
    ("StLive-Tests/StLiveBaselineVersionTest.class.st", include_str!("../smalltalk/src/StLive-Tests/StLiveBaselineVersionTest.class.st")),
    ("StLive-Tests/StLiveChangeLogTest.class.st", include_str!("../smalltalk/src/StLive-Tests/StLiveChangeLogTest.class.st")),
    ("StLive-Tests/StLiveClassLookupTest.class.st", include_str!("../smalltalk/src/StLive-Tests/StLiveClassLookupTest.class.st")),
    ("StLive-Tests/StLiveCommandsHelpersTest.class.st", include_str!("../smalltalk/src/StLive-Tests/StLiveCommandsHelpersTest.class.st")),
    ("StLive-Tests/StLiveCommandsLineEndingsTest.class.st", include_str!("../smalltalk/src/StLive-Tests/StLiveCommandsLineEndingsTest.class.st")),
    ("StLive-Tests/StLiveCommentRecordingTest.class.st", include_str!("../smalltalk/src/StLive-Tests/StLiveCommentRecordingTest.class.st")),
    ("StLive-Tests/StLiveCompileErrorTest.class.st", include_str!("../smalltalk/src/StLive-Tests/StLiveCompileErrorTest.class.st")),
    ("StLive-Tests/StLiveDescriberTest.class.st", include_str!("../smalltalk/src/StLive-Tests/StLiveDescriberTest.class.st")),
    ("StLive-Tests/StLiveServerTest.class.st", include_str!("../smalltalk/src/StLive-Tests/StLiveServerTest.class.st")),
    ("StLive-Tests/StLiveSourceNormalisationTest.class.st", include_str!("../smalltalk/src/StLive-Tests/StLiveSourceNormalisationTest.class.st")),
    ("StLive-Tests/StLiveSuggestionsTest.class.st", include_str!("../smalltalk/src/StLive-Tests/StLiveSuggestionsTest.class.st")),
    ("StLive-Tests/StLiveUndeclaredReferencesTest.class.st", include_str!("../smalltalk/src/StLive-Tests/StLiveUndeclaredReferencesTest.class.st")),
];

/// Startup script run by every `start`: upgrades the server inside the image if it was built from other sources, then starts it.
/// The sources come from this binary's own directory, so the image may have registered the StLive baseline from another
/// one (the template, or an earlier binary): `onConflictUseIncoming` replaces that registration instead of refusing.
const UPGRADE_ST: &str = r#"| env hash current run code |
env := Smalltalk os environment.
hash := env at: 'STLIVE_SOURCES_HASH' ifAbsent: [ nil ].
current := (Smalltalk globals at: #StLiveServer ifAbsent: [ nil ]) ifNotNil: [ :c | (c respondsTo: #installedHash) ifTrue: [ c installedHash ] ].
(hash notNil and: [ hash ~= current and: [ (env at: 'STLIVE_SRC' ifAbsent: [ nil ]) notNil ] ]) ifTrue: [
	[ Metacello new baseline: 'StLive'; repository: 'tonel://' , (env at: 'STLIVE_SRC'); onConflictUseIncoming; load.
	(Smalltalk globals at: #StLiveServer) installedHash: hash ]
		on: Error
		do: [ :e | Stdio stdout nextPutAll: 'stlive: could not upgrade the server in this image: ' , e messageText asString; lf; flush ] ].
"#;

/// Startup script run by every `start`: upgrade (if needed), then start the server. Its lines (stdout) go to the
/// instance log: `server starting`, then `port file written` (from startFromEnvironment) or `server did not start`.
pub fn serve_st() -> String {
    format!("{}{}", UPGRADE_ST, SERVE_BODY_ST)
}

const SERVE_BODY_ST: &str = r#"Stdio stdout nextPutAll: 'stlive: server starting'; lf; flush.
[ (Smalltalk globals at: #StLiveServer) startFromEnvironment ]
	on: Error
	do: [ :e |
		Stdio stdout nextPutAll: 'stlive: server did not start: ' , e class name , ': ' , e messageText asString; lf; flush.
		Smalltalk exitFailure ].
"#;

/// Script of `stlive run`: upgrade (if needed), load sources, evaluate, write results to files.
pub fn run_st() -> String {
    format!("{}{}", UPGRADE_ST, RUN_BODY_ST)
}

const RUN_BODY_ST: &str = r#"run := Smalltalk globals at: #StLiveRun.
run beginIn: (env at: 'STLIVE_RUN_DIR').
[ [
	| packages |
	packages := (STON fromString: (env at: 'STLIVE_RUN_PACKAGES' ifAbsent: [ '[]' ])) asArray.
	(STON fromString: (env at: 'STLIVE_RUN_LOAD' ifAbsent: [ '[]' ])) do: [ :dir | run loadTonelFrom: dir packages: packages ].
	(env at: 'STLIVE_RUN_PREPARE' ifAbsent: [ nil ]) ifNotNil: [ :file | Smalltalk compiler evaluate: file asFileReference contents ].
	code := (env at: 'STLIVE_RUN_FILE' ifAbsent: [ nil ])
		ifNotNil: [ :file | file asFileReference contents ]
		ifNil: [ env at: 'STLIVE_RUN_EXPR' ].
	run result: (Smalltalk compiler evaluate: code) ]
	on: Error
	do: [ :e | run failed: e ] ] ensure: [ run finish ].
Smalltalk exit: 0.
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
    feed(NEOJSON_ST);
    feed(&serve_st());
    feed(&run_st());
    format!("{:016x}", h)
}

/// Where the template (VM, image, sources) lives: `STLIVE_HOME`; without it `STLIVE_DIR` (an environment that
/// sets only the state directory must never read or write `~/.stlive`); otherwise `~/.stlive`.
pub fn home_dir() -> PathBuf {
    home_dir_from(std::env::var("STLIVE_HOME").ok().as_deref(), std::env::var("STLIVE_DIR").ok().as_deref())
}

fn home_dir_from(stlive_home: Option<&str>, stlive_dir: Option<&str>) -> PathBuf {
    if let Some(h) = stlive_home.filter(|s| !s.trim().is_empty()) {
        return PathBuf::from(h);
    }
    if let Some(d) = stlive_dir.filter(|s| !s.trim().is_empty()) {
        return PathBuf::from(d);
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

/// Write the embedded Smalltalk sources below `dir`, one file per entry of SOURCES.
fn write_source_tree(dir: &Path) -> Result<(), String> {
    for (rel, content) in SOURCES {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&path, content).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Where this binary's sources are: `<home>/src-<sources hash>`. Every stlive version has its own directory, so two
/// binaries sharing one home never overwrite each other's sources (start, run and attach load from here).
pub fn sources_dir(home: &Path) -> PathBuf {
    home.join(format!("src-{}", sources_hash()))
}

/// Make sure `sources_dir(home)` holds this binary's sources and answer it. The tree is filled under a temporary name
/// and renamed into place, so a concurrent start never loads a half-written tree; an existing tree is never rewritten.
pub fn write_sources(home: &Path) -> Result<PathBuf, String> {
    let dir = sources_dir(home);
    if dir.is_dir() {
        return Ok(dir);
    }
    let name = dir.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let tmp = home.join(format!("{}.tmp-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    if let Err(e) = write_source_tree(&tmp) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    match std::fs::rename(&tmp, &dir) {
        Ok(()) => Ok(dir),
        // another process put its complete tree there first (same name, same content)
        Err(_) if dir.is_dir() => {
            let _ = std::fs::remove_dir_all(&tmp);
            Ok(dir)
        }
        Err(e) => Err(format!("{}: {}", dir.display(), e)),
    }
}

/// Names of the packages of a pristine image (written by `init`), for `attach`.
pub fn system_packages(home: &Path) -> Option<Vec<String>> {
    let t = std::fs::read_to_string(home.join("system-packages.txt")).ok()?;
    Some(t.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
}

/// `std::fs::copy`, but a no-op when source and destination are the same file (copying a file onto itself truncates it to 0 bytes).
fn copy_unless_same(src: &Path, dst: &Path) -> std::io::Result<()> {
    if let (Ok(a), Ok(b)) = (std::fs::canonicalize(src), std::fs::canonicalize(dst)) {
        if a == b {
            return Ok(());
        }
    }
    std::fs::copy(src, dst).map(|_| ())
}

pub fn init(home: &Path, force: bool, local: Option<(PathBuf, PathBuf)>) -> Result<Value, String> {
    let pharo_dir = home.join("pharo");
    std::fs::create_dir_all(&pharo_dir).map_err(|e| e.to_string())?;

    // 1. Pharo 13 VM + base image: a local Pharo (--vm/--image) or get.pharo.org
    if let Some((lvm, limg)) = &local {
        copy_unless_same(limg, &pharo_dir.join("Pharo.image")).map_err(|e| format!("{}: {}", limg.display(), e))?;
        let _ = copy_unless_same(&limg.with_extension("changes"), &pharo_dir.join("Pharo.changes"));
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
        eprintln!("stlive: downloading Pharo 13 (VM + image, ~100 MB) from get.pharo.org ...");
        crate::platform::download_pharo(&pharo_dir)?;
    }
    let vm = match &local {
        Some((lvm, _)) => std::fs::canonicalize(lvm).map_err(|e| format!("{}: {}", lvm.display(), e))?,
        None => find_vm(&pharo_dir).ok_or("Pharo VM not found after download")?,
    };

    // 2. Sources: the template's copy (home/src, the tree the template image was built from)
    let src = home.join("src");
    write_source_tree(&src)?;
    std::fs::write(home.join("prepare.st"), PREPARE_ST).map_err(|e| e.to_string())?;

    // 3a. Base image + NeoJSON, built once and cached (network only here; re-running `init` after an stlive update is offline)
    let base = pharo_dir.join("Pharo.image");
    let neo_image = pharo_dir.join("neojson.image");
    let marker = pharo_dir.join("neojson.ok");
    let base_id = std::fs::metadata(&base).map(|m| m.len().to_string()).unwrap_or_default();
    let cached = !force && neo_image.exists() && std::fs::read_to_string(&marker).map(|t| t.trim() == base_id).unwrap_or(false);
    let mut neo_warning: Option<String> = None;
    if !cached {
        let _ = std::fs::remove_file(&marker);
        std::fs::copy(&base, &neo_image).map_err(|e| e.to_string())?;
        let _ = std::fs::copy(pharo_dir.join("Pharo.changes"), pharo_dir.join("neojson.changes"));
        std::fs::write(home.join("neojson.st"), NEOJSON_ST).map_err(|e| e.to_string())?;
        eprintln!("stlive: loading NeoJSON (once; cached as {}) ...", neo_image.display());
        let _ = Command::new(&vm)
            .arg("--headless")
            .arg(&neo_image)
            .arg("st")
            .arg(home.join("neojson.st"))
            .stdout(std::fs::File::create(home.join("neojson.log")).map(Stdio::from).unwrap_or_else(|_| Stdio::null()))
            .stderr(Stdio::null())
            .status();
        let log = std::fs::read_to_string(home.join("neojson.log")).unwrap_or_default();
        match log.lines().find(|l| l.contains("stlive: NeoJSON not loaded")) {
            Some(w) => neo_warning = Some(w.to_string()),
            None => { let _ = std::fs::write(&marker, &base_id); }
        }
    }
    // 3b. Image = (base + NeoJSON) + StLive package
    let image = pharo_dir.join("stlive.image");
    std::fs::copy(&neo_image, &image).map_err(|e| e.to_string())?;
    let _ = std::fs::copy(pharo_dir.join(if neo_image.with_extension("changes").exists() { "neojson.changes" } else { "Pharo.changes" }), pharo_dir.join("stlive.changes"));
    eprintln!("stlive: loading the StLive package into the image ...");
    run(
        Command::new(&vm)
            .arg("--headless")
            .arg(&image)
            .arg("st")
            .arg(home.join("prepare.st"))
            .env("STLIVE_SRC", &src)
            .env("STLIVE_SOURCES_HASH", sources_hash())
            .stdout(std::fs::File::create(home.join("prepare.log")).map(Stdio::from).unwrap_or_else(|_| Stdio::null()))
            .stderr(Stdio::null()),
        "image preparation",
    )?;
    // A syntax error in the sources must not produce an "initialised" image without a working server.
    if let Ok(log) = std::fs::read_to_string(home.join("prepare.log")) {
        let bad: Vec<&str> = log.lines().filter(|l| l.contains("Syntax Error") || l.contains("Missing opener") || l.contains("did not understand") || l.contains("Error:") || l.contains("Undeclared")).take(5).collect();
        if !bad.is_empty() {
            return Err(format!("loading the StLive package into the image reported problems (see {}):\n{}", home.join("prepare.log").display(), bad.join("\n")));
        }
    }
    if let Some(w) = neo_warning {
        eprintln!("stlive: warning – {} (JSON for applications will not be available; run `stlive init` again with network access)", w);
    }

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
                    crate::platform::link_or_copy(&e.path(), &link);
                }
            }
        }
    }
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_prefers_stlive_home_then_stlive_dir_then_default() {
        assert_eq!(home_dir_from(Some("/h"), Some("/d")), PathBuf::from("/h"));
        assert_eq!(home_dir_from(None, Some("/d")), PathBuf::from("/d"));
        assert_eq!(home_dir_from(Some(""), Some("/d")), PathBuf::from("/d"));
        assert_eq!(home_dir_from(None, None), crate::platform::home().join(".stlive"));
        assert_eq!(home_dir_from(None, Some(" ")), crate::platform::home().join(".stlive"));
    }

    #[test]
    fn init_with_the_template_base_image_as_source_does_not_truncate_it() {
        // `stlive init --image <home>/pharo/Pharo.image` (the file init itself would write) must keep the image intact.
        let home = std::env::temp_dir().join(format!("stlive-init-self-{}", std::process::id()));
        let pharo = home.join("pharo");
        std::fs::create_dir_all(&pharo).unwrap();
        let base = pharo.join("Pharo.image");
        std::fs::write(&base, b"image bytes").unwrap();
        std::fs::write(pharo.join("Pharo.changes"), b"changes").unwrap();
        let vm = pharo.join("no-such-vm");
        // the VM does not exist, so init fails after the copy step; the base image must still be intact
        let r = init(&home, false, Some((vm, base.clone())));
        assert!(r.is_err());
        assert_eq!(std::fs::read(&base).unwrap(), b"image bytes");
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn sources_are_written_once_under_the_hash_of_the_binary() {
        let home = std::env::temp_dir().join(format!("stlive-src-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let dir = write_sources(&home).unwrap();
        assert_eq!(dir, home.join(format!("src-{}", sources_hash())));
        let written = std::fs::read_to_string(dir.join("StLive/StLiveCommands.class.st")).unwrap();
        assert_eq!(written, include_str!("../smalltalk/src/StLive/StLiveCommands.class.st"));
        assert!(dir.join("StLive-Tests/StLiveUndeclaredReferencesTest.class.st").exists());
        // the tree is in place: a second call answers the same directory and leaves it alone
        std::fs::write(dir.join("marker"), b"kept").unwrap();
        assert_eq!(write_sources(&home).unwrap(), dir);
        assert_eq!(std::fs::read(dir.join("marker")).unwrap(), b"kept");
        // no temporary tree is left next to it
        assert!(std::fs::read_dir(&home).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().contains(".tmp-")));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn copy_onto_itself_keeps_the_file() {
        let dir = std::env::temp_dir().join(format!("stlive-init-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("Pharo.image");
        std::fs::write(&f, b"image bytes").unwrap();
        copy_unless_same(&f, &f).unwrap();
        // the same file reached through another path (dir/./Pharo.image)
        copy_unless_same(&dir.join(".").join("Pharo.image"), &f).unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"image bytes");
        // different files are still copied
        let g = dir.join("copy.image");
        copy_unless_same(&f, &g).unwrap();
        assert_eq!(std::fs::read(&g).unwrap(), b"image bytes");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
