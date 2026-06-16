use std::fs;
use std::os::unix::fs as unix_fs;

use skill_importer::{
    AgentEnablement, AgentEntries, AgentEntryStatus, DiscoveryRoots, SkillEntry, SkillInventory,
    SkillSource, discover_skills, inventory_to_json,
};

#[test]
fn canonical_skill_enabled_for_both_agents_appears_once() {
    let temp = tempfile::tempdir().expect("tempdir");
    let canonical_root = temp.path().join("canonical");
    let imports_root = temp.path().join("imports");
    let claude_root = temp.path().join("claude");
    let codex_root = temp.path().join("codex");

    fs::create_dir_all(&canonical_root).expect("canonical root");
    fs::create_dir_all(&claude_root).expect("claude root");
    fs::create_dir_all(&codex_root).expect("codex root");

    let skill_dir = canonical_root.join("checkout-helper");
    fs::create_dir_all(&skill_dir).expect("skill dir");
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: checkout-helper
description: Helps checkout flows stay tidy.
---

# Checkout Helper
"#,
    )
    .expect("skill file");

    unix_fs::symlink(&skill_dir, claude_root.join("checkout-helper")).expect("claude symlink");
    unix_fs::symlink(&skill_dir, codex_root.join("checkout-helper")).expect("codex symlink");

    let inventory = discover_skills(&DiscoveryRoots {
        canonical_root,
        imports_root,
        claude_code_root: claude_root,
        codex_root,
    })
    .expect("discovery succeeds with a missing imports root");

    assert_eq!(inventory.skills.len(), 1);

    let skill = &inventory.skills[0];
    assert_eq!(skill.name, "checkout-helper");
    assert_eq!(
        skill.description.as_deref(),
        Some("Helps checkout flows stay tidy.")
    );
    assert_eq!(skill.source, SkillSource::Canonical);
    assert_eq!(skill.enablement, AgentEnablement::Both);
    assert_eq!(
        skill.agent_entries.claude_code,
        AgentEntryStatus::CanonicalSymlink
    );
    assert_eq!(
        skill.agent_entries.codex,
        AgentEntryStatus::CanonicalSymlink
    );
}

#[test]
fn canonical_skill_not_linked_to_agent_roots_is_reported_as_disabled() {
    let temp = tempfile::tempdir().expect("tempdir");
    let canonical_root = temp.path().join("canonical");
    let claude_root = temp.path().join("missing-claude");
    let codex_root = temp.path().join("missing-codex");

    fs::create_dir_all(canonical_root.join("solo-skill")).expect("skill dir");
    fs::write(
        canonical_root.join("solo-skill").join("SKILL.md"),
        r#"---
name: solo-skill
description: Stays available without being enabled.
---
"#,
    )
    .expect("skill file");

    let inventory = discover_skills(&DiscoveryRoots {
        canonical_root,
        imports_root: temp.path().join("missing-imports"),
        claude_code_root: claude_root,
        codex_root,
    })
    .expect("missing roots are treated as empty");

    assert_eq!(inventory.skills.len(), 1);
    assert_eq!(inventory.skills[0].name, "solo-skill");
    assert_eq!(inventory.skills[0].enablement, AgentEnablement::Neither);
    assert_eq!(
        inventory.skills[0].agent_entries.claude_code,
        AgentEntryStatus::Missing
    );
    assert_eq!(
        inventory.skills[0].agent_entries.codex,
        AgentEntryStatus::Missing
    );
}

