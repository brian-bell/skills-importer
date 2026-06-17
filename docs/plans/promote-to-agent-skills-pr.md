# Plan: Promote Imports Through Agent Skills PRs

## Goal

Change `promote` from a local canonical-copy operation into a workflow that
copies an imported skill into the third-party skills area of
`https://github.com/brian-bell/agent-skills`, then launches a terminal running
headless Codex to prepare a pull request.

The launched Codex session should update the target repo's documentation,
installer script, and attribution metadata, and should include available
analysis results in the PR context.

## Current Behavior

`promote_imported_skill` currently:

- Resolves a draft imported skill from `imports_root`.
- Refuses a destination collision under `canonical_root`.
- Copies the skill into `canonical_root/<skill-name>`.
- Excludes the top-level `import.json` from the promoted copy.
- Marks the import manifest as `promoted: true`.
- Relinks enabled managed agent symlinks from the import path to the canonical
  path.

The JSON and TUI command paths dispatch through `workflow::OperationRequest`.
Promotion is tested through public core functions and command-level tests in
`tests/promote.rs`.

## Target Behavior

`promote` should:

- Resolve the imported skill exactly as it does today.
- Copy the whole skill directory, including related support files, to
  `<agent-skills-repo>/third-party/<skill-name>`.
- Exclude the managed top-level `import.json` from the copied skill.
- Refuse destination collisions before mutation.
- Preserve existing filesystem safety checks for managed agent entries.
- Mark the local import manifest as `promoted: true` after successful local
  promotion preparation.
- Relink enabled managed agent symlinks to the promoted third-party skill path.
- Launch a macOS terminal script that runs headless Codex in the
  `agent-skills` checkout.
- Prompt Codex to create a branch, update docs/scripts/attribution, commit,
  push, and open a PR.
- Include import source attribution and available analysis report paths in the
  handoff prompt.

## Assumptions To Confirm

- The default local checkout for `https://github.com/brian-bell/agent-skills`
  is `/Users/brian/dev/agent-skills`.
- The destination directory is always `third-party/<skill-name>`.
- "Related files" means every file under the imported skill directory except
  the top-level `import.json`.
- Promotion should not fail when no analysis report exists.
- A launch failure should fail promotion before writing `promoted: true`.
- `unpromote` should be revisited separately after the new external-PR
  promotion behavior lands.

## Non-Goals

- Do not directly commit, push, or open the agent-skills PR from
  `skill-importer` itself.
- Do not run arbitrary imported skill scripts during promotion.
- Do not overwrite existing third-party skills.
- Do not change import, enable, disable, delete, repository import, or analyzer
  semantics except where promotion integration requires it.
- Do not mutate real user-level agent roots in tests.

## Proposed Interfaces

Keep the existing command shape:

```bash
skill-importer promote --json --skill <name>
```

Add a configurable target repo root:

```bash
skill-importer promote --json --skill <name> --skills-repo /Users/brian/dev/agent-skills
```

Runtime default order:

1. Explicit `--skills-repo`.
2. `SKILL_IMPORTER_SKILLS_REPO`.
3. `/Users/brian/dev/agent-skills`.

Add a testable launcher abstraction:

```rust
trait PromotionPrLauncher {
    fn launch(&self, request: PromotePrLaunchRequest) -> Result<PromotePrLaunchResult, String>;
}
```

The real launcher should prepare a prompt and script, then open Terminal to run
headless Codex from the agent-skills checkout.

## TDD Implementation Plan

### Slice 1: Copy To Third-Party Destination

RED:

- Update a promotion behavior test to create a fake `agent-skills` checkout.
- Import `draft-helper`.
- Run promotion with the fake checkout as the target skills repo.
- Assert `third-party/draft-helper/SKILL.md` exists.
- Assert `third-party/draft-helper/import.json` does not exist.
- Assert the local import manifest remains available under `imports_root`.

GREEN:

- Extend the promotion request or operation context with a target skills repo.
- Compute the destination as `<skills-repo>/third-party/<skill-name>`.
- Copy the imported skill to that destination with the existing
  `ExcludeTopLevelImportManifest` behavior.

REFACTOR:

- Rename promotion internals that currently say `canonical_path` when they now
  mean promoted target path.
- Keep destination calculation in one helper.

### Slice 2: Preserve Supporting Files

RED:

- Add a local-path import test with nested support files, scripts, and assets.
- Promote it into a fake agent-skills checkout.
- Assert all normal support files are preserved below
  `third-party/<skill-name>`.

GREEN:

- Reuse the existing recursive copy operation for promotion.

REFACTOR:

- Keep copy policy explicit at the call site so future metadata exclusions are
  easy to audit.

### Slice 3: Third-Party Collision Safety

RED:

- Create `third-party/collision-helper` before promotion.
- Assert promotion fails before writing the import manifest.
- Assert enabled agent symlinks still point at the original import path.

GREEN:

- Move the collision check from `canonical_root/<skill-name>` to
  `<skills-repo>/third-party/<skill-name>`.

REFACTOR:

- Reuse the existing collision error shape if the wording still makes sense.

### Slice 4: Agent Entry Safety Still Holds

RED:

- Keep the existing unsafe entry matrix:
  - real directory
  - regular file
  - external symlink
  - broken symlink
  - wrong managed symlink
- Assert no third-party destination is created and no manifest mutation occurs.

GREEN:

- Preserve the existing preflight order so unsafe agent entries fail before
  mutation.

REFACTOR:

- Keep agent-entry safety independent from the new target repo concerns.

### Slice 5: Relink Enabled Agents To The Promoted Skill

RED:

