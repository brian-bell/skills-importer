# Rewrite skill-importer in Zig (0.15.1)

## Context

`skill-importer` is a Rust crate (`src/lib.rs` is 2,823 lines; all of `src/`
incl. TUI is ~8,099 lines) that manages local AI skill catalogs for Claude Code
and Codex (discovery, import, enable/disable, promote/delete, JSON automation
output, ratatui TUI). The goal is to rewrite it in **Zig 0.15.1** to drop the
Rust toolchain and ship a small, dependency-light native binary that
cross-compiles trivially with `zig build`.

The rewrite is **phased and CLI-first**: port the core domain model, CLI, and
JSON output into a fully working tool first, then add the TUI later. This plan is
the hardened version of the original sketch — a multi-agent ground-truth pass
against the actual Rust source/tests, the Zig 0.15.1 std API, and the CI/release
contract corrected (a) wrong `lib.rs` line references and glossed test-locked
algorithms, (b) Zig 0.15.1 API assumptions that won't compile or silently
misbehave, and (c) a self-contradictory acceptance oracle and CI section.

### Decisions locked in

- **Acceptance: clean break, v2 import store.** The Zig tool owns its own
  **versioned imports root** (default `<runtime_root>/.skill-importer/v2/imports`).
  It does **not** read or migrate the Rust tool's `.skill-importer/imports`, and
  it need **not** reproduce Rust's content-hash byte layout. This dissolves the
  parity contradiction: the acceptance oracle is **the ported test suite + JSON
  *shape* parity + Zig-internal hash stability**, NOT a byte-for-byte diff
  against the Rust binary. Pre-existing Rust imports/symlinks are out of scope
  (Zig classifies them as external/unmanaged, which is correct behavior).
- **CI: adopt `setup-zig`** pinned to 0.15.1 (replaces `brew install zig`);
  rewrite the `tests/github_workflows.rs` assertions accordingly.
- **Release: GoReleaser `builder: prebuilt`** fed by `zig build -Dtarget=...`
  cross-compiled binaries; preserve the test-locked Homebrew cask/tap strings.
- **Net timeout: best-effort worker-thread** 30s deadline (std.http has no
  socket timeout); document that the socket may linger on expiry.
- Scope cuts: **no TUI** (`src/tui/**`, deferred to Phase 7), **no analyzer**
  (`src/analyzer.rs`, dropped), no `render-analysis-report`. Repository import
  core is ported but the **CLI** keeps today's "unavailable repository provider"
  (repo import is TUI-only).

## Target layout

Mirror the Rust module boundaries:
`src/{main,cli,workflow,discovery,import,promote,skill_store,manifest,
frontmatter,fsutil,json_out,net,git,types}.zig`, plus `src/root_test.zig` — a
test aggregator that `@import`s every mirrored test file (see Phase 1). `tests/`
mirrors `tests/*.rs`; `Makefile` and `build.zig`/`build.zig.zon` replace Cargo.

## Dependency mapping (Rust crate → Zig std)

| Rust | Usage | Zig replacement |
|---|---|---|
| `serde_json` | manifests + JSON output | `std.json` (`parseFromSlice`; `std.json.Stringify` struct — **no** lowercase `stringify`) |
| `sha2` | content hashing | `std.crypto.hash.sha2.Sha256` (`init(.{})`/`update`/`final(&out)`) |
| `ureq` | URL fetch (30s, 1MB cap) | `std.http.Client` (**no timeout** → worker-thread; body → `*std.Io.Writer`; manual 1MB cap) |
| `clap` | CLI parsing | hand-written parser in `cli.zig` |
| `tempfile` | git checkout temp dir | hand-rolled random dir under `TMPDIR` (no `makeTempDir` in 0.15.1) + `defer deleteTree` |
| `git` subprocess | repo clone | `std.process.Child.run` (`git clone --depth 1`) |
| `ratatui`/`crossterm` | TUI | **deferred** (Phase 7) |
| `osascript`/`codex` | analyzer | **dropped** |

Memory: thread `std.mem.Allocator` everywhere; arena-per-operation freed at
command end. Tests use `std.testing.allocator`. Errors use a per-operation
tagged result (see Error design), not bare error sets, since Zig errors carry no
data.