#[test]
fn canonical_skills_can_be_enabled_for_only_one_agent() {
    let temp = tempfile::tempdir().expect("tempdir");
    let canonical_root = temp.path().join("canonical");
    let claude_root = temp.path().join("claude");
    let codex_root = temp.path().join("codex");

    fs::create_dir_all(&canonical_root).expect("canonical root");
    fs::create_dir_all(&claude_root).expect("claude root");
    fs::create_dir_all(&codex_root).expect("codex root");

    let claude_only = write_skill(
        &canonical_root,
        "claude-only",
        "Only Claude Code uses this.",
    );
    let codex_only = write_skill(&canonical_root, "codex-only", "Only Codex uses this.");

    unix_fs::symlink(&claude_only, claude_root.join("claude-only")).expect("claude symlink");
    unix_fs::symlink(&codex_only, codex_root.join("codex-only")).expect("codex symlink");

    let inventory = discover_skills(&DiscoveryRoots {
        canonical_root,
        imports_root: temp.path().join("missing-imports"),
        claude_code_root: claude_root,
        codex_root,
    })
    .expect("discovery succeeds");

    let claude_skill = inventory
        .skills
        .iter()
        .find(|skill| skill.name == "claude-only")
        .expect("claude-only skill");
    assert_eq!(claude_skill.enablement, AgentEnablement::ClaudeCode);
    assert_eq!(
        claude_skill.agent_entries.claude_code,
        AgentEntryStatus::CanonicalSymlink
    );
    assert_eq!(claude_skill.agent_entries.codex, AgentEntryStatus::Missing);

    let codex_skill = inventory
        .skills
        .iter()
        .find(|skill| skill.name == "codex-only")
        .expect("codex-only skill");
    assert_eq!(codex_skill.enablement, AgentEnablement::Codex);
    assert_eq!(
        codex_skill.agent_entries.claude_code,
        AgentEntryStatus::Missing
    );
    assert_eq!(
        codex_skill.agent_entries.codex,
        AgentEntryStatus::CanonicalSymlink
    );
}

#[test]
fn broken_agent_symlinks_are_reported_without_counting_as_enabled() {
    let temp = tempfile::tempdir().expect("tempdir");
    let claude_root = temp.path().join("claude");
    fs::create_dir_all(&claude_root).expect("claude root");
    unix_fs::symlink(
        temp.path().join("missing-target"),
        claude_root.join("missing-skill"),
    )
    .expect("broken symlink");

    let inventory = discover_skills(&DiscoveryRoots {
        canonical_root: temp.path().join("missing-canonical"),
        imports_root: temp.path().join("missing-imports"),
        claude_code_root: claude_root,
        codex_root: temp.path().join("missing-codex"),
    })
    .expect("discovery succeeds");

    assert_eq!(inventory.skills.len(), 1);
    assert_eq!(inventory.skills[0].name, "missing-skill");
    assert_eq!(inventory.skills[0].enablement, AgentEnablement::Neither);
    assert_eq!(
        inventory.skills[0].agent_entries.claude_code,
        AgentEntryStatus::BrokenSymlink
    );
}

#[test]
fn regular_files_in_agent_roots_do_not_create_skill_entries() {
    let temp = tempfile::tempdir().expect("tempdir");
    let claude_root = temp.path().join("claude");
    fs::create_dir_all(&claude_root).expect("claude root");
    fs::write(claude_root.join("README.md"), "notes, not a skill").expect("regular file");

    let inventory = discover_skills(&DiscoveryRoots {
        canonical_root: temp.path().join("missing-canonical"),
        imports_root: temp.path().join("missing-imports"),
        claude_code_root: claude_root,
        codex_root: temp.path().join("missing-codex"),
    })
    .expect("discovery succeeds");

    assert!(inventory.skills.is_empty());
}

#[test]
fn symlinked_skill_directories_in_collection_roots_are_discovered() {
    let temp = tempfile::tempdir().expect("tempdir");
    let canonical_root = temp.path().join("canonical");
    let source_root = temp.path().join("source");
    fs::create_dir_all(&canonical_root).expect("canonical root");
    fs::create_dir_all(&source_root).expect("source root");

    let source_skill = write_skill(
        &source_root,
        "linked-canonical",
        "Discovered through a collection symlink.",
    );
    unix_fs::symlink(&source_skill, canonical_root.join("linked-canonical"))
        .expect("collection symlink");

    let inventory = discover_skills(&DiscoveryRoots {
        canonical_root,
        imports_root: temp.path().join("missing-imports"),
        claude_code_root: temp.path().join("missing-claude"),
        codex_root: temp.path().join("missing-codex"),
    })
    .expect("discovery succeeds");

    assert_eq!(inventory.skills.len(), 1);
    assert_eq!(inventory.skills[0].name, "linked-canonical");
    assert_eq!(inventory.skills[0].source, SkillSource::Canonical);
}

