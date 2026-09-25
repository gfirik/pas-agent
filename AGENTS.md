# Working on PAS-Agent

Guidance for AI coding agents (and humans) contributing to this repository.

## Layout

- `crates/pas-agent-core`: library with the session model (`session.rs`), storage and discovery
  (`storage.rs`), git capture (`git.rs`), and context generation and the marked-block
  writer (`context.rs`).
- `crates/pas-agent-cli`: the `pas-agent` binary (`src/main.rs`) and end-to-end tests
  (`tests/cli.rs`).
- `website/`: the Astro site (landing page and docs). Design rules are in `website/DESIGN.md`.

## Before You Finish a Change

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
cd website && bun run build   # only if the website changed
```

## Rules

- `session.json` is a public format. New fields need `#[serde(default)]`, and breaking
  changes need a `SCHEMA_VERSION` bump plus a migration.
- `export` must never destroy user content outside the `pas-agent` block.
- Anything written into exported Markdown must go through `inline()` in `context.rs`.
- Every CLI behaviour change needs an end-to-end test in `crates/pas-agent-cli/tests/cli.rs`
  and matching updates to `website/src/pages/docs/` and `README.md`.
- The website must only describe features that exist; unbuilt work is labelled as coming next.
- Keep the core free of CLI dependencies (no clap in `pas-agent-core`).
