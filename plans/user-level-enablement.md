# User-Level Skill Enablement Plan

## Goal

When a user enables a skill, `skill-importer` should make that skill available
to future agent sessions by symlinking the skill into the selected agent's
user-level skill directory.

- Claude Code enablement should create `~/.claude/skills/<skill-name>`.
- Codex enablement should create `~/.agents/skills/<skill-name>`.
- The symlink should point at the resolved owned skill source, either canonical
  or imported.
- Development runs should keep using disposable roots unless explicitly launched
  in production mode.

The current core enable path already creates symlinks under the resolved agent
root. This plan focuses on locking down the production default roots, preserving
safe override behavior, documenting the distinction between dev and production
runs, and adding a Makefile production TUI command.

## Non-Goals

- Do not copy skill directories into agent roots.
- Do not overwrite existing unsafe agent entries.
- Do not change import, promotion, or deletion semantics except where tests
  reveal a direct interaction with user-level enablement.
- Do not make `make run-tui` mutate real user-level agent directories.

## Acceptance Criteria

- `skill-importer enable --json --skill NAME --agent codex`, without
  `--codex-root`, creates `~/.agents/skills/NAME`.
- `skill-importer enable --json --skill NAME --agent claude-code`, without
  `--claude-code-root`, creates `~/.claude/skills/NAME`.
- The created entry is a symlink whose canonical target is the owned canonical
  or imported skill directory.
- Missing user-level agent skill directories are created as needed.
- Enabling an already-correct symlink reports `skip_unchanged`.
- Enabling both agents in one command creates or verifies both user-level
  symlinks.
- Existing unsafe entries are refused without mutation, including real
  directories, regular files, external symlinks, wrong managed symlinks, and
  broken symlinks.
- Root override flags still work for tests, automation, and disposable
  development roots.
- The TUI uses the same production default roots when launched without root
  overrides.
- A `make-run-prod` Makefile command launches the TUI without disposable root
  overrides, so it uses production/default roots.
- README documentation distinguishes disposable dev runs from production TUI
  runs.

## TDD Implementation Plan

### Slice 1: Prove CLI Codex Defaults Are User-Level

RED:

- Add an integration-style command test that runs the binary with a temporary
  `HOME`.
- Create a canonical skill in a fake catalog repo so command root discovery has
  a real skill source.
- Run `skill-importer enable --json --skill global-helper --agent codex`
  without `--codex-root`.
- Assert `$HOME/.agents/skills/global-helper` exists, is a symlink, and
  canonicalizes to the canonical skill path.
- Assert JSON includes `create_directory` for `$HOME/.agents/skills` when the
  root is missing and `create_symlink` for the skill entry.

GREEN:

- If this fails, fix only the default root selection path in `src/main.rs`.
- Keep filesystem mutation in the existing core enable flow.
- Prefer extracting a small default-root helper if that makes the behavior easy
  to test without mutating process-global environment in unit tests.

REFACTOR:

- Name helpers around concepts such as user-level Claude Code root and
  user-level Codex root.
- Keep `RootArgs::into_discovery_roots` as the runtime boundary that combines
  CLI overrides with defaults.

### Slice 2: Prove CLI Claude Code Defaults Are User-Level

RED:

- Add the sibling command test for
  `skill-importer enable --json --skill global-helper --agent claude-code`.
- Use temporary `HOME`.
- Assert `$HOME/.claude/skills/global-helper` is the created symlink and points
  at the expected source.

GREEN:

- Apply the same default-root fix to Claude Code if needed.
- Preserve `--claude-code-root` override behavior.

REFACTOR:

- Remove any duplicated assertion setup by adding test helpers in the test
  module, not production abstractions.

### Slice 3: Prove Multi-Agent Enablement Uses Both User Roots

RED:

- Add a command test for enabling one skill with both agents:
  `--agent claude-code --agent codex`.
- Assert both user-level symlinks exist under temporary `HOME`.
- Assert action JSON includes separate agent-specific entries.

GREEN:

- Reuse the existing multi-agent planning path.
- If behavior fails, fix dispatch or root resolution rather than adding a
  special multi-agent branch.

REFACTOR:

- Keep deduplication behavior covered by existing core tests unless this slice
  exposes a command-level regression.

### Slice 4: Preserve Safety at User-Level Paths

RED:

- Add one command-level safety regression using temporary `HOME`.
- Place an unsafe entry at the default user-level agent path before enabling.
- Assert enable fails, error text names the unsafe entry, and the entry remains
  untouched.

GREEN:

- Reuse existing `exact_managed_symlink_state` safety behavior.
- Avoid weakening any current library-level safety tests.

REFACTOR:

- Keep error wording consistent with existing enable/disable failures.

### Slice 5: Prove TUI Production Defaults

RED:

- Add a focused test around the `tui` command using the injected TUI runner.
- Verify that launching `skill-importer tui` without root overrides passes roots
  whose agent paths are the user-level defaults.
- Avoid parent-process environment mutation where possible. Prefer testing
  extracted helper functions or using a subprocess with a temporary `HOME`.

GREEN:

- If needed, route TUI command parsing through the same root defaulting helpers
  used by JSON commands.

REFACTOR:

- Keep the TUI reducer and rendering code unchanged unless this test reveals
  that the TUI bypasses workflow root handling.

### Slice 6: Add Production Make Command

RED:

- Add or update a Makefile-oriented smoke expectation in documentation review:
  the repository exposes a production TUI command named `make-run-prod`.

GREEN:

- Add a Makefile target:

  ```make
  .PHONY: make-run-prod

  make-run-prod:
  	$(CARGO) run -- tui
  ```

- Do not pass `ROOT_FLAGS` from this target.
- Keep `run-tui` using disposable `.skill-importer/dev` roots.

REFACTOR:

- Consider whether `run-prod` should be added as a friendlier alias later, but
  keep this implementation focused on the requested `make-run-prod` command.

### Slice 7: Document Dev vs Production Runs

RED:

- Treat README review as the failing spec: a user should be able to tell which
  commands mutate disposable roots and which commands use user-level roots.

GREEN:

- Update README development docs to show:

  ```bash
  make run-tui
  make make-run-prod
  ```

- Explain that `make run-tui` uses disposable roots under `.skill-importer/dev`.
- Explain that `make make-run-prod` launches the TUI with production/default
  roots and may create or remove symlinks in `~/.claude/skills` and
  `~/.agents/skills` when enable/disable actions are used.
- Clarify that direct JSON commands without root overrides also use production
  defaults.

REFACTOR:

- Keep README wording brief and operational. Avoid duplicating the full command
  usage block.

## Verification

Run the standard repository checks from the repo root:

```bash
make fmt-check
make clippy
make test
```

Also manually smoke-check the command surfaces with disposable roots:

```bash
make run-tui
cargo run -- enable --json --skill <test-skill> --agent codex \
  --canonical-root <path> \
  --imports-root <path> \
  --codex-root <temp-path>
```

Manual production smoke should be explicit because it can affect real
user-level skill directories:

```bash
make make-run-prod
```

## Files Expected To Change

- `src/main.rs`: default-root helper tests or root defaulting fixes if needed.
- `tests/enable_disable.rs`: command-level enablement and safety coverage.
- `Makefile`: `make-run-prod` target.
- `README.md`: production/default root behavior and Makefile command docs.

`src/lib.rs` should only change if tests reveal a real gap in the existing core
enablement behavior.