#[test]
fn imported_agent_only_and_agent_entry_statuses_are_reported() {
    let temp = tempfile::tempdir().expect("tempdir");
    let imports_root = temp.path().join("imports");
    let claude_root = temp.path().join("claude");
    let codex_root = temp.path().join("codex");
    let external_root = temp.path().join("external");

    fs::create_dir_all(&claude_root).expect("claude root");
    fs::create_dir_all(&codex_root).expect("codex root");
    fs::create_dir_all(&external_root).expect("external root");

    let imported = write_skill(&imports_root, "imported-skill", "Imported but unpromoted.");
    unix_fs::symlink(&imported, claude_root.join("imported-skill")).expect("imported symlink");

    let external = write_skill(&external_root, "external-skill", "Managed somewhere else.");
    unix_fs::symlink(&external, codex_root.join("external-skill")).expect("external symlink");

    write_skill(
        &claude_root,
        "agent-directory",
        "A real agent-root directory.",
    );

    let inventory = discover_skills(&DiscoveryRoots {
        canonical_root: temp.path().join("missing-canonical"),
        imports_root,
        claude_code_root: claude_root,
        codex_root,
    })
    .expect("discovery succeeds");

    let imported_skill = find_skill(&inventory, "imported-skill");
    assert_eq!(imported_skill.source, SkillSource::Imported);
    assert_eq!(
        imported_skill.agent_entries.claude_code,
        AgentEntryStatus::ImportedSymlink
    );

    let external_skill = find_skill(&inventory, "external-skill");
    assert_eq!(external_skill.source, SkillSource::AgentOnly);
    assert_eq!(
        external_skill.agent_entries.codex,
        AgentEntryStatus::ExternalSymlink
    );

    let agent_directory = find_skill(&inventory, "agent-directory");
    assert_eq!(agent_directory.source, SkillSource::AgentOnly);
    assert_eq!(
        agent_directory.agent_entries.claude_code,
        AgentEntryStatus::SkillDirectory
    );
}

#[test]
fn canonical_source_wins_when_skill_exists_in_canonical_and_imports() {
    let temp = tempfile::tempdir().expect("tempdir");
    let canonical_root = temp.path().join("canonical");
    let imports_root = temp.path().join("imports");

    write_skill(&canonical_root, "shared-skill", "Canonical description.");
    write_skill(&imports_root, "shared-skill", "Imported description.");

    let inventory = discover_skills(&DiscoveryRoots {
        canonical_root,
        imports_root,
        claude_code_root: temp.path().join("missing-claude"),
        codex_root: temp.path().join("missing-codex"),
    })
    .expect("discovery succeeds");

    assert_eq!(inventory.skills.len(), 1);
    assert_eq!(inventory.skills[0].source, SkillSource::Canonical);
    assert_eq!(
        inventory.skills[0].description.as_deref(),
        Some("Canonical description.")
    );
}

#[test]
fn promoted_import_metadata_survives_canonical_source_precedence() {
    let temp = tempfile::tempdir().expect("tempdir");
    let canonical_root = temp.path().join("canonical");
    let imports_root = temp.path().join("imports");

    write_skill(&canonical_root, "shared-skill", "Canonical description.");
    let imported = write_skill(&imports_root, "shared-skill", "Imported description.");
    write_import_manifest(&imported, true);

    let inventory = discover_skills(&DiscoveryRoots {
        canonical_root,
        imports_root,
        claude_code_root: temp.path().join("missing-claude"),
        codex_root: temp.path().join("missing-codex"),
    })
    .expect("discovery succeeds");

    assert_eq!(inventory.skills.len(), 1);
    assert_eq!(inventory.skills[0].source, SkillSource::Canonical);
    assert_eq!(
        inventory.skills[0].description.as_deref(),
        Some("Canonical description.")
    );
    assert!(inventory.skills[0].promoted);
}

#[test]
fn source_repository_list_survives_canonical_source_precedence() {
    let temp = tempfile::tempdir().expect("tempdir");
    let canonical_root = temp.path().join("canonical");
    let imports_root = temp.path().join("imports");

    write_skill(&canonical_root, "shared-skill", "Canonical description.");
    let imported = write_skill(&imports_root, "shared-skill", "Imported description.");
    write_repository_import_manifest(
        &imported,
        "https://example.test/shared.git",
        "skills/shared-skill",
    );

    let inventory = discover_skills(&DiscoveryRoots {
        canonical_root,
        imports_root,
        claude_code_root: temp.path().join("missing-claude"),
        codex_root: temp.path().join("missing-codex"),
    })
    .expect("discovery succeeds");

    assert_eq!(inventory.skills.len(), 1);
    assert_eq!(inventory.skills[0].source, SkillSource::Canonical);
    assert_eq!(inventory.skills[0].source_repository, None);
    assert_eq!(
        inventory.source_repositories,
        vec![skill_importer::SourceRepositoryEntry {
            repository: "https://example.test/shared.git".to_string(),
            skills: vec![skill_importer::SourceRepositorySkill {
                skill_name: "shared-skill".to_string(),
                skill_path: "skills/shared-skill".to_string(),
            }],
        }]
    );
}

