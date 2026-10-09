# Changelog

## 0.1.0
First public version.
- Persistent headless Pharo 13 image with a loopback JSON-line server; `stlive init/start/stop/status/instances/image clone`.
- `eval` with per-job Transcript/deprecation capture, timeouts that become sessions.
- Debug sessions that keep failed executions alive: frames, variables (argument/temporary/block kinds), receiver, evaluation inside a frame, `resume`/`restart`/`return`/`rerun`, stale-frame detection, recursion detection.
- Object inspection by reference with paging; code browsing; method compile/remove; recorded changes with unified diff.
- Tests with grouped failures; Metacello baseline loading.
- `save`: minimal-diff write-back to Tonel (classes, methods, class side, extensions, removals).