## Factual corrections (verified against the Rust source/tests)

**Treat `tests/*.rs` as the authoritative spec; the `lib.rs:NNNN` hints below are
the verified locations.**

| Original claim | Correction (verified) |
|---|---|
| `clean_frontmatter_value` at `lib.rs:824` | It is at **`lib.rs:2642`**. 824 is `parse_skill_frontmatter`. |
| One frontmatter parser | **Two**: `parse_skill_frontmatter` (824, *strict* — errors on missing delimiters; imports) and `parse_skill_metadata` (2618, *lenient* — returns `Option`; discovery). Port **both**. Also `validate_skill_name` (878 — name must be a single `Normal` path component). |
| Symlink classify at `1490`/`1906` | `create_symlink` at **1906** is right; **1490 is unrelated**. Real classifiers: `agent_entry_status` (**2490**), `classify_symlink_target` (**2521**). |
| Promotion = "stage-copy → atomic swap" (one path) | **Two branches**: fresh `create_dir`+copy (697) and overwrite `replace_promoted_skill_from_import` (**1558**) = `remove_dir_all`-then-`rename` (non-atomic) into a **PID-named hidden staging dir** `.{name}.promotion-staging-{pid}-{index}` (scan 0..1000 for a free name) with **action path rebasing** (`action_with_rebased_path`). `promote.rs` is test-locked on the recorded `copy_file` actions. |
| Repo scan at `1916` | Correct. Add: strict root-skill candidate first, else BFS; `MAX_REPOSITORY_SCAN_DEPTH=8`; sorts by `file_name`; strict on root, `IgnoreInvalid` nested. |
| Root resolution = one chain at `cli.rs:327` | 327 is the struct; logic is `into_discovery_roots` (**339–392**), **not linear** — roots resolve independently: `AGENT_SKILLS_REPO`→canonical only; catalog-repo discovery→imports only; `HOME`→claude/codex + agent-skills default; imports defaults to `<runtime_root>/.skill-importer/imports` (→ **v2** for the Zig tool). |

**Test-locked behaviors that must be ported:**
- `merge_skill` precedence/accumulation (2540): `Canonical=0<Imported=1<AgentOnly=2`;
  `promoted` OR-accumulated; `source_repository` captured once from an imported
  entry; `source`/`analysis_skill_dir` overwritten only on lower precedence;
  `description` filled only if currently `None`.
- **Per-call-site sort keys differ:** discovery sorts full `PathBuf`;
  `hash_directory` and repo scan sort by **`file_name` only**;
  `source_repositories` by `(skill_name, skill_path)`; final skills in BTreeMap
  name order.
- `directory_content_hash` **errors** on non-dir/non-file entries (does not skip).
- `agent_entry_status` edges: symlink→file or resolvable-not-dir ⇒ **Missing**;
  canonicalize `NotFound` ⇒ **BrokenSymlink**; other IO errors ⇒ **propagate**.
- `refuse_collection_collision` (894) reads **every** existing skill's
  frontmatter to detect a name collision under a different dir name.
- Local-import guards: `refuse_reserved_local_skill_entries` (rejects source
  containing `import.json`) + `refuse_imports_root_inside_source`. Two local
  hashing paths: directory (hash + recursive copy) vs markdown file (copy to
  `SKILL.md`, hash the string).
- `store_import` rollback (`create_dir` `AlreadyExists`⇒`Collision`;
  `remove_dir_all` on failure); batch repo import preflight + **reverse-order
  rollback** of created skill paths *and* created import roots.
- Asymmetric draft policy: enable uses `DraftImportPolicy::Reject`
  (unpromoted⇒`NotPromoted`); disable uses `Allow`.
- Executor records `CreateDirectory` **before** `CreateSymlink`; enable
  canonicalizes source before linking so `AlreadyCorrect`/`SkipUnchanged`
  detection matches.
- **`Unpromote`** exists in `workflow.rs` (29, 85) with **no CLI surface** — the
  Zig core `OperationRequest` must include it to match.

## Zig 0.15.1 API corrections (load-bearing)