#[test]
fn repository_import_metadata_is_discovered_for_imported_skills() {
    let temp = tempfile::tempdir().expect("tempdir");
    let imports_root = temp.path().join("imports");
    let imported = write_skill(&imports_root, "repo-helper", "Imported from a repository.");
    write_repository_import_manifest(&imported, "https://example.test/skills.git", "repo-helper");

    let inventory = discover_skills(&DiscoveryRoots {
        canonical_root: temp.path().join("missing-canonical"),
        imports_root,
        claude_code_root: temp.path().join("missing-claude"),
        codex_root: temp.path().join("missing-codex"),
    })
    .expect("discovery succeeds");

    let skill = find_skill(&inventory, "repo-helper");
    assert_eq!(
        skill.source_repository.as_ref(),
        Some(&skill_importer::ImportSourceRepository {
            repository: "https://example.test/skills.git".to_string(),
            skill_path: "repo-helper".to_string(),
        })
    );
}

#[test]
fn legacy_import_manifest_without_repository_metadata_still_discovers() {
    let temp = tempfile::tempdir().expect("tempdir");
    let imports_root = temp.path().join("imports");
    let imported = write_skill(
        &imports_root,
        "legacy-helper",
        "Imported before source lists.",
    );
    write_import_manifest(&imported, false);

    let inventory = discover_skills(&DiscoveryRoots {
        canonical_root: temp.path().join("missing-canonical"),
        imports_root,
        claude_code_root: temp.path().join("missing-claude"),
        codex_root: temp.path().join("missing-codex"),
    })
    .expect("legacy manifest is valid");

    let skill = find_skill(&inventory, "legacy-helper");
    assert_eq!(skill.source_repository, None);
    assert!(inventory.source_repositories.is_empty());
}

#[test]
fn source_repositories_are_grouped_and_sorted_from_imported_skills() {
    let temp = tempfile::tempdir().expect("tempdir");
    let imports_root = temp.path().join("imports");
    let beta = write_skill(&imports_root, "repo-beta", "Second repository skill.");
    write_repository_import_manifest(&beta, "https://example.test/two.git", "beta");
    let alpha = write_skill(&imports_root, "repo-alpha", "First repository skill.");
    write_repository_import_manifest(&alpha, "https://example.test/one.git", "nested/alpha");
    let root = write_skill(&imports_root, "repo-root", "Root repository skill.");
    write_repository_import_manifest(&root, "https://example.test/one.git", ".");

    let inventory = discover_skills(&DiscoveryRoots {
        canonical_root: temp.path().join("missing-canonical"),
        imports_root,
        claude_code_root: temp.path().join("missing-claude"),
        codex_root: temp.path().join("missing-codex"),
    })
    .expect("discovery succeeds");

    assert_eq!(
        inventory.source_repositories,
        vec![
            skill_importer::SourceRepositoryEntry {
                repository: "https://example.test/one.git".to_string(),
                skills: vec![
                    skill_importer::SourceRepositorySkill {
                        skill_name: "repo-alpha".to_string(),
                        skill_path: "nested/alpha".to_string(),
                    },
                    skill_importer::SourceRepositorySkill {
                        skill_name: "repo-root".to_string(),
                        skill_path: ".".to_string(),
                    },
                ],
            },
            skill_importer::SourceRepositoryEntry {
                repository: "https://example.test/two.git".to_string(),
                skills: vec![skill_importer::SourceRepositorySkill {
                    skill_name: "repo-beta".to_string(),
                    skill_path: "beta".to_string(),
                }],
            },
        ]
    );
}

