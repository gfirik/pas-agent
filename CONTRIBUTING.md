# Contributing to PAS-Agent

Thank you for your interest in contributing to PAS-Agent! As an open-source, local-first tool for AI agent interoperability, community contributions are essential to expanding support for different agents and workflows.

## How to Contribute

1. **Report Bugs**: If you find a bug, please open an issue with reproduction steps.
2. **Suggest Features**: Have an idea for a new agent exporter or workflow? Open an issue to discuss it before writing code.
3. **Submit Pull Requests**: Feel free to submit PRs for bug fixes, documentation improvements, or discussed features.

## Development Setup

PAS-Agent is written in Rust. You will need:
- Rust 1.85+ (`rustup`)
- Git
- Bun (only for the website)

```bash
# Clone the repo
git clone https://github.com/gfirik/pas-agent
cd pas-agent

# Build the CLI
cargo build

# Run tests
cargo test

# Website (optional)
cd website && bun install && bun run dev
```

Project layout and the rules every change should follow (file-format compatibility,
non-destructive export, docs kept in sync) are in [AGENTS.md](AGENTS.md). Website design
rules are in [website/DESIGN.md](website/DESIGN.md).

## Pull Request Guidelines

- Ensure `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass.
- Keep PRs focused on a single feature or bug fix.
- Add tests for new core functionality.
- Update documentation if you change CLI arguments or behavior.

Thank you for helping make AI coding agents more portable!
