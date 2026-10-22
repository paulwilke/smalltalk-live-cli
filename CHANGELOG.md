# Changelog

## 0.8.1 – Windows fixes from the first Windows run
First successful runs of `init`, `start`, `load` and `test run` on Windows 11 (Pharo 13, VM 10.3.9), done by the Orbit project. Both fixes are Windows-only; Unix behaviour is unchanged.
- **Fix (Windows): `stlive start` hands the VM plain paths.** `std::fs::canonicalize` answers the extended-length form (`\\?\C:\...`) on Windows, which the Pharo VM does not open ("Image file not found"), so `start` ended with "the image ended before it published its port". `start`, `run` and `init --vm` now use `platform::absolute`, which strips the `\\?\` and `\\?\UNC\` prefixes.
- **Fix (Windows): the detached image no longer inherits stlive's own stdio handles.** A child created with redirected stdio inherited stlive's stdin/stdout/stderr when they were pipes, so a script reading the answer of `stlive start` through a pipe hung until the image ended. The standard handles are now made non-inheritable before the spawn.
- README: the Windows caveat now says what was verified (Windows 11 by the Orbit project: `init`, `start`, `load`, `eval`, tests) and what was not: GUI mode, the `ui` commands and `open-ui`, and the end-to-end suite on Windows.

## 0.8.0 – feedback from the Orbit rebuild
**Fresh runs and test sets**
- **`stlive test run` takes several targets and glob patterns**: `stlive test run 'Orbit-*-Tests' Other-Tests 'Foo>>testBar'` runs them as one run (each test once); `*`/`?` match packages and test class names; no match → `no_tests_matched`. The result lists `targets` and `expanded`.
- **`stlive test run --fresh --load <Baseline> --repository tonel://… [--group G] [targets]`**: a temporary instance from the template, the baseline loaded, the tests run, the instance stopped and deleted – the user's own instance is never touched. Without targets it runs the `*-Tests` packages the load added. A failed load (error, warnings or undeclared names) exits **6** (`load_failed`); `--keep` leaves the instance running to debug red tests (`result.instance`). Ctrl-C / SIGTERM stop and delete the temporary instance (exit **130** / **143**); `stlive instances` shows `temporary` / `orphaned`, `stlive instances --prune-temporary` removes orphans.
- **`load` reports undeclared names**: `undeclared` lists names that methods of the loaded packages reference but nothing defines, with the referencing methods (Pharo 13 registers them silently; they fail only at run time). Plain `load` still exits 0.

**Saved images**
- **Fix: a saved image did not come up again** (seen on macOS: 100 % CPU, no port file, even `Pharo saved.image eval "1+1"` hung). The acceptor process was saved with the image and, after the next start, looped on the dead listener at priority 50, starving the startup. The server now hooks into `SessionManager`: the acceptor ends before every snapshot (`image save`, `image export --keep-server`, a plain `Smalltalk snapshot:`), the earlier session's listener and processes are dropped after a restart, and the accept loop never spins. An image saved by stlive ≤ 0.7.x may already contain a spinning acceptor that no start can repair: delete that instance's image (`.stlive/<instance>.image`), `start` rebuilds it from the template.
- The instance log says what happened: `stlive: server starting`, `stlive: port file written …`, `stlive: server did not start: …`; when `start` times out it appends `port file not written within 60 s` and shows the last log lines.

