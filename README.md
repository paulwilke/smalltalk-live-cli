# stlive – a live-debugging CLI for Smalltalk (Pharo)

**Give a coding agent (or yourself, from a shell) the thing that makes Smalltalk special: a running system whose failed executions you can open up, inspect, repair and continue – without restarting anything.**

`stlive` talks to a long-running headless [Pharo](https://pharo.org) image. A failed `eval` or test does not produce a stack-trace string; it leaves a *live debug session* in the image that you can query across many short CLI calls – frame by frame, object by object – then fix the method in the running image, re-run the original operation, and finally write the change back to Tonel files with a minimal `git diff`.

```text
$ stlive eval - <<< "| c | c := Cart new. c setUp. c add: 10. c total"
{"ok":false,"error":{"type":"MessageNotUnderstood","session":"dbg-3fa-1",
  "message":"receiver of \"adaptToNumber:andSend:\" is nil","first_application_frame":3, ...}}

$ stlive debug frames dbg-3fa-1 --limit 4 | jq -c '.result.frames[]|[.index,.label,.origin]'
[0,"UndefinedObject(Object)>>#adaptToInteger:andSend:","framework"]
[1,"SmallInteger(Integer)>>#+","framework"]
[2,"SmallInteger>>#+","framework"]
[3,"Cart>>#total","application"]

$ stlive debug frame dbg-3fa-1 3 --no-source | jq -c '.result|{statement,variables}'
{"statement":"^ sum * (1 + taxRate)","variables":[{"name":"sum","print":"15","kind":"temporary"}]}

$ stlive method compile Cart - <<< $'total\n\t| sum |\n\t...\n\t^ sum * (1 + (taxRate ifNil: [ 0 ]))'
{"ok":true,"result":{"selector":"total","stale_frames":[{"session":"dbg-3fa-1","frame":3}]}}

$ stlive debug resume dbg-3fa-1     # continues the OLD execution – never claims a repair
{"ok":false,"mode":"resume","error":{ ... "continued_from":"dbg-3fa-1"}}

$ stlive debug rerun dbg-3fa-1      # the original operation, from scratch, with the new code
{"ok":true,"result":{"mode":"rerun","fresh_execution":true,"value":{"class":"SmallInteger","print":"15"}}}

$ stlive save --package Demo --dir src && git diff --stat
```

## Why

Classic agent loops are *edit file → restart process → read a log*. In a live Smalltalk image the state *is* the answer: the object that failed is still there, with its neighbours. `stlive` is a deliberately small interface to that:

- **Navigation instead of dumps.** Sessions, frames and objects are addressable references (`dbg-…`, `obj-…`). Every answer is compact, paged and says when it was truncated – an agent never has to swallow a full stack trace or a 5-million-element collection.
- **Resume ≠ re-run ≠ restart, and the tool says so.** Compiling a method does not repair an already-running frame. `debug resume`, `debug restart --frame n` and `debug rerun` are different operations, labelled `fresh_execution` / `fix_verified`; compile results list the `stale_frames` that still run the old code.
- **Real Pharo, not a re-implementation.** Fixes are compiled by Pharo's own compiler into the live image (`stale` detection, `DebugSession` restart/return, `OpalCompiler` evaluation inside a frame).
- **Persistent.** The image keeps running between calls; short-lived CLI processes only carry JSON.
- **Round trip to your repo.** `save` patches Tonel files method by method (new classes, extension methods, class-side methods included), so reviews stay readable.

## Install

Requirements: macOS or Linux (x86-64/arm64 where Pharo 13 VMs exist), `curl`, `unzip`, a [Rust toolchain](https://rustup.rs) to build, and `jq` for the examples.

```bash
cargo install --git https://github.com/paulwilke/smalltalk-live-cli   # or: cargo build --release
stlive init        # one time: downloads Pharo 13 (~100 MB) and builds an image containing the server
```

`stlive init` keeps everything in `~/.stlive` (override with `STLIVE_HOME`). Nothing else is installed.

## Use

```bash
cd my-project                   # state (port, pid, log, image copy) lives in ./.stlive
stlive start                    # headless image in the background
stlive load MyProject --repository tonel://$PWD/src --group Tests      # Metacello baseline
stlive test run MyProject-Tests
stlive eval 'MyClass new run'
stlive stop --force
```

Several images can run side by side (`--instance NAME`, `stlive image clone`, `stlive instances`). `--text` prints an indented human-readable form instead of JSON; `--pretty` pretty-prints JSON.

`--log FILE` (or `STLIVE_LOG`) appends one JSON line per call (time, command, duration, output size, ok, key result fields) and `--tag`/`STLIVE_TAG` marks a scene or task – metadata and a 60-character summary of the call, never full code.

Exit codes: `0` ok · `1` command failed, exception in the image or failing tests · `2` usage · `3` image not running/unreachable · `4` protocol error · `5` timeout.

### Already have a running image?

```bash
stlive init --vm /path/to/Pharo --image /path/to/Pharo.image   # once: use your local Pharo (no download); also records the framework package list
stlive attach --port <port of the running server>                # loads the server into the RUNNING image, no restart
```

An image started by an older version of this tool (or any server that speaks the same JSON-line protocol) keeps running; `stlive` then talks to it through its own port. Everything done through `eval` or the IDE is recorded as well, so `save` can write it back.

### What you can do

| Area | Commands |
|---|---|
| Image | `init` (`--vm`/`--image` for a local Pharo), `attach`, `start`, `stop --force`, `status`, `ping`, `instances`, `image info\|save\|clone`, `load` |
| Evaluate | `eval` (stdin with `-`, `--in <ref>` as receiver, `--timeout`, `--full` complete text, `--no-session`; Transcript and deprecations reported separately) |
| Code | `find class\|package\|implementors\|senders`, `class show\|create` (class-side ivars, class variables, comment), `method show\|compile\|remove`, `package list`, `changes list\|show\|watch` |
| Tests | `test list`, `test run` (per test isolation, failures **grouped by cause**, one session kept per group) |
| Objects | `obj show\|items\|var\|text\|compare\|referrers\|graph\|release` – paged, with stable references; `graph` marks cycles and shared nodes |
| Debugging | `debug list\|inspect\|frames\|frame\|locals\|receiver\|eval\|resume\|restart\|return\|rerun\|terminate` |
| Persist | `save --package X \| --all --dir src [--dry-run]` |

Full reference: [docs/commands.md](docs/commands.md). How it works: [docs/architecture.md](docs/architecture.md).

## CLI or MCP?

Short version: this is a CLI because a shell is the one integration point every coding agent already has – and the design keeps the *protocol* separate, so an MCP wrapper would be a thin layer on top (see [docs/cli-vs-mcp.md](docs/cli-vs-mcp.md) for the honest trade-offs).

## Using it with a coding agent

[`skill/SKILL.md`](skill/SKILL.md) is a ready-to-use instruction file (for Claude Code skills or any agent's project instructions) describing the investigate → fix → re-run → verify → save loop and the pitfalls.

## Safety

- The server listens on `127.0.0.1` only, on an ephemeral port published in the instance's state directory. There is **no authentication**: anyone who can connect to that local port can execute arbitrary Smalltalk in the image. Treat it like a local dev REPL, not a service.
- Destructive operations (`stop`, `method remove`) need `--force`. Output is bounded everywhere; endless loops become sessions after `--timeout` instead of hanging.
- References and sessions carry the image's run id, so they are rejected as `stale_ref` after a restart rather than silently pointing at something else.

## Limitations (v0.1)

- Only Pharo 13 is tested. macOS arm64 is the development platform; Linux should work but is less exercised. Windows is not supported.
- Direct writes to the VM's stdout/stderr are not captured (Transcript is).
- No single-stepping (`step into/over`) yet; sessions can be restarted per frame, resumed, or have a frame return a value.
- `save` knows classes, instance/class-side methods, extensions and removals of methods; it never rewrites an existing class definition, and does not handle traits, class-variable changes or comments.
- Endless recursion is reported as a repeating pattern of the top frames; the starting call is deep below the 100 000-frame inspection cap.

## Development

```bash
cargo build --release
./tests/e2e.sh          # needs `stlive init` once; ~30 s; starts its own image in a temp dir
./scripts/gen-reference.sh
```

The Smalltalk side lives in `smalltalk/src` (Tonel) and is compiled into the binary; `stlive init` writes it out and loads it with Metacello.

## License and trademarks

MIT, see [LICENSE](LICENSE). *stlive* is an independent tool and is not affiliated with or endorsed by the Pharo project or the Pharo consortium; "Pharo" and other product names are used only to say what the tool works with.