**Pin exactly 0.15.1** (`.minimum_zig_version = "0.15.1"` in `build.zig.zon` + a
CI version guard). The dev machine runs **0.16.0, which is API-incompatible**
(`Dir`→`std.Io.Dir`, fs ops gain an `io` param, realpath reworked) — never
verify against it; install 0.15.1 locally and in CI.

- **`net.zig` (HIGH):** `std.http.Client` has **no timeout** — implement the 30s
  budget via a `std.Thread` joined against a deadline (best-effort; socket may
  linger). `fetch` streams the body into a caller-supplied `*std.Io.Writer` (not
  a returned string); `.location = .{ .url = url }` is a tagged union. Use
  `std.Io.Writer.Allocating` + post-call `written().len > 1_000_000` reject
  (avoid `fixed` writer overflow bugs). Pass `redirect_buffer` +
  `redirect_behavior` to match ureq's follow-redirects. TLS auto-loads the
  system CA bundle.
- **`json_out.zig` (HIGH — contract):** no `std.json.stringify` (lowercase) and
  no `writeStream` in 0.15.1 — use the `std.json.Stringify` struct with a
  **method-driven explicit emitter** (`beginObject`/`objectField`/`write`/
  `endObject` + a `switch` for enum→string). Key order == call order.
  `Options{ .whitespace = .indent_2 }` matches serde. `emit_null_optional_fields`
  defaults **true** — use conditional `objectField` for the omit-vs-null table.
  serde emits **no** trailing newline; the stdout `\n` is a separate write — and
  **flush** the buffered writer (Writergate).
- **`fsutil.zig` (HIGH):** `statFile` **follows** symlinks (no no-follow option)
  — classify via directory iteration `entry.kind == .sym_link` or
  `std.posix.fstatat(..., AT.SYMLINK_NOFOLLOW)`. Resolve symlink targets with
  **lexical `std.fs.path.resolve`, NOT `realpath`** (realpath requires existence
  and dereferences). Hand-implement `canonicalize_existing_ancestor` (no std
  helper). No recursive copy in std — build on `Dir.walk` + `makePath` +
  `copyFile`, and **recreate `.sym_link` entries via `readLink`+`symLink`**
  (copyFile dereferences). Keep promotion staging on the **same mount** as the
  destination (`rename` fails `RenameAcrossMountPoints`).
- **`manifest.zig`:** `std.json.parseFromSlice(T, gpa, bytes,
  .{ .ignore_unknown_fields = true })`; `Parsed(T)` is arena-owned — dupe slices
  that outlive `deinit()`. On-disk `import.json` is 2-space pretty with **no
  trailing newline** (distinct from stdout).
- **`build.zig`/`build.zig.zon` (MEDIUM, Phase-1 blocker):** `addExecutable`/
  `addTest` require `.root_module = b.createModule(...)` (0.14 direct form
  removed). `build.zig.zon` needs `.name` as an **enum literal**
  (`.skill_importer`), a `.fingerprint`, `.minimum_zig_version`, `.paths`, empty
  `.dependencies`. `std.ArrayList` is **unmanaged-by-default**
  (`var l: std.ArrayList(T) = .empty; try l.append(alloc, x); l.deinit(alloc)`).
  **Test discovery:** `zig build test` only runs `test{}` in the root + imported
  files — add `src/root_test.zig` aggregating every mirrored test file, else
  tests are **silently skipped**. No `makeTempDir` in 0.15.1.
- **`git.zig`:** `std.process.Child.run(.{...,.argv=&.{"git","clone","--depth",
  "1",url,dest},...})`; handle spawn `FileNotFound` as "git not installed."
  Provider = **struct-of-function-pointers** (no traits) so tests inject a fake.
- **Writergate (cross-cutting):** `std.io`→`std.Io.Reader`/`Writer`; main stdout
  is `File.stdout().writer(&buf)` then `&fw.interface`, then **flush**. Custom
  `format` is `fn format(self, *std.Io.Writer) Error!void`, invoked with `{f}`.

## JSON output contract (the port must hit exactly)

- 2-space indent; keys emit in **declaration order** (not alphabetical); `skills`
  array is **name-sorted**. PathBuf→plain string; `imported_at`→**unquoted
  integer**; `content_hash`→`"sha256:"+lowercase-hex`.
