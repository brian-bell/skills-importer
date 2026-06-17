# skill-importer

Rust CLI and keyboard-first TUI for inspecting and managing local AI skills
across promoted third-party storage, imported placeholder storage, Claude Code
skills, and Codex skills.

## Features

- Discover skills from promoted third-party, imported, Claude Code, and Codex
  roots.
- Read skill metadata from `SKILL.md` frontmatter.
- Report whether each skill is enabled for Claude Code, Codex, both, or neither.
- Classify agent entries as managed symlinks, external symlinks, real
  directories, broken symlinks, unmanaged files, or missing entries.
- Import skills from Markdown, local paths, URLs, and repositories.
- Enable, disable, promote, and delete skills with filesystem safety checks.
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

`make run-list` and `make run-tui` use disposable local roots under
`.skill-importer/dev` for the companion agent-skills repo, imports, and agent
roots:

```bash
make run-list
make run-tui
```

Use the production TUI target when you want user-level agent roots instead of
disposable development agent roots:

```bash
make run-prod
```

`make run-prod` runs `skill-importer tui` without root overrides. It uses
normal CLI defaults for promoted third-party and imported skills, while enable
and disable actions can create or remove user-level symlinks in
`~/.claude/skills` and `~/.agents/skills`.

Override roots when needed:

```bash
make run-list AGENT_SKILLS_REPO=/path/to/agent-skills
make run-tui CANONICAL_ROOT=/path/to/agent-skills/third-party
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

`AGENT_SKILLS_REPO` controls the companion repository root used for promoted
third-party skills. When it is not set, the default is `~/dev/agent-skills`; the
promoted root is always `<agent-skills-repo>/third-party`. JSON commands without
root overrides use `.skill-importer/imports` under the detected runtime repo,
plus user-level agent roots: `~/.claude/skills` for Claude Code and
`~/.agents/skills` for Codex.

Repository imports persist structured `source_repository` metadata in each
import manifest. `skill-importer list --json` includes that metadata on imported
skill entries and derives a top-level `source_repositories` list grouping
repository-imported skills by repository.

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
p             confirm promotion for selected skill
r             confirm deletion for selected import
m             import Markdown from prompt text
f             import local path from prompt text
u             import URL from prompt text
g             import repository from prompt text
space         toggle repository candidate selection
enter         confirm prompt, confirmation, or repository candidate
esc           cancel prompt or repository selection
q             quit from the main screen
```
