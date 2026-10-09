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
6. **Verify**: write a regression test, **see it fail first** (`test run`), then pass after the fix; also run neighbouring tests. Check the restored objects, not only a green bar.
7. **Persist**: changes made through the CLI live only in the image. `changes list|show` (unified diff), then `save --package <Pkg> --dir <repo>/src [--dry-run]` or `save --all --dir <repo>/src`. Packages come from `changes list` (`unsaved_by_package`) or a frame's `package`. Check `git diff` – it should be minimal – and commit with git.
8. **Clean up**: `debug terminate --all` (sessions of failing tests stay suspended; max 60 are kept).

## Practical tips
- Quoting: code with Smalltalk string literals (`'…'`) inside a single-quoted shell argument loses its quotes; use stdin: `stlive eval - <<'EOF' … EOF`.
- `print` fields are truncated at 120 characters; get the full text with `obj text <ref>` or page with `obj items`.
- Endless recursion: a timeout session; read `error.recursion.cycle` to see the repeating frames.
- Output of `Transcript show:` and deprecation warnings arrive in `result.output`, not mixed into values.
- `eval --no-session` for exploratory snippets so typos do not leave suspended sessions; `eval --full` for the complete text of a value.
- `obj graph <ref> --depth 3` shows identity, shared references and cycles; `obj referrers <ref>` answers who points to an object.
- `--log file --tag name` records a call log (metadata plus a 60-character summary). `changes watch` streams new changes.
- Edits made with `eval` (`compile:`, new instance variables, ...) are recorded too and included by `save`.
- `image stop`/`stop --force` and `method remove --force` are destructive – only when asked.

## Report
State the cause, the minimal diff, which tests ran (counts), and what you could not verify.
