# Architecture

```
agent / shell ──► stlive (Rust, short-lived) ──► 127.0.0.1:<port> ──► headless Pharo image (long-lived)
                  JSON line per request/response                       StLiveServer
                                                                        ├─ jobs      (one worker Process per evaluation)
                                                                        ├─ sessions  (suspended executions = debug sessions)
                                                                        └─ registry  (object references)
```

## Transport
One JSON object per line over a fresh TCP connection to loopback; one JSON line back. The port and a per-run *epoch* are written by the image to `<state>/<instance>.port.json`. Every response carries the epoch. TCP on loopback was chosen over Unix domain sockets because Pharo's socket support for the latter is thin and loopback needs no setup.

## Keeping a failed execution alive
Each evaluation (`eval`, one test, `debug eval`, `load`) runs in its own `Process` (`StLiveJob`) whose `errorHandler` is a `StLiveErrorHandler`. Pharo calls `Process>>handleError:` *on top of the failing stack* before anything is unwound. The handler registers a `StLiveSession` (wrapping a `DebugSession`), wakes the waiting request, and suspends the process. The stack, receivers and temporaries therefore stay intact and are reached later from any connection. Warnings and notifications keep their default behaviour. Timeouts suspend the running worker the same way (`kind: timeout`).

## Resume, restart, re-run
- `debug resume` – resumes the suspended process: the handler returns and execution continues with the *existing* frames (optionally answering a value for the failed send).
- `debug restart --frame n` – unwinds to that frame and restarts it; if the method was recompiled since, the frame is rewound onto the new `CompiledMethod` (`DebugSession>>rewindContextToMethod:fromContext:`).
- `debug return` – makes a frame return a value.
- `debug rerun` – executes the stored origin (`cmd` + `args`) again as a fresh job.

Only `rerun` produces `fresh_execution: true`; none of them is ever reported as a *verified* fix by the tool itself.

## Stale detection
A frame is `stale` when the method it runs is no longer the one installed in its class. `method compile` reports `stale_frames` across all suspended sessions.

## References
`obj-<epoch>-<n>`, `dbg-<epoch>-<n>`, `job-<epoch>-<n>`. Non-trivial objects are returned as summaries with a reference; strings up to 100 characters, numbers, booleans, nil, characters and symbols are inlined. At most 5000 object references and 60 sessions are kept (oldest evicted/terminated). Unknown or evicted references yield `unknown_ref`/`unknown_session`; references from earlier runs yield `stale_ref`.

## Output hygiene
A per-job `Transcript` tee captures text written by the worker; `Deprecation` warnings are collected and counted instead of cluttering output. Stacks are cut at the job boundary so tooling frames never appear; they are walked at most 100 000 deep.

## Writing back (`save`)
The image records every change made through the CLI (`changes.pending`). The CLI patches Tonel files per method (replace in place, append new, remove), detects the symbol/string Tonel style of the target directory, and marks changes as saved. Methods compiled with a `*Package` protocol become `Class.extension.st`.

## Layout
`src/` Rust client (`main.rs`, `save.rs`, `init.rs`) · `smalltalk/src/StLive` server (Commands, Server, Job, Session, ErrorHandler, Describer, TranscriptTee, Failure) · `tests/e2e.sh` · `skill/` agent instructions.