- **stdout** gets one trailing `\n`; **on-disk `import.json`** gets none.
- **Omit-vs-null:** omit when `None` for `JsonSkillEntry.description`,
  `*.source_repository`, `SkillAction.{agent,target,source}`; **emit `null`** for
  `RepositorySkillCandidate.description` (the one exception).
- Object key orders (= emit order): inventory `skills`,`source_repositories`;
  `JsonSkillEntry` `name`,`description?`,`source`,`source_repository?`,
  `promoted`,`enablement`,`agent_entries`; `enablement` `claude_code`,`codex`
  (bools from `is_enabled()` — `AgentEnablement` enum is never serialized);
  `ImportManifest` `source_type`,`source_location`,`source_repository?`,
  `imported_at`,`content_hash`,`promoted`; `ImportResult` `skill_name`,
  `skill_path`,`manifest_path`,`manifest`,`actions`; `SkillOperationResult`
  `skill_name`,`actions`; `SkillAction` `action`,`agent?`,`path`,`target?`,
  `source?`.
- **Internally-tagged** `RepositoryImportResult` (`tag="kind"`): `imported`
  (ImportResult fields flattened beside the tag), `imported_batch`
  (`{"kind":...,"imports":[...]}`), `selection` (selection fields flattened).
- Enum strings (snake_case): `JsonSkillSource` canonical/imported/agent_only;
  `AgentEntryStatus` missing/skill_directory/canonical_symlink/imported_symlink/
  external_symlink/broken_symlink; `ImportSourceType` markdown/local_path/url/
  repository; `ImportActionKind` create_directory/write_skill/copy_file/
  write_manifest; `SkillActionKind` create_directory/create_symlink/
  remove_symlink/copy_file/write_manifest/remove_directory/skip_unchanged;
  `SkillAgent` **in JSON** claude_code/codex (**CLI input uses hyphen**
  `claude-code`); `RepositoryImportResult.kind` imported/imported_batch/selection.
- Action counts: markdown/url import emit exactly `create_directory`,
  `write_skill`, `write_manifest`. Malformed `import.json` makes `list` **fail**
  (non-zero exit); missing roots ⇒ empty `skills`, success.

## Error-with-payload design (decide in Phase 1)

Every `ImportError`/`SkillOperationError` variant carries data that Zig error
sets can't hold, and `main.rs` renders payloads into test-asserted text. Return
a per-operation tagged result, not a bare error set:

```zig
pub fn Result(comptime Ok: type) type {
    return union(enum) { ok: Ok, err: ErrorInfo };
}
pub const ErrorInfo = struct {
    kind: ErrorKind,                          // mirrors every Rust variant
    name: ?[]const u8 = null, path: ?[]const u8 = null,
    field: ?[]const u8 = null, message: ?[]const u8 = null,
    reason: ?[]const u8 = null, url: ?[]const u8 = null,
    repository: ?[]const u8 = null,
    actions: std.ArrayList(SkillAction) = .empty, // replaces SkillOperationFailure.actions
};
```

- All strings arena-owned; freed with the per-operation arena.
- Translate std error sets (`FetchError`, `RealPathError`, `ReadLinkError`,
  `Child`) into domain `ErrorKind`s at module boundaries.
- The partial-action list is **test-observable only**, not user-facing JSON
  (workflow.rs renders only `failure.error`) — don't serialize it.

## CLI surface

Commands: `list`, `import markdown` (stdin or `--source-location`),
`import path <path>`, `import url <url>`, `enable <skill> [agents]`,
`disable <skill> [agents]`, `promote <skill>`, `delete <skill>`, `tui` (stub
that parses roots, rejects `--json`, and errors "not yet implemented"). `parse_agent`
accepts `claude-code`/`codex`. Hand-written parser replaces clap's
`allow_hyphen_values` for **every** value option (e.g. `-skill.md`-style
values), and reproduces clap's exact uncolored help/usage/error strings
(help-as-success to stdout). Root resolution per `into_discovery_roots`
(independent per-root, conditional HOME, `AGENT_SKILLS_REPO` absolute
validation). `main.zig` wires the net fetcher + unavailable repo provider, prints
`skill-importer: {error}`, and sets exit codes.

## Implementation phases (TDD; `zig build test` per phase)

