# Skill Importer Repo

This repository contains the standalone Rust `skill-importer` crate for
inspecting and managing local AI skill catalogs.

## Layout

- `src/lib.rs` exposes `discover_skills`, `DiscoveryRoots`, and filesystem-safe
  skill operations.
- `src/workflow.rs` owns operation dispatch over resolved `DiscoveryRoots`.
- `src/json_adapter.rs` renders workflow outcomes to stable JSON automation
  output.
- `src/main.rs` contains command parsing, CLI/TUI root handling, and runtime
  adapter wiring.
- `src/tui/` contains reducer-friendly app state, key mapping, ratatui
  rendering, and the crossterm terminal loop.
- `tests/` covers discovery, imports, enable/disable, promote/delete, JSON
  commands, and TUI behavior.
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
make make-run-prod
```

The underlying commands are:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

`make run-list` and `make run-tui` use disposable local roots under
`.skill-importer/dev` for imports and agent roots. By default, they look for a
sibling skills catalog at `../skills/catalog/portable`; override
`SKILLS_REPO`, `CANONICAL_ROOT`, `IMPORTS_ROOT`, `CLAUDE_CODE_ROOT`, or
`CODEX_ROOT` when working elsewhere.

`make make-run-prod` runs the TUI without disposable root overrides. It uses
normal CLI defaults for canonical and imported skills, plus user-level agent
roots at `~/.claude/skills` and `~/.agents/skills`, so enable/disable actions
can mutate user-level skill symlinks.

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
  selection.
- JSON commands expose Markdown, path, and URL imports; repository import is
  exposed through the core library and TUI.
- Enable/disable, promote, and delete operations keep filesystem safety checks
  in core operation code. CLI and TUI code should translate requests into the
  workflow module rather than reimplementing dispatch or JSON result mapping.

## Conventions

- Use TDD for behavior changes when practical.
- Keep filesystem mutations covered through public core or command interfaces.
- Prefer disposable roots in tests and manual TUI smoke runs.
- Do not let tests or manual verification touch real `~/.claude/skills` or
  `~/.agents/skills` unless explicitly configured.
