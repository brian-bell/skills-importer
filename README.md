# skill-importer

Rust CLI and keyboard-first TUI for inspecting and managing local AI skill
catalogs across canonical storage, imported storage, Claude Code skills, and
Codex skills.

## Features

- Discover skills from canonical, imported, Claude Code, and Codex roots.
- Read skill metadata from `SKILL.md` frontmatter.
- Report whether each skill is enabled for Claude Code, Codex, both, or neither.
- Classify agent entries as managed symlinks, external symlinks, real
  directories, broken symlinks, unmanaged files, or missing entries.
- Import skills from Markdown, local paths, URLs, and repositories.
- Enable, disable, promote, unpromote, and delete skills with filesystem safety
  checks.
- Launch isolated skill analysis reports from the TUI on macOS.
- Run a JSON automation interface or an interactive ratatui terminal UI.

Internally, resolved operations flow through a shared workflow module, and the
JSON adapter renders those outcomes for automation consumers. Command parsing
and root defaulting stay at the CLI/runtime edge.

## Development

```bash
make build
make test
make fmt-check
make clippy
make check
```

`make run-list` and `make run-tui` use the shared data directory at
`~/.skills-source` for canonical skills, imports, Claude Code links, and Codex
links:

```bash
make run-list
make run-tui
```

Override roots when needed:

```bash
make run-list DATA_DIR=/path/to/skills-source
make run-tui CANONICAL_ROOT=/path/to/catalog/portable
```

## JSON Commands

Automation commands require `--json` and write stable JSON output:

```bash
skill-importer list --json
skill-importer import markdown --json < SKILL.md
skill-importer import path --json --path ./some-skill
skill-importer import url --json --url https://example.test/skill.md
skill-importer enable --json --skill my-skill --agent codex
skill-importer disable --json --skill my-skill --agent claude-code
skill-importer promote --json --skill my-imported-skill
skill-importer delete --json --skill my-unpromoted-import
```

All commands accept root overrides:

```bash
--canonical-root PATH
--imports-root PATH
--claude-code-root PATH
--codex-root PATH
```

JSON commands without root overrides use `~/.skills-source` as a single data
directory. The default roots are `~/.skills-source/catalog/portable` for
canonical skills, `~/.skills-source/imports` for imports,
`~/.skills-source/claude-code` for Claude Code links, and
`~/.skills-source/codex` for Codex links.

Repository imports persist structured `source_repository` metadata in each
import manifest. `skill-importer list --json` includes that metadata on imported
skill entries and derives a top-level `source_repositories` list grouping
repository-imported skills by repository.

Repository import is available in the TUI. Direct JSON automation commands
currently expose Markdown, path, and URL imports.

## TUI

Run the interactive TUI with:

```bash
skill-importer tui
```

Important keys:

```text
j/k or arrows  move selection
c             toggle selected skill for Claude Code
x             toggle selected skill for Codex
p             confirm promotion or unpromotion
r             confirm deletion for selected import
m             import Markdown from prompt text
f             import local path from prompt text
u             import URL from prompt text
g             import repository from prompt text
i             toggle all/imported source filter
A             launch isolated skill analysis
space         toggle repository candidate selection
enter         confirm prompt, confirmation, or repository candidate
esc           cancel prompt or repository selection
q             quit from the main screen
```

Skill analysis launches a Codex run against a snapshot of the selected skill
and writes an HTML report under the user cache directory. It requires macOS and
the `codex` executable.