**Reloading, saving and drift** (planned as 0.7.1, released here)
- **`save` writes a changed class comment** of an existing class back to its Tonel file (replaced, added or removed); comment changes made through `eval` (`Foo comment: '…'`) are recorded.
- **`method compile` compiles with CR line endings**, like the Tonel loader: a multi-line string literal now holds the same characters whether the method was compiled through stlive or loaded from files. Recorded source, diffs, `method show` and error positions stay LF.
- **Fix: a method whose body starts on its header line** (`Foo >> baseline: spec [ <baseline>`) was read with a wrong selector (`baseline:[`): `drift` reported drift without any change and `save` appended a duplicate instead of replacing it.
- **`load` reports what it does not touch**: `stale_packages` (loaded packages the baseline no longer lists), `not_reloaded_packages` (loaded packages outside the requested groups, with a `--group` hint). `load --prune` unloads the stale ones; packages with unsaved changes or listed by another loaded baseline stay (`prune_skipped`). `drift --all` reports packages a baseline in `--dir` lists that are loaded but have no folder (`packages_missing_in_files`).
- `drift` and `save` read methods written on one line (`Foo >> one [ ^ 1 ]`); before, drift reported them as missing in files and `save` could not replace them.
- A package a baseline dropped stays in `stale_packages` across `image save` and a restart until `load --prune` removes it.

**Internals and tests**
- Each stlive binary writes its embedded Smalltalk sources to its own `~/.stlive/src-<hash>` (reported by `start` as `sources`), so two stlive versions sharing one home no longer overwrite each other's sources. To load StLive's own `Tests` group into an instance, use that `sources` directory.
- **SUnit tests for the server** (`StLive-Tests`, 150 tests, group `Tests` of `BaselineOfStLive`, not loaded by `init`, removed by `image export`), run by the e2e suite through `stlive test run StLive-Tests`.
- New dependency: `signal-hook` (MIT/Apache-2.0).
- Skill: `Semaphore>>waitTimeoutSeconds:` answers `true` on timeout; string literals loaded from Tonel hold CR; `test run --fresh`, patterns, `undeclared`.
- 203 headless e2e checks and 36 Rust unit tests, run on Linux x86-64. The 19 GUI checks were not re-run for this release (no display in the build environment).

## 0.7.0 – gaps found while rebuilding a multi-module system with stlive
- **`STLIVE_DIR` is the home when `STLIVE_HOME` is unset.** An environment that only sets the state directory no longer clones from, or `init`s into, `~/.stlive` by accident; it gets `not_initialized` until `stlive init` runs there.
- **Traits** in `save` and `drift`: trait definitions (`Name.trait.st`), trait methods on both sides (`TFoo classSide >> sel`), classes using traits (`#traits`/`#classTraits` are written and kept when a definition is patched). A method a class gets from a trait is no longer reported as drift (`package.methods` lists local methods only). `class create --trait`, `class create --traits 'TFoo + TBar'`; `class show` reports `kind` and `traits`.
- **`test run` runs what SUnit runs**: the test cases of `buildSuite` – inherited test methods of an abstract superclass, an abstract class runs its concrete subclasses, and a `ParametrizedTestCase` runs once per parameter combination (previously only the first set): cases are reported as `Class>>selector (#param->value)`, `Class>>selector` runs every combination of that selector and `Class>>selector (#param->value)` one of them (also via `debug rerun`); `test list` counts the same way.
- **Skipped tests** (`self skip`) are reported as `skipped` / `skipped_tests`, neither passed nor failed; `test run --skips-fail` exits 1 when tests were skipped (for projects whose policy treats skips as failures).
- **Fix: `test run` failed with `unknown_session` when more than 60 tests failed.** The image keeps 60 sessions; the oldest were evicted while the run was still going and the final grouping tried to end them. Non-representative sessions now end as soon as their failure group is known, evicted ones are reported as `sessions_ended`. The socket budget of `test run` is at least 10 minutes (large suites).
- **`load` re-loads properly**: (1) a package modified in the image (methods compiled through stlive) is overwritten by the files – Monticello's merge-or-load question used to be answered with "cancel" silently; the result lists `modified_packages_overwritten` and marks the superseded recorded changes as saved (`changes_superseded`) so `save` cannot write old code back; (2) the `BaselineOf` package itself is reloaded from the repository when its class already exists (Metacello never does), so new packages, groups and dependencies in a changed baseline take effect (`baseline_package: reloaded`).
- **A headless image survives unhandled errors in forked processes.** Pharo's default handler for processes without their own (the NonInteractiveUIManager) quits the image. While the server runs, `StLiveBackgroundErrorHandler` logs the error with ten frames to the instance log, keeps the last 50 under `image info` → `background_errors` (`status` → count) and terminates only the failing process. GUI images keep their debugger. `image export` restores Pharo's handler.
- Rust unit tests (`cargo test`) for the home directory rule, the `init --image` self-copy fix, trait handling in `save`/`drift` and the exit code of skipped tests; e2e checks for every item above.