- Enable an imported skill for Claude Code and Codex in temporary roots.
- Promote it into a fake agent-skills checkout.
- Assert both agent symlinks canonicalize to
  `<skills-repo>/third-party/<skill-name>`.
- Assert action JSON includes remove/create symlink actions with the promoted
  target path.

GREEN:

- Update the relink plan to use the third-party destination path.

REFACTOR:

- Keep action reporting stable and explicit enough for CLI/TUI status output.

### Slice 6: Manifest Mutation Semantics

RED:

- Add one success test that asserts `promoted: true` only after copy and launch
  preparation succeeds.
- Add one fake-launcher failure test that asserts the manifest remains
  `promoted: false` if Codex launch preparation fails.

GREEN:

- Sequence promotion as:
  1. Preflight.
  2. Copy skill.
  3. Prepare and launch PR workflow.
  4. Write manifest.
  5. Relink managed agents.

REFACTOR:

- If rollback becomes necessary, add narrow cleanup for destinations created in
  the same operation. Do not remove pre-existing paths.

### Slice 7: Build The PR Handoff Prompt

RED:

- Unit test a pure prompt builder.
- Given skill name, promoted path, source metadata, optional repository
  metadata, and optional analysis paths, assert the prompt instructs Codex to:
  - create or update a branch in `brian-bell/agent-skills`
  - verify the copied skill files
  - update `README.md`
  - update `AGENTS.md`
  - update `scripts/install-skills.sh`
  - update `third-party/ATTRIBUTION.md`
  - preserve upstream attribution from `import.json`
  - include analysis findings when present
  - run available checks
  - commit, push, and open a PR

GREEN:

- Implement deterministic prompt rendering from structured inputs.

REFACTOR:

- Keep the prompt builder free of filesystem and process-launching concerns.

### Slice 8: Include Analysis Results If Present

RED:

- Add tests for analysis discovery:
  - no report found: prompt states that no analysis report was found and
    promotion still succeeds.
  - report JSON/HTML found: prompt includes those paths.
  - unreadable or malformed report path: promotion records a warning but does
    not block.

GREEN:

- Add a narrow analysis lookup helper that can find the latest matching
  analyzer report for a skill when one exists.

REFACTOR:

- Do not couple promotion to analyzer execution. Promotion only consumes
  existing analysis artifacts.

### Slice 9: Launch Headless Codex In Terminal

RED:

- Unit test launch-plan rendering without opening Terminal.
- Assert the generated script:
  - runs from the agent-skills checkout
  - invokes `codex exec` in headless mode
  - passes the rendered prompt file
  - quotes paths safely
  - refuses missing `codex`
  - refuses a missing target repo checkout

GREEN:

- Implement a `TerminalPromotionPrLauncher` modeled on the existing analyzer
  launcher pattern.
- Write prompt/script files into a cache or temp workspace.
- Launch Terminal with the generated script.

REFACTOR:

- Share only low-risk utilities with the analyzer launcher. Avoid creating a
  broad terminal-launch abstraction unless duplication becomes meaningful.

### Slice 10: CLI And JSON Integration

RED:

- Add a command-level test with a fake launcher:
  - `promote --json --skill command-promote --skills-repo <fake-repo>`
    succeeds.
  - JSON includes the promoted skill name and filesystem actions.
  - JSON includes a PR workflow launch action/result.
- Add a command-level launcher failure test with clear stderr.

GREEN:

- Thread the target repo and launcher through `workflow::execute` and
  `main.rs`.

REFACTOR:

- Keep JSON mapping in `json_adapter` simple. Prefer serializable result data
  over ad hoc CLI-only output.

### Slice 11: TUI Integration

RED:

- Update TUI terminal tests so the promote operation uses the new workflow.
- Assert success status mentions the launched PR workflow or prompt path.

GREEN:

- Pass the real launcher from the terminal adapter.
- Preserve the existing confirmation flow.

REFACTOR:

- Keep reducer state focused on operation status. Do not add filesystem logic to
  TUI state.

### Slice 12: Documentation

RED:

- Add or update documentation expectations manually during review:
  - `promote` now targets the agent-skills third-party directory.
  - promotion launches a PR handoff workflow.
  - default skills repo path and override behavior are documented.

GREEN:

- Update `README.md` and `AGENTS.md` only if the command interface or behavior
  changes in a way users or future agents need to know.

REFACTOR:

- Remove obsolete language that says promotion only copies into the local
  canonical root.

## Acceptance Criteria

- Promotion copies imported skills to
  `/Users/brian/dev/agent-skills/third-party/<skill-name>` by default.
- The destination repo path can be overridden for tests and automation.
- Support files are copied, but the managed top-level `import.json` is not.
- Destination collisions fail before mutation.
- Unsafe agent entries still fail before mutation.
- Local import metadata records successful promotion.
- Enabled managed agent symlinks point at the promoted third-party skill.
- A Terminal window launches a headless Codex PR workflow.
- The Codex prompt asks for documentation, installer script, attribution, and
  analysis-result handling.
- Tests use fake repos, fake launchers, and disposable roots.

## Verification

Run:

```bash
make fmt-check
make clippy
make test
make check
```

Manual smoke test with disposable roots:

```bash
cargo run -- promote --json --skill <imported-skill> \
  --skills-repo /Users/brian/dev/agent-skills \
  --imports-root <disposable-imports> \
  --claude-code-root <disposable-claude> \
  --codex-root <disposable-codex>
```

## Commit And Shipping Notes

- Do not commit or push directly to `main`.
- Do not ship unrelated work on an existing PR.
- After implementation, use the commit workflow for the `skill-importer` change.
- Let the launched headless Codex handle the separate `agent-skills` PR.
