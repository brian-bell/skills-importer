# Plan: Agent Skills Third-Party Promotion Roots

## Goal

Move promoted skill management out of the importer repo's canonical catalog
model and into the companion `brian-bell/agent-skills` repository.

Imported skills should continue to land in the gitignored placeholder storage
under `.skill-importer/imports`. Promote, unpromote, enable, and disable should
operate around the companion repo instead:

- Promote copies an imported skill to `<agent-skills-repo>/third-party/<skill-name>`.
- Unpromote deletes `<agent-skills-repo>/third-party/<skill-name>`.
- Enable creates Claude Code or Codex symlinks to the promoted third-party copy.
- Disable removes those managed symlinks.

The default companion repo is `~/dev/agent-skills`, overrideable with an
environment variable.

## Non-Goals

- Do not change import storage away from `.skill-importer/imports`.
- Do not enable unpromoted placeholder imports directly.
- Do not mutate real user-level agent roots in tests unless explicitly
  configured.
- Do not commit or push directly to `main`.
- Do not ship unrelated dirty work from either repository.

## Current System Observations

- `DiscoveryRoots` currently has `canonical_root`, `imports_root`,
  `claude_code_root`, and `codex_root` fields.
- CLI defaults currently derive the import placeholder from the runtime repo and
  `canonical_root` from a catalog-style repo layout.
- Imports already store into `imports_root`, commonly under
  `.skill-importer/imports`.
- Promotion currently copies from `imports_root/<skill>` to
  `canonical_root/<skill>` and rewrites any enabled import symlinks to that
  canonical path.
- Unpromotion currently deletes the canonical copy, marks the import manifest
  unpromoted, and relinks enabled agents back to the placeholder import.
- Enable and disable currently resolve the source from discovered canonical or
  imported skills, then create or remove agent symlinks to that source.
- TUI state already has confirmation mode for promote, unpromote, and delete,
  but it does not yet distinguish ordinary promote from overwrite-confirmed
  promote.

## Proposed Model

Add a companion repo root to runtime configuration:

- Env var: `AGENT_SKILLS_REPO`
- Default: `~/dev/agent-skills`
- Promoted third-party root: `<agent-skills-repo>/third-party`
- Promoted skill path: `<agent-skills-repo>/third-party/<skill-name>`

Keep `imports_root` as the placeholder import storage:

- Default remains under `.skill-importer/imports`.
- Imported manifests continue to track `promoted: bool`.

For compatibility, the implementation can either:

- Extend `DiscoveryRoots` with `agent_skills_repo` or
  `third_party_root`, while keeping `canonical_root` for existing tests during
  migration.
- Or reinterpret `canonical_root` internally as the promoted third-party root.

Prefer the first option if the change remains contained; it names the domain
more honestly and prevents future confusion.

## Implementation Steps

1. Refresh base state.
   - Pull latest `main`.
   - Create a non-main branch.
   - Check `git status --short` and preserve unrelated files, including
     untracked `.DS_Store`.

2. Add companion repo root resolution.
   - Add `AGENT_SKILLS_REPO` handling to CLI/root default code.
   - Default to `~/dev/agent-skills`.
   - Derive the promoted third-party root as `<agent-skills-repo>/third-party`.
   - Add a CLI override only if needed for tests or explicit user workflows;
     otherwise prefer env var plus existing explicit root flags.

3. Update discovery.
   - Discover promoted skills from the third-party root.
   - Treat symlinks into the third-party root as managed promoted symlinks.
   - Continue discovering placeholder imports from `.skill-importer/imports`.
   - Preserve merged inventory behavior and `promoted` manifest reporting.
   - Ensure unpromoted imports appear as imported but not enabled.

4. Update promote.
   - Resolve source from `.skill-importer/imports/<skill-name>`.
   - Refuse missing, unsupported, agent-only, or already-promoted imports.
   - Copy to `<agent-skills-repo>/third-party/<skill-name>`.
   - Exclude top-level `import.json` from the promoted copy.
   - Preserve supporting directories and files.
   - If the destination exists, require explicit overwrite confirmation.
   - On overwrite, replace only the exact promoted skill directory under the
     third-party root.
   - Mark the import manifest `promoted: true`.
   - Relink any managed agent symlinks that still point at the placeholder
     import to the third-party copy.

5. Update unpromote.
   - Resolve only promoted imports.
   - Confirm in TUI because this deletes from the companion repo.
   - Delete `<agent-skills-repo>/third-party/<skill-name>`.
   - Remove managed Claude Code or Codex symlinks pointing at that third-party
     copy.
   - Do not relink agents back to `.skill-importer/imports`.
   - Mark the import manifest `promoted: false`.

6. Update enable and disable.
   - Enable should resolve only to
     `<agent-skills-repo>/third-party/<skill-name>`.
   - Enabling an unpromoted placeholder import should fail clearly.
   - Disable should remove only managed symlinks pointing at the third-party
     skill path.
   - Preserve unsafe-entry protection for real directories, regular files,
     external symlinks, broken symlinks, and symlinks to the wrong managed
     target.

7. Update TUI confirmation flow.
   - Keep `p` as promote/unpromote toggle.
   - If selected import is unpromoted and the third-party destination is empty,
     normal confirmation can promote.
   - If the destination already exists, show an overwrite confirmation modal
     naming the skill and destination path.
   - If selected import is promoted, show an unpromote/delete confirmation modal
     naming the third-party path that will be deleted.
   - Terminal operation dispatch should pass overwrite intent through to the
     core workflow.

8. Update JSON and errors.
   - Preserve stable action JSON where possible.
   - Add or reuse action kinds for overwrite replacement if needed.
   - Make collision errors name the third-party destination path.
   - Make unpromoted-enable errors distinguish "known import, not promoted"
     from "unknown skill".

