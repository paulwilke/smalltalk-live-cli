---
name: stlive
description: Work inside a running Pharo (Smalltalk) image with the `stlive` CLI – evaluate code, inspect live objects, debug failing code frame by frame, fix methods in the running image, re-run, run tests and write the fix back to Tonel files. Use for Pharo/Smalltalk tasks where the behaviour of running code matters (failing tests, wrong results, unknown library internals).
---

# stlive – live development in a Pharo image

Every command prints JSON: `{"ok":true,"result":...}` or `{"ok":false,"error":...}` (use `--text` for a readable form). Use `jq` to pick fields; never dump big results – ask for the part you need. Exit code 0 = ok, 1 = failure/exception/failing tests, 3 = image not running.

## Setup
```bash
stlive init                                   # once per machine: Pharo 13 + image with the server (~/.stlive)
stlive start                                  # in the project directory; state in ./.stlive
stlive load <Baseline> --repository tonel://$PWD/src --group Tests     # Metacello; reports new packages
stlive image info                             # version, loaded packages, unsaved changes, sessions
```
`--instance NAME` runs several images side by side. References (`obj-…`) and sessions (`dbg-…`) die with the image process (`stale_ref` after a restart).

## The loop
1. **Reproduce** with `eval` or `test run`. A failure is not a tool error: it becomes a **debug session** that keeps the stack alive.
   - `stlive eval '<code>'` (`-` reads stdin; `--timeout ms` turns endless loops into a session; `--in <ref>` sets `self`). Result: `result.value.{class,print,ref}`, `result.output.{transcript,deprecations}`. Failure: `error.{type,message,session,first_application_frame}` (and `error.recursion` for endless recursion).
   - `stlive test run <TestClass | Class>>test | Package>` → `result.{ran,passed,failed}` and `result.groups[]` (failures grouped by cause, first application frame, one representative session each). `stlive test list <glob>` finds test classes.
2. **Navigate in small steps** (never print the whole stack):
   - `debug frames <session> --limit 5` → `origin` is application/framework/eval; start at `first_application_frame`.
   - `debug frame <session> <n> [--no-source]` → executing statement, receiver, variables with `kind` (argument, temporary, block_argument, block_temporary, outer_block_*).
   - `debug eval <session> --frame <n> '<expr>'` evaluates with that frame's variables visible – prefer experiments over guessing.
   - `debug receiver <session> --frame n`, `obj show|items|var|text|compare <ref>` follow references; collections and long strings are paged.
3. **Understand the code**: `method show 'Class>>selector'`, `class show Foo`, `find implementors|senders <selector>`, `find class '<glob>'`, `package list '<glob>'`.
4. **Hypothesis → small experiment → change the method**: `method compile ClassName - <<'EOF' … EOF` (also `--file`, `--source`). An existing method keeps its protocol; a NEW method needs `--protocol <name>` (`'*Pkg'` for an extension of package Pkg). The result lists `stale_frames`: suspended executions still running the old code.
5. **Re-run correctly.** `debug resume` continues the OLD execution and proves nothing. Use `debug rerun <session>` (fresh execution of the original operation with the current code) or `debug restart <session> --frame n` (only when restarting that frame is semantically right). Then run the tests.
6. **Verify**: write a regression test, **see it fail first** (`test run`), then pass after the fix; also run neighbouring tests. Check the restored objects, not only a green bar. If a test fails because its *expectation* is wrong, correct the test; open a debug session only when the code behaves unexpectedly.
7. **Persist**: changes made through the CLI live only in the image. `changes list|show` (unified diff), then `save --package <Pkg> --dir <repo>/src [--dry-run]` or `save --all --dir <repo>/src`. Packages come from `changes list` (`unsaved_by_package`) or a frame's `package`. Check `git diff` – it should be minimal – and commit with git.
8. **Finish**: the last command before you report done is `stlive save --all --dir <repo>/src --verify` (or `stlive drift --all --dir <repo>/src`). **Exit 1 means drift** – the image has methods or classes the files lack (or the reverse); fix it, otherwise a fresh image will behave differently. Then `debug terminate --all` (sessions of failing tests stay suspended; max 60 are kept).

## GUI development (Spec2/Morphic windows)

Start the image with `stlive start --gui`; Pharo opens its window and stays connected. The loop is *change → look → check*:

