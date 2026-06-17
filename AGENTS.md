# Skill Importer Repo

This repository contains the standalone Rust `skill-importer` crate for
inspecting and managing local AI skill catalogs.

## Layout

- `src/lib.rs` exposes `discover_skills`, `DiscoveryRoots`, and filesystem-safe
  skill operations.
- `src/workflow.rs` owns operation dispatch over resolved `DiscoveryRoots`.
- `src/json_adapter.rs` renders workflow outcomes to stable JSON automation
  output.
- `src/analyzer.rs` builds isolated Codex-based skill analysis launch plans and
  renders analyzer JSON into HTML reports.
- `src/main.rs` contains command parsing, CLI/TUI root handling, and runtime
  adapter wiring.
- `src/tui/` contains reducer-friendly app state, key mapping, ratatui
  rendering, and the crossterm terminal loop.
- `tests/` covers discovery, imports, enable/disable, promote/unpromote/delete,
  analyzer behavior, JSON commands, and TUI behavior.
- `plans/` contains implementation plans for larger importer work.

## Development

Use the Makefile targets from the repo root:

```bash
make build
make test
make fmt-check
make clippy
make check
make run-list
make run-tui
```

The underlying commands are:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

`make run-list` and `make run-tui` use the shared data directory at
`~/.skills-source` for canonical skills, imports, Claude Code links, and Codex
links. Override `DATA_DIR`, `CANONICAL_ROOT`, `IMPORTS_ROOT`,
`CLAUDE_CODE_ROOT`, or `CODEX_ROOT` when working elsewhere.

## Behavior

- Missing roots are treated as empty.
- Canonical and imported skill directories are detected from `SKILL.md`
  frontmatter.
- Agent entries report aggregate enablement for Claude Code, Codex, both, or
  neither.
- Agent entry status distinguishes real directories, managed symlinks, external
  symlinks, broken symlinks, missing entries, and unmanaged files.
- Imports support Markdown from stdin, local path imports, URL imports, and
  repository imports. Repository imports can return an interactive multi-skill
  selection. Repository import is wired through the core library and TUI; direct
  JSON commands expose Markdown, path, and URL imports.
- Enable/disable, promote/unpromote, and delete operations keep filesystem
  safety checks in core operation code. CLI and TUI code should translate
  requests into the workflow module rather than reimplementing dispatch or JSON
  result mapping.
- TUI analysis launches are macOS-only, require the `codex` executable, snapshot
  the selected skill into an isolated workspace, and write reports under the
  user cache directory.

## Conventions

- Use TDD for behavior changes when practical.
- Keep filesystem mutations covered through public core or command interfaces.
- Prefer disposable roots in tests and manual TUI smoke runs by setting
  `DATA_DIR` or explicit root overrides.
- Do not let tests or manual verification touch real `~/.skills-source` unless
  explicitly configured.