#[test]
fn malformed_import_manifest_for_imported_skill_fails_discovery() {
    let temp = tempfile::tempdir().expect("tempdir");
    let imports_root = temp.path().join("imports");
    let imported = write_skill(&imports_root, "broken-manifest", "Imported skill.");
    fs::write(imported.join("import.json"), "{not valid json").expect("manifest");

    let error = discover_skills(&DiscoveryRoots {
        canonical_root: temp.path().join("missing-canonical"),
        imports_root,
        claude_code_root: temp.path().join("missing-claude"),
        codex_root: temp.path().join("missing-codex"),
    })
    .expect_err("malformed import manifest should fail discovery");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn quoted_frontmatter_values_strip_one_matching_quote_pair() {
    let temp = tempfile::tempdir().expect("tempdir");
    let canonical_root = temp.path().join("canonical");
    let skill_dir = canonical_root.join("quoted-skill");
    fs::create_dir_all(&skill_dir).expect("skill dir");
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: quoted-skill
description: """quoted"""
---
"#,
    )
    .expect("skill file");

    let inventory = discover_skills(&DiscoveryRoots {
        canonical_root,
        imports_root: temp.path().join("missing-imports"),
        claude_code_root: temp.path().join("missing-claude"),
        codex_root: temp.path().join("missing-codex"),
    })
    .expect("discovery succeeds");

    assert_eq!(
        inventory.skills[0].description.as_deref(),
        Some(r#"""quoted"""#)
    );
}

#[test]
fn analysis_skill_dir_tracks_canonical_imported_and_duplicate_precedence_without_json_leak() {
    let temp = tempfile::tempdir().expect("tempdir");
    let canonical_root = temp.path().join("canonical");
    let imports_root = temp.path().join("imports");
    fs::create_dir_all(&canonical_root).expect("canonical root");
    fs::create_dir_all(&imports_root).expect("imports root");

    let canonical = write_skill(&canonical_root, "shared-helper", "Canonical wins.");
    write_skill(&imports_root, "shared-helper", "Imported loses.");
    let imported = write_skill(&imports_root, "imported-helper", "Imported path.");

    let inventory = discover_skills(&DiscoveryRoots {
        canonical_root,
        imports_root,
        claude_code_root: temp.path().join("claude"),
        codex_root: temp.path().join("codex"),
    })
    .expect("discover");

    let shared = find_skill(&inventory, "shared-helper");
    assert_eq!(shared.source, SkillSource::Canonical);
    assert_eq!(
        shared.analysis_skill_dir.as_deref(),
        Some(fs::canonicalize(canonical).expect("canonical").as_path())
    );
    let imported_skill = find_skill(&inventory, "imported-helper");
    assert_eq!(
        imported_skill.analysis_skill_dir.as_deref(),
        Some(fs::canonicalize(imported).expect("imported").as_path())
    );

    let json = serde_json::to_value(inventory_to_json(&inventory)).expect("json");
    assert_eq!(
        json["skills"][0]["analysis_skill_dir"],
        serde_json::Value::Null
    );
    assert!(
        !serde_json::to_string(&json)
            .expect("json string")
            .contains(temp.path().to_string_lossy().as_ref())
    );
}

#[test]
fn duplicate_collection_names_use_first_lexical_entry_for_analysis_path() {
    let temp = tempfile::tempdir().expect("tempdir");
    let canonical_root = temp.path().join("canonical");
    fs::create_dir_all(&canonical_root).expect("canonical root");

    let first = write_skill_with_metadata(&canonical_root, "a-entry", "same-name", "First.");
    write_skill_with_metadata(&canonical_root, "z-entry", "same-name", "Second.");

    let inventory = discover_skills(&DiscoveryRoots {
        canonical_root,
        imports_root: temp.path().join("imports"),
        claude_code_root: temp.path().join("claude"),
        codex_root: temp.path().join("codex"),
    })
    .expect("discover");

    let skill = find_skill(&inventory, "same-name");
    assert_eq!(
        skill.analysis_skill_dir.as_deref(),
        Some(fs::canonicalize(first).expect("first").as_path())
    );
}

#[test]
fn duplicate_imported_names_keep_first_lexical_source_repository_metadata() {
    let temp = tempfile::tempdir().expect("tempdir");
    let imports_root = temp.path().join("imports");
    fs::create_dir_all(&imports_root).expect("imports root");

    let first = write_skill_with_metadata(&imports_root, "a-entry", "same-name", "First.");
    write_repository_import_manifest(&first, "https://example.test/first.git", "skills/first");
    let second = write_skill_with_metadata(&imports_root, "z-entry", "same-name", "Second.");
    write_repository_import_manifest(&second, "https://example.test/second.git", "skills/second");

    let inventory = discover_skills(&DiscoveryRoots {
        canonical_root: temp.path().join("canonical"),
        imports_root,
        claude_code_root: temp.path().join("claude"),
        codex_root: temp.path().join("codex"),
    })
    .expect("discover");

    let skill = find_skill(&inventory, "same-name");
    assert_eq!(
        skill.source_repository.as_ref(),
        Some(&skill_importer::ImportSourceRepository {
            repository: "https://example.test/first.git".to_string(),
            skill_path: "skills/first".to_string(),
        })
    );
    assert_eq!(
        inventory.source_repositories,
        vec![skill_importer::SourceRepositoryEntry {
            repository: "https://example.test/first.git".to_string(),
            skills: vec![skill_importer::SourceRepositorySkill {
                skill_name: "same-name".to_string(),
                skill_path: "skills/first".to_string(),
            }],
        }]
    );
}

#[test]
fn canonical_skill_with_duplicate_imports_keeps_first_imported_repository_metadata() {
    let temp = tempfile::tempdir().expect("tempdir");
    let canonical_root = temp.path().join("canonical");
    let imports_root = temp.path().join("imports");
    fs::create_dir_all(&imports_root).expect("imports root");
    write_skill(&canonical_root, "same-name", "Canonical wins.");

    let first = write_skill_with_metadata(&imports_root, "a-entry", "same-name", "First.");
    write_repository_import_manifest(&first, "https://example.test/first.git", "skills/first");
    let second = write_skill_with_metadata(&imports_root, "z-entry", "same-name", "Second.");
    write_repository_import_manifest(&second, "https://example.test/second.git", "skills/second");

    let inventory = discover_skills(&DiscoveryRoots {
        canonical_root,
        imports_root,
        claude_code_root: temp.path().join("claude"),
        codex_root: temp.path().join("codex"),
    })
    .expect("discover");

    let skill = find_skill(&inventory, "same-name");
    assert_eq!(skill.source, SkillSource::Canonical);
    assert_eq!(skill.source_repository, None);
    assert_eq!(
        inventory.source_repositories,
        vec![skill_importer::SourceRepositoryEntry {
            repository: "https://example.test/first.git".to_string(),
            skills: vec![skill_importer::SourceRepositorySkill {
                skill_name: "same-name".to_string(),
                skill_path: "skills/first".to_string(),
            }],
        }]
    );
}

#[test]
fn collection_symlink_outside_root_is_discovered_but_not_analyzable() {
    let temp = tempfile::tempdir().expect("tempdir");
    let canonical_root = temp.path().join("canonical");
    let external_root = temp.path().join("external");
    fs::create_dir_all(&canonical_root).expect("canonical root");
    let external = write_skill(&external_root, "external-helper", "External.");
    unix_fs::symlink(&external, canonical_root.join("external-helper")).expect("symlink");

    let inventory = discover_skills(&DiscoveryRoots {
        canonical_root,
        imports_root: temp.path().join("imports"),
        claude_code_root: temp.path().join("claude"),
        codex_root: temp.path().join("codex"),
    })
    .expect("discover");

    let skill = find_skill(&inventory, "external-helper");
    assert_eq!(skill.source, SkillSource::Canonical);
    assert_eq!(skill.analysis_skill_dir, None);
}

#[test]
fn agent_only_analysis_paths_use_real_skill_directories_with_claude_precedence() {
    let temp = tempfile::tempdir().expect("tempdir");
    let claude_root = temp.path().join("claude");
    let codex_root = temp.path().join("codex");
    fs::create_dir_all(&claude_root).expect("claude root");
    fs::create_dir_all(&codex_root).expect("codex root");
    let claude = write_skill(&claude_root, "agent-helper", "Claude.");
    write_skill(&codex_root, "agent-helper", "Codex.");
    let missing_metadata = claude_root.join("no-skill-file");
    fs::create_dir_all(&missing_metadata).expect("missing metadata dir");

    let inventory = discover_skills(&DiscoveryRoots {
        canonical_root: temp.path().join("canonical"),
        imports_root: temp.path().join("imports"),
        claude_code_root: claude_root,
        codex_root,
    })
    .expect("discover");

    let agent = find_skill(&inventory, "agent-helper");
    assert_eq!(agent.source, SkillSource::AgentOnly);
    assert_eq!(
        agent.analysis_skill_dir.as_deref(),
        Some(fs::canonicalize(claude).expect("claude").as_path())
    );
    let no_skill_file = find_skill(&inventory, "no-skill-file");
    assert_eq!(no_skill_file.analysis_skill_dir, None);
}

#[test]
fn inventory_to_json_serializes_named_enum_values_as_stable_strings() {
    let inventory = SkillInventory {
        skills: vec![
            SkillEntry {
                name: "canonical-helper".to_string(),
                description: None,
                source: SkillSource::Canonical,
                source_repository: None,
                promoted: false,
                enablement: AgentEnablement::Both,
                agent_entries: AgentEntries {
                    claude_code: AgentEntryStatus::SkillDirectory,
                    codex: AgentEntryStatus::CanonicalSymlink,
                },
                analysis_skill_dir: None,
            },
            SkillEntry {
                name: "imported-helper".to_string(),
                description: None,
                source: SkillSource::Imported,
                source_repository: None,
                promoted: true,
                enablement: AgentEnablement::Both,
                agent_entries: AgentEntries {
                    claude_code: AgentEntryStatus::ImportedSymlink,
                    codex: AgentEntryStatus::ExternalSymlink,
                },
                analysis_skill_dir: None,
            },
            SkillEntry {
                name: "agent-only-helper".to_string(),
                description: None,
                source: SkillSource::AgentOnly,
                source_repository: None,
                promoted: false,
                enablement: AgentEnablement::Neither,
                agent_entries: AgentEntries {
                    claude_code: AgentEntryStatus::BrokenSymlink,
                    codex: AgentEntryStatus::Missing,
                },
                analysis_skill_dir: None,
            },
        ],
        source_repositories: Vec::new(),
    };
    let json =
        serde_json::to_value(inventory_to_json(&inventory)).expect("serialize json inventory");

    let skills = json["skills"].as_array().expect("skills array");
    assert_eq!(skills[0]["source"], "canonical");
    assert_eq!(skills[0]["promoted"], false);
    assert_eq!(skills[0]["agent_entries"]["claude_code"], "skill_directory");
    assert_eq!(skills[0]["agent_entries"]["codex"], "canonical_symlink");
    assert_eq!(skills[1]["source"], "imported");
    assert_eq!(skills[1]["promoted"], true);
    assert_eq!(
        skills[1]["agent_entries"]["claude_code"],
        "imported_symlink"
    );
    assert_eq!(skills[1]["agent_entries"]["codex"], "external_symlink");
    assert_eq!(skills[2]["source"], "agent_only");
    assert_eq!(skills[2]["promoted"], false);
    assert_eq!(skills[2]["agent_entries"]["claude_code"], "broken_symlink");
    assert_eq!(skills[2]["agent_entries"]["codex"], "missing");
}

fn write_skill(root: &std::path::Path, name: &str, description: &str) -> std::path::PathBuf {
    write_skill_with_metadata(root, name, name, description)
}

fn write_skill_with_metadata(
    root: &std::path::Path,
    entry_name: &str,
    metadata_name: &str,
    description: &str,
) -> std::path::PathBuf {
    let skill_dir = root.join(entry_name);
    fs::create_dir_all(&skill_dir).expect("skill dir");
    fs::write(
        skill_dir.join("SKILL.md"),
        format!(
            r#"---
name: {metadata_name}
description: {description}
---
"#
        ),
    )
    .expect("skill file");
    skill_dir
}

fn write_import_manifest(skill_dir: &std::path::Path, promoted: bool) {
    fs::write(
        skill_dir.join("import.json"),
        format!(
            r#"{{
  "source_type": "local_path",
  "source_location": "/tmp/source",
  "imported_at": 1,
  "content_hash": "abc123",
  "promoted": {promoted}
}}"#
        ),
    )
    .expect("import manifest");
}

fn write_repository_import_manifest(
    skill_dir: &std::path::Path,
    repository: &str,
    skill_path: &str,
) {
    fs::write(
        skill_dir.join("import.json"),
        serde_json::json!({
            "source_type": "repository",
            "source_location": format!("{repository}#{skill_path}"),
            "source_repository": {
                "repository": repository,
                "skill_path": skill_path,
            },
            "imported_at": 1,
            "content_hash": "abc123",
            "promoted": false,
        })
        .to_string(),
    )
    .expect("import manifest");
}

fn find_skill<'inventory>(
    inventory: &'inventory skill_importer::SkillInventory,
    name: &str,
) -> &'inventory skill_importer::SkillEntry {
    inventory
        .skills
        .iter()
        .find(|skill| skill.name == name)
        .expect("skill exists")
}