1. Run anything that touches widgets with `--ui` (`eval --ui`, `method compile --ui`, `debug eval --ui`): it runs in the UI process, so layout and drawing stay safe. Without `--ui` an evaluation runs in a worker process – fine for data, not for widgets.
2. **Look**: `stlive ui windows` (titles, presenter classes), then `stlive ui screenshot --window <title>` writes a PNG – open it and check the result visually instead of guessing.
3. **Check behaviour**: `stlive ui press <button label>` clicks a button; read state back with `eval`.
4. Errors in the UI become ordinary debug sessions (no Pharo debugger window opens, the GUI keeps running): debug and fix them with the normal loop, then redo the action. `ui_unavailable` means the UI process was busy or a modal dialog is open – close the dialog or wait.
5. Errors that stlive did not start (a person clicking) still open Pharo's own debugger; that is expected.

For web UIs run `stlive open-ui <url>` (chromeless Chrome/Edge window) instead – no GUI mode needed.

## Practical tips
- Before you finish: `stlive save --all --dir <repo>/src --verify` (or `stlive drift --all --dir <repo>/src`). It compares the image with the Tonel files and fails if a method exists only in the image – changes that were not recorded would otherwise only show up in a fresh image (red tests).
- To run the finished program from the sources in a clean image: `stlive run --load <repo>/src --eval 'MyApp run' -- arg1 arg2` (stdout = only the result, errors on stderr, `StLiveRun arguments` for the arguments, `StLiveRun exit: n` for the exit code). Do not write your own loader script.
  Example – a start script (the Smalltalk goes into a file, so shell quoting cannot mangle string literals):
  ```bash
  # start.st
  | args |
  args := StLiveRun arguments.
  args isEmpty ifTrue: [ StLiveRun stderr: 'usage: app <file>'. StLiveRun exit: 2 ].
  MyApp new run: args first

  # app.sh
  exec stlive run --load src --package MyApp --package MyApp-Tools --prepare deps.st --file start.st -- "$@"
  ```
  `--prepare deps.st` is a Smalltalk file evaluated after loading (for example a Metacello load of another library); `--package` fixes the load order; the value of the expression is the program's stdout.
- Non-ASCII output: in `stlive run` use the expression value or `StLiveRun stdout:` (UTF-8 is handled). Writing to `Stdio stdout` directly is not reliable for non-ASCII characters.
- JSON: NeoJSON (MIT, by Sven Van Caekenberghe) is loaded by `stlive init`: `NeoJSONWriter toString: anObject`, `NeoJSONReader fromString: s`. Re-run `stlive init` for images built before 0.6.1 (needs network once). `STONJSON` is also there but cannot write an `OrderedCollection`.
- `shared reference detected` (STON): the same literal such as `#()` or `''` used in several fields of one structure breaks STON serialisation. Use `Array new`, fresh objects, or NeoJSON. Exercise the error paths of your program, not only the good ones: that is where it shows.
- `stlive run` exit codes: 0 ok, 1 unhandled exception in your expression (message and stack on stderr), the code you pass to `StLiveRun exit: n`, 124 timeout, 2 usage error (e.g. an unknown stlive option before `--`). Everything after `--` belongs to your program.
- A `MessageNotUnderstood` error lists `did_you_mean` – real selectors of the receiver. Use them instead of guessing from memory; `find implementors <selector>` shows where a selector lives.
- A `value.hint` means the printed value was cut; use `eval --full` or `obj text <ref>`.
- Quoting: code with Smalltalk string literals (`'…'`) inside a single-quoted shell argument loses its quotes; use stdin: `stlive eval - <<'EOF' … EOF`.
- `print` fields are truncated at 120 characters; get the full text with `obj text <ref>` or page with `obj items`.
- Endless recursion: a timeout session; read `error.recursion.cycle` to see the repeating frames.
- Output of `Transcript show:` and deprecation warnings arrive in `result.output`, not mixed into values.
- `eval --no-session` for exploratory snippets so typos do not leave suspended sessions; `eval --full` for the complete text of a value.
- `obj graph <ref> --depth 3` shows identity, shared references and cycles; `obj referrers <ref>` answers who points to an object.
- `stlive image export <file> --force` ships a copy of the image without the stlive server.
- `--log file --tag name` records a call log (metadata plus a 60-character summary). `changes watch` streams new changes.
- Edits made with `eval` (`compile:`, new instance variables, ...) are recorded too and included by `save`.
- `image stop`/`stop --force` and `method remove --force` are destructive – only when asked.

## Report
State the cause, the minimal diff, which tests ran (counts), and what you could not verify.