- [x] **Phase 1 — Scaffold + cross-cutting decisions.** `build.zig` (root_module
form), `build.zig.zon` (enum name, fingerprint, `minimum_zig_version=0.15.1`,
empty deps), `src/root_test.zig` aggregator, Makefile stubs, `types.zig`. **Lock
now:** full `ErrorInfo` union, complete `json_out.zig` emitter spec, `ArrayList`
unmanaged convention, arena-per-operation, Writergate flush. *Gate:* `zig build`
+ `zig fmt --check`; aggregator runs.

- [x] **Phase 2 — frontmatter + manifest + hashing + fsutil.** Both frontmatter
parsers + `validate_skill_name`; manifest read/write (dual newline); content
hash (big-endian u64 length prefixes, POSIX path bytes, `file_name`-sorted, error
on non-dir/non-file); fsutil symlink classify (no-follow), lexical target
resolution, recursive copy recreating symlinks, hand-rolled
`canonicalize_existing_ancestor`. *Tests (new, fixture-level):* known-tree →
exact `sha256:…` (assert a **Zig-computed golden**, not the Rust value);
markdown-string hash; symlink classify cases; ancestor canonicalize.

- [ ] **Phase 3 — discovery + JSON contract.** `discovery.zig` (full `merge_skill`,
per-call-site sort keys) + complete `json_out.zig`. *Tests:* port `discovery.rs`
(~25) + `list_command.rs` (~7) incl. the stable-enum-string test; add
merge-collision + shared-prefix ordering unit tests. *Gate:* JSON-shape parity.
  - [x] Added `discovery.zig` with missing-root handling, owned-root discovery,
        agent-only entries, source precedence, imported repository metadata, and
        source-repository aggregation.
  - [x] Added `json_out.zig` with explicit inventory and import-result JSON
        emitters for stable key order and enum strings.
  - [x] Added focused Zig tests for discovery merge behavior and JSON shape.
  - [ ] Port the full `discovery.rs` and `list_command.rs` suites.
  - [ ] Complete parity checks for all list-command JSON cases.

- [ ] **Phase 4a — markdown/url/path imports + net.zig.** `import.zig`, local-import
guards, dual local hashing, `store_import` rollback; `net.zig` (writer-injection
fetch, Allocating + size check, worker-thread 30s deadline, redirects). **Imports
write to the v2 root.** *Tests:* `import_markdown.rs` (~8), `import_local_path.rs`
(~14), `import_url.rs` (~5) incl. loopback HTTP + over-1MB.
  - [x] Added markdown import validation, v2 import storage, `SKILL.md` writing,
        `import.json` writing, content hashing, and ordered import actions.
  - [x] Added import-result JSON output and command-level markdown import smoke.
  - [ ] Add path imports, local import guards, rollback, and dual local hashing.
  - [ ] Add URL imports and `net.zig`.
  - [ ] Port the full markdown/path/url import suites.

- [ ] **Phase 4b — repository scan core + git.zig.** BFS depth-8 (no-follow),
strict-vs-IgnoreInvalid, normalized selectors, batch preflight + reverse-order
rollback, `kind`-tagged selection JSON; `git.zig` via `Child.run` + fake
provider. *Tests:* `import_repository.rs` (~20).

- [ ] **Phase 5a — enable/disable.** `skill_store.zig` planner+executor: asymmetric
`DraftImportPolicy`, managed-symlink safety checks, `CreateDirectory`-before-
`CreateSymlink`, source canonicalization, first-seen agent dedupe. *Tests:*
`enable_disable.rs` (~15).

- [ ] **Phase 5b — promote/unpromote/delete.** `promote.zig`: both branches (fresh
create+copy vs overwrite remove-then-rename), PID-named staging (0..1000), action
rebasing, relink, manifest `promoted=true`; delete enabled-guard; `Unpromote`
core (no CLI). *Tests:* `promote.rs` (~15), `delete_import.rs` (~7).