9. Update docs and Makefile.
   - Document `AGENT_SKILLS_REPO`.
   - Keep `make run-list` and `make run-tui` using disposable import and agent
     roots by default.
   - Avoid manual smoke tests that mutate real `~/.claude/skills`,
     `~/.agents/skills`, or `~/dev/agent-skills` unless explicitly configured.

## TDD Plan

Use vertical red-green-refactor slices. Each test should exercise public
library or command behavior instead of private helpers.

1. Root defaults.
   - RED: command/root parsing test expects `AGENT_SKILLS_REPO` to derive the
     third-party promoted root.
   - GREEN: implement env var/default root resolution.
   - REFACTOR: keep root naming small and explicit.

2. Import storage remains unchanged.
   - RED: import command or library test expects imported skill storage under
     `.skill-importer/imports`, with no third-party write.
   - GREEN: adjust root changes without affecting import storage.
   - REFACTOR: isolate placeholder import path from promoted root path.

3. Promote to third-party.
   - RED: promote an imported skill and expect files at
     `<agent-skills-repo>/third-party/<skill-name>`, no copied top-level
     `import.json`, and manifest `promoted: true`.
   - GREEN: update promotion destination.
   - REFACTOR: keep copy safety reusable.

4. Promote overwrite.
   - RED: promote fails when third-party destination already exists and no
     overwrite confirmation is present.
   - GREEN: preserve collision behavior.
   - RED: promote with overwrite confirmation replaces only the exact
     destination skill directory.
   - GREEN: add overwrite request plumbing and safe replacement.

5. Enable promoted skill.
   - RED: enabling a promoted import creates an agent symlink to the
     third-party copy.
   - GREEN: update enable source resolution.
   - REFACTOR: share promoted-source resolution between enable and disable.

6. Refuse unpromoted enable.
   - RED: enabling an unpromoted import fails with a clear not-promoted style
     error and creates no symlink.
   - GREEN: enforce promoted-only enablement.

7. Disable promoted skill.
   - RED: disabling removes a managed symlink to third-party and is idempotent
     when missing.
   - GREEN: update disable planning.
   - RED: disabling refuses symlinks to placeholder imports or other wrong
     targets.
   - GREEN: preserve unsafe target checks.

8. Unpromote.
   - RED: unpromote deletes the third-party copy, removes managed agent
     symlinks, and marks the manifest `promoted: false`.
   - GREEN: replace old relink-to-import behavior.
   - REFACTOR: ensure partial failure action reporting remains useful.

9. Discovery.
   - RED: inventory shows a promoted import discovered from third-party, with
     agent symlink status classified as promoted/canonical managed symlink.
   - GREEN: update discovery classification.
   - RED: unpromoted placeholder import remains visible but not enableable.
   - GREEN: ensure inventory source and promoted flags are coherent.

10. TUI confirmation.
    - RED: reducer/render tests show overwrite confirmation before queuing an
      overwrite promote request.
    - GREEN: add overwrite-aware confirmation state.
    - RED: unpromote confirmation queues unpromote and renders delete path
      language.
    - GREEN: update modal state and terminal dispatch.

## Review Loop Expectations

Before implementation:

- Review this plan for root naming, public API churn, and whether overwrite
  behavior is safe enough.
- Quality gate: 8/10.

During implementation:

- Run at least two focused review passes over the final diff.
- Review criteria:
  - No real user roots are touched by tests.
  - No unrelated files are changed.
  - Unpromoted imports cannot be enabled.
  - Overwrite and unpromote cannot delete outside
    `<agent-skills-repo>/third-party/<skill-name>`.
  - Error messages name the relevant path.
  - Existing import behavior remains stable.

## Verification

Run the full project checks:

```bash
make fmt-check
make clippy
make test
```

Run focused checks while developing:

```bash
cargo test promote
cargo test enable_disable
cargo test discovery
cargo test list_command
cargo test tui_state
cargo test tui_render
cargo test workflow
```

Manual smoke tests should use disposable roots:

```bash
AGENT_SKILLS_REPO=/tmp/agent-skills-smoke make run-list
AGENT_SKILLS_REPO=/tmp/agent-skills-smoke make run-tui
```

## Acceptance Criteria

- Imports still create placeholder copies under `.skill-importer/imports`.
- Promotion copies to `<agent-skills-repo>/third-party/<skill-name>`.
- Promotion refuses existing destinations unless overwrite is explicitly
  confirmed.
- Overwrite only replaces the selected third-party skill directory.
- Unpromotion deletes the third-party copy after confirmation.
- Unpromotion removes managed agent symlinks to that copy and does not relink
  agents to placeholder imports.
- Enable creates managed Claude Code or Codex symlinks to the third-party copy.
- Disable removes only those managed symlinks.
- Unpromoted imports cannot be enabled.
- Discovery, JSON output, and TUI state accurately reflect promoted,
  unpromoted, enabled, disabled, and unsafe entries.
- Tests cover filesystem mutations through public core or command interfaces.

## Risks And Stop Conditions

- Stop if root naming changes require a broad public API migration beyond this
  behavior change.
- Stop if an overwrite or unpromote path cannot be proven to live under
  `<agent-skills-repo>/third-party`.
- Stop if tests would need to mutate real `~/dev/agent-skills`,
  `~/.claude/skills`, or `~/.agents/skills`.
- Stop if the companion repo has unrelated changes that would be overwritten or
  deleted by a manual smoke test.
- Ask for review before committing if the plan needs product-level decisions,
  especially around CLI overwrite flags or public root names.