## 0.6.1
- **NeoJSON in the template image**: `stlive init` loads NeoJSON (core, pinned to v18, MIT licensed, from upstream – not bundled) so applications and `stlive run` have `NeoJSONWriter`/`NeoJSONReader` (it writes an `OrderedCollection`, and shared literals such as `#()` do not break it). `init` warns instead of failing when the download is not possible. Re-run `stlive init` to get it into an existing template.
- **Fetched once, then cached**: NeoJSON is loaded into a cached base image (`~/.stlive/pharo/neojson.image`) the first time; later `stlive init` runs (for example after an stlive update) reuse it and need no network. `run`, `start` and every instance only copy the template image – nothing is downloaded per call. `init --force` refreshes the cache.
- **Every command is logged** with `--log`: `run`, `save`, `drift`, `start`, `stop`, `init`, `attach`, `image export` … (not only the ones that talk to the image), including unreachable-instance failures.
- Skill: `save … --verify` is now the fixed last step; `stlive run` exit codes and a start-script example (`StLiveRun arguments`, `exit:`, `--prepare`); STON "shared reference" tip; wrong test expectation → fix the test, open a session only for unexpected behaviour.
- README: third-party notices (Pharo, NeoJSON).


## 0.6.0 – from pilot runs
- **Fix: changes made by `eval` were silently not recorded** after a test run that left a failing test suspended. Recording was suppressed globally while a job ran inside the suppressed block; the suppression is now per process.
- `stlive drift --package X|--all --dir src`: compares the image with the Tonel files (methods and classes missing on either side, methods whose source differs; exit code 1 on drift). `stlive save … --verify` runs it after writing. `save` now **warns** (and exits 1) about recorded changes it could not write (for example a class without a package) instead of staying silent.
- `stlive run --load <tonel dir> --eval '<expr>' -- <args>`: one-shot headless run of a program from Tonel sources in a **fresh image**. stdout carries only the result (Strings raw, UTF-8 intact; Pharo's own console noise never reaches it), errors and stack go to stderr, exit code is passed through (`StLiveRun exit: n`), arguments arrive via `StLiveRun arguments`, `StLiveRun stdout:`/`stderr:` for explicit output; `--file`, `--prepare`, `--package`, `--timeout`, `--keep`.
- Failed sends now carry `did_you_mean`: existing selectors of the receiver that look like the one that was not understood (typos, wrong keyword names, wrong arity).
- Truncated `print` values carry a `hint` how to get the full text (`eval --full`, `obj text <ref>`).
- The call log of `test run` now has `sessions` (the debug sessions of the red tests) and `groups` (cause, count, frame).
- `init` fails loudly when loading the StLive package into the image reports problems (it used to report success).


## 0.5.0
- **The server upgrades itself.** `stlive start` compares a hash of the embedded Smalltalk sources with the one recorded in the image and, if they differ (an image built by an older stlive – the template *and* the per-project copies), loads the current server into the image before starting it. Previously a new binary silently kept using an old server (for instance the old Transcript failure in GUI mode) until `stlive init --force`.
- `stlive image export <file.image> --force`: save a copy of the image **for delivery without the stlive server** (sessions ended, listener stopped, Transcript restored, `StLive` packages removed) – the delivered image has no open evaluation port; the `.changes` and `.sources` files are written next to it. This instance ends.
- `stlive --version` now matches the release.


## 0.4.0 – live development with a window
- `stlive start --gui`: Pharo starts **with** its window (Morphic/Spec now, Bloc/Toplo later) and stays connected; port file, pid, log, instance state and image copy work exactly as in headless mode.
- `eval --ui`, `debug eval --ui`, `method compile --ui`: run in Morphic's UI process, so Spec/Morphic changes are laid out and drawn safely.
- Errors in the UI process become stlive debug sessions, never a Pharo debugger window. Like Pharo's own debugger, stlive first starts a fresh UI process (so the GUI keeps running) and then suspends the failed one; after `resume` the old UI process ends itself.
- `ui windows`, `ui screenshot [--window n|title] [--out file]` (PNG), `ui press <button label|presenter variable>`: the loop "change → look → check" without external tools. `ui press` runs the click in the UI process under stlive's error handling.
- Transcript capture no longer breaks the server start: it adapts to the Transcript type (GUI images use a forwarding proxy) and can never prevent the server from starting.
- Warnings raised by stlive evaluations no longer open dialogs; they are reported under `output.warnings`.
- `image save` reports suspended sessions (they cannot survive a restart); `stop` needs no dialog in GUI mode.
- `stlive open-ui <url>`: chromeless app window (Chrome `--app` on macOS/Linux, Edge on Windows) for web UIs.
- Windows: process handling, links, home directory, `Pharo.exe` and the download were separated into `platform.rs` and compile for the Windows target; **not yet run on a Windows machine**.
- A deleted instance image is rebuilt from the template on the next `start`.


## 0.3.1
- `stlive start` writes `.stlive/.gitignore` (`*`) so the state directory – including Pharo's `pharo-local/ombu-sessions` – is never committed by accident.

## 0.3.0
- `stlive attach --port N`: load the server into an image that is already running (started by an older tool or any JSON-line server), no restart needed.
- `stlive init --vm <Pharo> --image <Pharo.image>`: use a local Pharo instead of downloading.
- Changes made by `eval` or the IDE (e.g. `compile:`, adding instance variables) are now recorded like CLI changes (`origin: "eval"`) and written by `save`; `save` also updates the variable lists of an existing class definition. Bulk loads and test runs are not recorded.
- `changes list --since N` and `changes watch` (JSON lines stream of new changes).
- Call log lines carry a 60-character `summary` of the call.
- `eval --no-source` drops statement text from error frames; frame statements are capped at 160 characters.
- Error messages for `method remove` and `stop` name `--force`.
- `stop` no longer gives up when the dying image does not answer.

## 0.2.0
- Call log: `--log` / `STLIVE_LOG`, `--tag` / `STLIVE_TAG` (metadata only).
- `method compile` errors carry the undeclared variable, line and column (previously an empty message).
- `load` reports `warnings`, `classes_per_package` and `empty_packages` (a package that loaded without classes).
- `eval --full`, `debug eval --full` (complete text), `eval --no-session` (no suspended debug state after a failure).
- `debug` commands accept the frame positionally or as `--frame` everywhere.
- `class create --class-ivars --class-vars --comment`; `save` writes `#classVars`, `#classInstVars` and the class comment.
- New `obj referrers` and `obj graph` (identity, shared nodes, cycles).
- `image info` shows `changes_by_package`.
- Fix: `changes.mark_saved` was never dispatched (underscore in command name), so saved changes stayed pending.

## 0.1.0
First public version.
- Persistent headless Pharo 13 image with a loopback JSON-line server; `stlive init/start/stop/status/instances/image clone`.
- `eval` with per-job Transcript/deprecation capture, timeouts that become sessions.
- Debug sessions that keep failed executions alive: frames, variables (argument/temporary/block kinds), receiver, evaluation inside a frame, `resume`/`restart`/`return`/`rerun`, stale-frame detection, recursion detection.
- Object inspection by reference with paging; code browsing; method compile/remove; recorded changes with unified diff.
- Tests with grouped failures; Metacello baseline loading.
- `save`: minimal-diff write-back to Tonel (classes, methods, class side, extensions, removals).
