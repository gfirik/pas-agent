# PAS-Agent

[![CI](https://github.com/gfirik/pas-agent/actions/workflows/ci.yml/badge.svg)](https://github.com/gfirik/pas-agent/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/gfirik/pas-agent)](https://github.com/gfirik/pas-agent/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**Portable Agent Sessions: carry your work from one AI coding agent to the next.**

Start a task in Claude Code, hit a usage limit or want a second opinion, and continue the
*same* task in Codex, Cursor or Gemini CLI. PAS-Agent keeps a small, local record of the
task, the progress, and the decisions. It writes that record into
the instruction file the next agent already reads at startup, so you don't have to
re-explain everything.

> **Status: alpha.** The CLI works end to end, but the session format and commands may still change
> before 1.0.

**Website and docs:** [gfirik.github.io/pas-agent](https://gfirik.github.io/pas-agent/)

![A PAS-Agent session: init, update, checkpoint, then export to Codex](assets/demo.gif)

```text
Claude Code ──► pas-agent checkpoint ──► .pas-agent/session.json ──► pas-agent export --to codex ──► Codex
```

## Why

Every AI coding agent keeps its own private session. When you switch agents, the new one
doesn't know what the task is, what's been done, what was decided, or what's blocking.
PAS-Agent keeps that context in one provider-neutral file and hands it to whichever agent
comes next.

## Install

PAS-Agent needs **git** on your `PATH`.

**Prebuilt binary** (Linux, macOS, Windows): download the archive for your platform from the
[latest release](https://github.com/gfirik/pas-agent/releases/latest), extract it, and put
`pas-agent` somewhere on your `PATH`.

**With Cargo** (Rust 1.85+):

```bash
cargo install --git https://github.com/gfirik/pas-agent pas-agent-cli
```

Or from a clone:

```bash
git clone https://github.com/gfirik/pas-agent.git
cd pas-agent
cargo install --path crates/pas-agent-cli
```

This installs a single binary called `pas-agent`.

## Quick Start

```bash
# In your project (any subdirectory works; the session lives at the git root)
pas-agent init --task "Add JWT auth"

# Record progress as you go. Flags can be repeated, and items may contain commas.
pas-agent update --completed "Login endpoint" \
                 --remaining "Token refresh" --remaining "Tests" \
                 --decision "Use jose, not jsonwebtoken, for edge runtime support" \
                 --next "Implement refreshToken()"

pas-agent checkpoint "Login flow done"   # snapshots task, git state and changed files
pas-agent status                          # numbered lists; use the numbers with --done / remove
pas-agent update --done 1 --next "Write tests"   # finish remaining item #1, move on

# Hand off
pas-agent export --to codex               # writes AGENTS.md
codex                                     # reads it at startup
```

### Let the agent do it

`pas-agent` is an ordinary command, so your agent can keep the session up to date itself. Add
this to your own part of `CLAUDE.md` / `AGENTS.md` (outside the PAS-Agent block):

```markdown
## Session handoff (PAS-Agent)
When you finish a piece of work, record it with
`pas-agent update --completed "..." --next "..."` (plus `--decision` / `--blocker` as needed).
At a milestone or before the session ends, run `pas-agent checkpoint "<summary>"`.
```

More in [Workflows](https://gfirik.github.io/pas-agent/docs/workflows/) and
[Troubleshooting](https://gfirik.github.io/pas-agent/docs/troubleshooting/).

## Supported Agents

| `--to` | Writes | Read at startup by |
|---|---|---|
| `claude-code` (`claude`) | `CLAUDE.md` | Claude Code |
| `codex` | `AGENTS.md` | Codex |
| `opencode` | `AGENTS.md` | OpenCode |
| `cursor` | `AGENTS.md` | Cursor |
| `kiro-cli` (`kiro`) | `AGENTS.md` | Kiro CLI |
| `antigravity` (`agy`) | `AGENTS.md` | Antigravity |
| `gemini-cli` (`gemini`) | `GEMINI.md` | Gemini CLI |

Use `--output <path>` to write the context anywhere else.

## Your Files Are Safe

`export` never overwrites a hand-written `CLAUDE.md` or `AGENTS.md`. It writes only inside a
marked block and leaves everything else alone:

```markdown
# Your project rules          ← untouched

<!-- pas-agent:start ... -->
# PAS-Agent Session Context
...
<!-- pas-agent:end -->
```

Later exports replace only that block. `--force` replaces the whole file.

## Where State Lives

Everything is in `.pas-agent/session.json`, a versioned JSON file at the project root.
Commit it to carry the session across machines, or add it to `.gitignore` to keep it
local. The CLI makes no network requests.

Anything in the session ends up in a file your agent treats as instructions, so review
changes to `.pas-agent/` the same way you'd review changes to `CLAUDE.md`.

## Commands

| Command | What it does |
|---|---|
| `init` | Create a session at the project root |
| `update` | Set task, objective, next action; add or complete list items |
| `remove <list> <N>…` | Remove items by number |
| `checkpoint [msg]` | Snapshot task state, git state and changed files |
| `status` | Show the session with numbered lists |
| `list` | List checkpoints |
| `export --to <agent>` | Write context for the next agent |

Run `pas-agent <command> --help` for every flag, or see the
[CLI reference](https://gfirik.github.io/pas-agent/docs/cli-reference/).

## Roadmap

- [x] Portable, versioned session format with checkpoints and git state capture
- [x] Non-destructive export for Claude Code, Codex, OpenCode, Cursor, Kiro CLI, Antigravity and Gemini CLI
- [x] Prebuilt binaries for Linux, macOS and Windows
- [ ] Shell hooks that update and export automatically when you switch agents
- [ ] Optional AI-assisted summary of what happened in a session

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). The project layout is simple:

```text
crates/pas-agent-core   session model, storage, git capture, context export
crates/pas-agent-cli    the pas-agent binary and end-to-end tests
website/                landing page and docs (Astro); design notes in website/DESIGN.md
```

## License

[MIT](LICENSE)
