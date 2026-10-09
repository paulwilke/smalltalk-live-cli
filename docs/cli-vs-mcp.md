# CLI or MCP?

Both can expose exactly the same operations. The question is which integration fits the way you work.

| | CLI (this tool) | MCP server |
|---|---|---|
| Setup in an agent | nothing – every agent that can run shell commands can use it; put the instructions in `SKILL.md`/project docs | register a server per client; tool schemas are loaded into the model's context |
| Context cost | pay only for the commands used; output is whatever you pipe through `jq` | all tool definitions occupy context up front |
| Composability | pipes, `jq`, scripts, `xargs`, CI, git hooks, human use in a terminal | only through the agent |
| Statefulness | state lives in the image; each call is independent (sessions are references) | a persistent server could also keep connection state, but the image already does |
| Discoverability | `--help`, a skill file | typed tool schemas the client can validate and show |
| Typed arguments | strings on a command line (quoting with Smalltalk strings is awkward – use stdin) | JSON schemas, no shell quoting |
| Permissions UX | the agent's shell permission prompts | per-tool approval granularity |

Our take: the hard part of live development is *not* the transport, it is the shape of the answers (compact, paged, referencing, honest about resume vs re-run). That lives in the image and is transport-agnostic. A CLI gets you started on any machine with zero client configuration and is pleasant for humans; if you want typed tools and per-tool approvals, an MCP server can be a ~200-line adapter that forwards to the same JSON line protocol (`{"cmd": "...", "args": {...}}` on the local port). This repository does not ship one yet.
