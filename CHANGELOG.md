# Changelog

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