- [ ] **Phase 6 — workflow + CLI + main.** `workflow.zig` (incl. `Unpromote`);
`cli.zig` hand-parser (independent per-root resolution, exact clap strings,
hyphen-value handling, `tui` stub); `main.zig` wiring. *Tests:* `workflow.rs`
(~6) incl. pretty-json-with-trailing-newline; port `cli.rs` unit tests.
*Milestone: full CLI/JSON parity minus TUI/analyzer.*
  - [x] Added minimal `main.zig` wiring for `list --json`, `import markdown
        --json`, root flags, stdout newline/flush, and `tui` stub.
  - [ ] Add `workflow.zig`.
  - [ ] Add full `cli.zig` parser and parity tests.
  - [ ] Wire remaining commands.

- [ ] **Phase 6.5 — CI/Release (one atomic PR, suite never red).** Rewrite all 4
workflow YAMLs to **`setup-zig`@pinned-0.15.1** + `zig build test` + `zig fmt
--check`; Makefile recipe bodies + `ROOT_FLAGS`/`--` arg-passing; `.goreleaser
.yml` to **`builder: prebuilt`** with a darwin/linux × amd64/arm64 `zig build
-Dtarget=...` matrix (keep the locked `name_template`, `homebrew_casks`,
quarantine caveats, `brian-bell/homebrew-tap`); and **rewrite all 6
`github_workflows.rs` fns** in the same PR — replace `cargo fmt/clippy/test`
with the Zig equivalents, drop the `!mlugg/setup-zig@` negative + `brew install
zig` assertions, and swap `builder: rust`/rust-target-triples for `builder:
prebuilt`. Map `make clippy`→no-op/`zig build test`; `make check`→`fmt-check +
test`. Gate on local `goreleaser release --snapshot`.
  - [x] Updated Makefile recipes for Zig build/test/fmt/check and v2 disposable
        import roots.
  - [x] Added `ZIG_DIRECT_TARGET=aarch64-macos.15.0` local workaround for Zig
        0.15.1 build-runner linkage on newer macOS hosts.
  - [ ] Update GitHub Actions workflows, GoReleaser config, and
        `tests/github_workflows.rs`.
  - [ ] Run snapshot release verification.

- [ ] **Phase 7 — TUI (deferred, separate effort).** Re-introduce `src/tui/**` using
libvaxis or a hand-rolled termios+ANSI layer; port the reducer model and
`tui_*.rs` tests against a headless backend. Out of scope for this plan's
acceptance.

## Verification

Per phase and at the end:

```bash
zig fmt --check src
zig build
zig build test          # ported suite via src/root_test.zig — the primary oracle
make check              # fmt-check + test (CI parity)
```

End-to-end smoke against **disposable** roots (real `~/.claude/skills` /
`~/.agents/skills` untouched), imports in the v2 dir:

```bash
zig build
CANONICAL_ROOT=.skill-importer/dev/canonical \
IMPORTS_ROOT=.skill-importer/dev/v2/imports \
CLAUDE_CODE_ROOT=.skill-importer/dev/claude \
CODEX_ROOT=.skill-importer/dev/codex \
  ./zig-out/bin/skill-importer list
printf '%s\n' '---' 'name: demo' 'description: d' '---' \
  | ./zig-out/bin/skill-importer import markdown
./zig-out/bin/skill-importer promote demo
./zig-out/bin/skill-importer enable demo claude-code
./zig-out/bin/skill-importer list
```

**Acceptance:** the ported test suite is the contract. Validate JSON **shape**
(keys/enums/formatting/omit-vs-null/trailing newline) against the Rust tool, but
do **not** require content-hash equality — the v2 store makes Zig hashes
independent.

## Risks / watch-items

- **0.15.1 vs local 0.16.0** — install/pin 0.15.1; CI guard on version.
- **std.http / std.json / Writergate churn** — isolate behind `net.zig`/
  `json_out.zig`; method-driven emitter, worker-thread timeout, explicit flush.
- **Silent test skipping** — the `root_test.zig` aggregator is mandatory.
- **Symlink no-follow** — `statFile` follows; use `entry.kind`/`fstatat` and
  lexical resolve, or discovery/scan/classification all break.
- **CI/release atomicity** — workflow YAML, Makefile, `.goreleaser.yml`, and the
  6 `github_workflows.rs` fns must change in one PR.
- **Error-with-payload** — full `ErrorInfo` union decided in Phase 1, applied
  consistently.
- **Allocator discipline** — arena per CLI operation freed at command end.
