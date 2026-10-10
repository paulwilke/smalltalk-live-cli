# Changelog

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
