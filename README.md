# stlive – let an AI agent debug Smalltalk by looking at the running program

`stlive` is a command-line tool for **Pharo Smalltalk**. It keeps one Pharo image running in the background and lets you – or a coding agent such as Claude Code – work *inside* it from a shell:

- run code and, when it crashes, **keep the crashed program alive** instead of getting a wall of text,
- **look around** in it, step by step: which call failed, what were the variables, what does that object contain,
- **fix the method right there**, without restarting anything,
- **run the operation again**, run the tests, and
- **write the fix back** into your source files as a small, clean `git diff`.

Normally an agent edits files, restarts the program and reads a log. Smalltalk can do much better because the program *is* the development environment – `stlive` hands that ability to the agent.

## What can I do with it?

| I want to… | Command |
|---|---|
| run some code in my running app | `stlive eval 'Cart new total'` |
| see why it crashed – without a 200-line stack trace | `stlive debug frames dbg-… --limit 5`, then `stlive debug frame dbg-… 3` |
| look at an object and what it points to | `stlive obj show obj-…`, `stlive obj graph obj-… --depth 3` |
| try an idea inside the failing method's context | `stlive debug eval dbg-… --frame 3 'sum + 1'` |
| fix a method in the running app | `stlive method compile Cart --file total.st` |
| prove the fix, not just hope | `stlive debug rerun dbg-…` (runs the original operation again with the new code) |
| run tests and see failures *grouped by cause* | `stlive test run MyApp-Tests` |
| get the fix into my git repo | `stlive save --package MyApp --dir src` |

Every answer is small JSON (or readable text with `--text`), paged and with references to dig deeper – so an agent never has to read huge dumps.

## Two minutes: a crash, a fix, a proof

```text
$ stlive eval - <<< "| c | c := Cart new. c setUp. c add: 10. c total"
{"ok":false,"error":{"type":"MessageNotUnderstood","session":"dbg-3fa-1",
  "message":"receiver of \"adaptToNumber:andSend:\" is nil","first_application_frame":3, ...}}

$ stlive debug frames dbg-3fa-1 --limit 4 | jq -c '.result.frames[]|[.index,.label,.origin]'
[0,"UndefinedObject(Object)>>#adaptToInteger:andSend:","framework"]
[1,"SmallInteger(Integer)>>#+","framework"]
[2,"SmallInteger>>#+","framework"]
[3,"Cart>>#total","application"]            <- our own code: the tax rate is nil

$ stlive method compile Cart - <<< $'total\n\t...\n\t^ sum * (1 + (taxRate ifNil: [ 0 ]))'
{"ok":true,"result":{"selector":"total","stale_frames":[{"session":"dbg-3fa-1","frame":3}]}}

$ stlive debug resume dbg-3fa-1     # continue the OLD, broken run: stlive never calls this a repair
{"ok":false,"mode":"resume", ...}

$ stlive debug rerun dbg-3fa-1      # run the original operation again, from scratch, with the new code
{"ok":true,"result":{"mode":"rerun","fresh_execution":true,"value":{"class":"SmallInteger","print":"15"}}}

$ stlive save --package Demo --dir src && git diff --stat
```

The crashed run stays in the image as a **debug session** (`dbg-3fa-1`). You can come back to it with any number of separate commands – that is the trick that makes this useful for agents, who call tools one at a time.

## The skill: how an agent learns to use it

An agent that has never seen `stlive` will fumble with it. [`skill/SKILL.md`](skill/SKILL.md) is a short instruction file that teaches the **working loop** – reproduce → look in small steps → form a hypothesis and try it → fix → *re-run (not just resume)* → test → save – plus the traps (shell quoting, `resume` proves nothing, add a regression test and see it fail first, save every package).

Install it for Claude Code by copying it:

```bash
mkdir -p ~/.claude/skills/stlive && cp skill/SKILL.md ~/.claude/skills/stlive/SKILL.md   # all projects
# or per project: .claude/skills/stlive/SKILL.md
```

Then just ask: *"The test `CartTest` fails – find the cause with stlive, fix it and save the change."* Other agents can use the same file as project instructions. Without the skill the tool still works; the skill makes agents use it **efficiently and correctly**.

## Why it is built this way

- **Navigation instead of dumps.** Sessions, frames and objects have short references (`dbg-…`, `obj-…`). Answers are compact, paged and say when they were cut off.
- **Resume ≠ re-run ≠ restart – and the tool says so.** Recompiling a method does not repair a run that is already half-way through. `debug resume`, `debug restart --frame n` and `debug rerun` are different, labelled operations (`fresh_execution`, `fix_verified`), and `method compile` lists the `stale_frames` that still run old code.
- **Real Pharo.** Fixes are compiled by Pharo's own compiler into the live image; debugging uses Pharo's own debug machinery.
- **Round trip to your repo.** `save` patches Tonel files method by method, so code review stays readable.

## Developing a window (GUI mode)

```bash
stlive start --gui                      # Pharo opens WITH its window and stays connected
stlive eval --ui - <<< "MyPresenter new open. 1"                 # --ui: run in the UI process (safe for Spec/Morphic)
stlive eval --ui "(Smalltalk at: #App) labelPresenter label: 'Hello'. 1"
stlive ui windows                       # titles, presenter classes, bounds
stlive ui screenshot --window Demo      # PNG -> ./.stlive/screenshots/…   (look at it, or let the agent look)
stlive ui press Increment               # click a button by its label
stlive method compile MyPresenter --ui --file new.st    # change code while the window is open
```

An error inside the UI process becomes an ordinary stlive debug session – **no Pharo debugger window** appears and the GUI keeps running. That gives agents the loop *change → look → check* for Spec2/Morphic now (Bloc/Toplo are the same idea). For web UIs use `stlive open-ui http://localhost:8080` (a chromeless Chrome/Edge window). Details: [docs/architecture.md](docs/architecture.md#gui-mode-and-the-ui-process).

## Install

Requirements: macOS or Linux (x86-64/arm64 where Pharo 13 VMs exist), `curl`, `unzip`, a [Rust toolchain](https://rustup.rs) to build, and `jq` for the examples.

```bash
cargo install --git https://github.com/paulwilke/smalltalk-live-cli   # or: cargo build --release
stlive init        # one time: downloads Pharo 13 (~100 MB) and builds an image containing the server
```

Per project, state lives in `./.stlive` (a `.gitignore` inside it is created automatically, so nothing from it – images, logs, Pharo's `ombu-sessions` – gets committed).

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
| Window | `start --gui`, `eval/compile --ui`, `ui windows\|screenshot\|press`, `open-ui` |
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

## Safety

- The server listens on `127.0.0.1` only, on an ephemeral port published in the instance's state directory. There is **no authentication**: anyone who can connect to that local port can execute arbitrary Smalltalk in the image. Treat it like a local dev REPL, not a service.
- Destructive operations (`stop`, `method remove`) need `--force`. Output is bounded everywhere; endless loops become sessions after `--timeout` instead of hanging.
- References and sessions carry the image's run id, so they are rejected as `stale_ref` after a restart rather than silently pointing at something else.

## Limitations

- GUI mode is verified with Spec2/Morphic on macOS (Pharo 13). Windows support is written (separate `platform.rs`, builds for the Windows target) but has not been run on Windows.

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
