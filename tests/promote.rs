use std::fs;
use std::os::unix::fs::{self as unix_fs, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use skill_importer::{
    DiscoveryRoots, EnableSkillRequest, ImportLocalPathRequest, ImportMarkdownRequest,
    PromoteSkillOptions, PromoteSkillRequest, SkillActionKind, SkillAgent, SkillOperationError,
    UnpromoteSkillRequest, enable_skill, import_local_path_skill, import_markdown_skill,
    promote_imported_skill_with_launcher,
    promotion_pr::{PromotePrLaunchRequest, PromotePrLaunchResult, PromotionPrLauncher},
    unpromote_imported_skill,
};

#[test]
fn promotion_copies_imported_skill_without_import_manifest_and_marks_import_promoted() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    let skills_repo = skills_repo(temp.path());
    import_markdown(&roots, "draft-helper");

    let result = promote(&roots, "draft-helper", &skills_repo, &RecordingLauncher)
        .expect("promote succeeds");

    assert!(
        skills_repo
            .join("third-party")
            .join("draft-helper")
            .join("SKILL.md")
            .exists()
    );
    assert!(
        !skills_repo
            .join("third-party")
            .join("draft-helper")
            .join("import.json")
            .exists(),
        "managed import metadata should not be copied into third-party skills"
    );
    assert!(
        roots
            .imports_root
            .join("draft-helper")
            .join("import.json")
            .exists()
    );
    let manifest = read_manifest(&roots.imports_root.join("draft-helper").join("import.json"));
    assert_eq!(manifest["promoted"], true);
    assert!(manifest["promotion_id"].as_str().is_some());
    assert!(
        roots
            .imports_root
            .join(".skill-importer")
            .join("promotions")
            .join("draft-helper.json")
            .exists()
    );
    assert!(
        !skills_repo
            .join(".skill-importer")
            .join("promotions")
            .exists(),
        "promotion ownership metadata should stay out of the external agent-skills checkout"
    );
    assert!(
        result
            .actions
            .iter()
            .any(|action| action.action == SkillActionKind::WriteManifest)
    );
    assert!(
        result
            .actions
            .iter()
            .any(|action| action.action == SkillActionKind::LaunchPromotionPrWorkflow)
    );
}

#[test]
fn promotion_preserves_supporting_files_from_local_imports() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    let skills_repo = skills_repo(temp.path());
    let source = temp.path().join("source").join("support-helper");
    fs::create_dir_all(source.join("references")).expect("support dir");
    fs::write(source.join("SKILL.md"), skill_markdown("support-helper")).expect("skill file");
    fs::write(source.join("references").join("notes.md"), "# Notes\n").expect("support file");
    import_local_path_skill(
        &roots,
        ImportLocalPathRequest {
            path: source.as_path(),
        },
    )
    .expect("import local path");

    promote(&roots, "support-helper", &skills_repo, &RecordingLauncher).expect("promote succeeds");

    assert_eq!(
        fs::read_to_string(
            skills_repo
                .join("third-party")
                .join("support-helper")
                .join("references")
                .join("notes.md")
        )
        .expect("support file"),
        "# Notes\n"
    );
}

#[test]
fn promotion_relinks_enabled_import_symlinks_to_canonical_skill() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    let skills_repo = skills_repo(temp.path());
    let import = import_markdown(&roots, "enabled-helper");
    enable_skill(
        &roots,
        EnableSkillRequest {
            skill_name: "enabled-helper",
            agents: &[SkillAgent::ClaudeCode, SkillAgent::Codex],
        },
    )
    .expect("enable both");

    let result = promote(&roots, "enabled-helper", &skills_repo, &RecordingLauncher)
        .expect("promote succeeds");

    let promoted = fs::canonicalize(skills_repo.join("third-party").join("enabled-helper"))
        .expect("promoted target");
    assert_eq!(
        fs::canonicalize(roots.claude_code_root.join("enabled-helper")).expect("claude target"),
        promoted
    );
    assert_eq!(
        fs::canonicalize(roots.codex_root.join("enabled-helper")).expect("codex target"),
        promoted
    );
    assert!(
        result
            .actions
            .iter()
            .any(|action| action.action == SkillActionKind::RemoveSymlink
                && action.target == Some(import.skill_path.clone()))
    );
    assert!(
        result
            .actions
            .iter()
            .any(|action| action.action == SkillActionKind::CreateSymlink
                && action.target == Some(promoted.clone()))
    );
}

#[test]
fn unpromotion_removes_third_party_copy_marks_import_draft_and_relinks_enabled_agents() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    let skills_repo = skills_repo(temp.path());
    let import = import_markdown(&roots, "enabled-helper");
    enable_skill(
        &roots,
        EnableSkillRequest {
            skill_name: "enabled-helper",
            agents: &[SkillAgent::ClaudeCode, SkillAgent::Codex],
        },
    )
    .expect("enable both");
    promote(&roots, "enabled-helper", &skills_repo, &RecordingLauncher).expect("promote succeeds");

    let result = unpromote_imported_skill(
        &roots,
        UnpromoteSkillRequest {
            skill_name: "enabled-helper",
        },
    )
    .expect("unpromote succeeds");

    assert!(
        !skills_repo
            .join("third-party")
            .join("enabled-helper")
            .exists()
    );
    let manifest = read_manifest(
        &roots
            .imports_root
            .join("enabled-helper")
            .join("import.json"),
    );
    assert_eq!(manifest["promoted"], false);
    assert_eq!(manifest["promoted_path"], Value::Null);
    assert_eq!(manifest["promoted_repo"], Value::Null);
    assert_eq!(manifest["promotion_id"], Value::Null);
    assert!(
        !roots
            .imports_root
            .join(".skill-importer")
            .join("promotions")
            .join("enabled-helper.json")
            .exists()
    );
    assert_eq!(
        fs::canonicalize(roots.claude_code_root.join("enabled-helper")).expect("claude target"),
        import.skill_path
    );
    assert_eq!(
        fs::canonicalize(roots.codex_root.join("enabled-helper")).expect("codex target"),
        import.skill_path
    );
    assert!(
        result
            .actions
            .iter()
            .any(|action| action.action == SkillActionKind::RemoveDirectory)
    );
    assert!(
        result
            .actions
            .iter()
            .any(|action| action.action == SkillActionKind::WriteManifest)
    );
}

#[test]
fn promotion_refuses_canonical_collision_before_mutating() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    let skills_repo = skills_repo(temp.path());
    let import = import_markdown(&roots, "collision-helper");
    enable_skill(
        &roots,
        EnableSkillRequest {
            skill_name: "collision-helper",
            agents: &[SkillAgent::ClaudeCode],
        },
    )
    .expect("enable");
    write_skill(&skills_repo.join("third-party"), "collision-helper");

    let error = promote(&roots, "collision-helper", &skills_repo, &RecordingLauncher)
        .expect_err("collision fails");

    assert!(matches!(
        error.error,
        SkillOperationError::Collision { name, .. } if name == "collision-helper"
    ));
    let manifest = read_manifest(
        &roots
            .imports_root
            .join("collision-helper")
            .join("import.json"),
    );
    assert_eq!(manifest["promoted"], false);
    assert_eq!(
        fs::canonicalize(roots.claude_code_root.join("collision-helper")).expect("claude target"),
        import.skill_path
    );
}

#[test]
fn promotion_refuses_unsafe_agent_entries_without_mutating() {
    for case in [
        UnsafeEntry::Directory,
        UnsafeEntry::File,
        UnsafeEntry::ExternalSymlink,
        UnsafeEntry::BrokenSymlink,
        UnsafeEntry::WrongManagedSymlink,
    ] {
        let temp = tempfile::tempdir().expect("tempdir");
        let roots = roots(temp.path());
        let skills_repo = skills_repo(temp.path());
        import_markdown(&roots, "unsafe-helper");
        place_unsafe_entry(&roots, "unsafe-helper", case);

        let error = promote(&roots, "unsafe-helper", &skills_repo, &RecordingLauncher)
            .expect_err("unsafe entry fails");

        assert!(matches!(
            error.error,
            SkillOperationError::UnsafeAgentEntry { .. }
        ));
        assert!(
            !skills_repo
                .join("third-party")
                .join("unsafe-helper")
                .exists()
        );
        assert_entry_still_exists(&roots.claude_code_root.join("unsafe-helper"), case);
    }
}

#[test]
fn promotion_reports_unsupported_skill_entries_without_agent_entry_language() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    let skills_repo = skills_repo(temp.path());
    import_markdown(&roots, "unsupported-entry-helper");
    let import_dir = roots.imports_root.join("unsupported-entry-helper");
    let unsupported_entry = import_dir.join("linked-skill.md");
    unix_fs::symlink(import_dir.join("SKILL.md"), &unsupported_entry).expect("support symlink");
    let expected_entry = fs::canonicalize(&import_dir)
        .expect("canonical import dir")
        .join("linked-skill.md");

    let error = promote(
        &roots,
        "unsupported-entry-helper",
        &skills_repo,
        &RecordingLauncher,
    )
    .expect_err("unsupported source entry fails");

    assert!(matches!(
        error.error,
        SkillOperationError::UnsupportedSkillEntry { ref path, .. } if *path == expected_entry
    ));
    assert!(
        !error.error.to_string().contains("agent entry"),
        "source skill entry errors should not mention agent entries"
    );
    assert!(
        !skills_repo
            .join("third-party")
            .join("unsupported-entry-helper")
            .exists(),
        "failed promotion should remove partial third-party destination"
    );
}

#[test]
fn promotion_refuses_missing_skills_repo_before_mutating() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    let missing_repo = temp.path().join("missing-agent-skills");
    import_markdown(&roots, "missing-repo-helper");

    let error = promote(
        &roots,
        "missing-repo-helper",
        &missing_repo,
        &RecordingLauncher,
    )
    .expect_err("missing repo fails");

    assert!(matches!(
        error.error,
        SkillOperationError::InvalidSkillsRepo { ref path, .. } if *path == missing_repo
    ));
    assert!(
        !missing_repo.exists(),
        "promotion must not create a fake agent-skills checkout"
    );
    let manifest = read_manifest(
        &roots
            .imports_root
            .join("missing-repo-helper")
            .join("import.json"),
    );
    assert_eq!(manifest["promoted"], false);
}

#[test]
fn promotion_refuses_git_checkout_without_agent_skills_identity_before_mutating() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    let unrelated_repo = temp.path().join("unrelated-repo");
    fs::create_dir_all(unrelated_repo.join(".git")).expect("git metadata");
    fs::create_dir_all(unrelated_repo.join("third-party")).expect("third-party");
    import_markdown(&roots, "wrong-repo-helper");

    let error = promote(
        &roots,
        "wrong-repo-helper",
        &unrelated_repo,
        &RecordingLauncher,
    )
    .expect_err("wrong repo fails");

    assert!(matches!(
        error.error,
        SkillOperationError::InvalidSkillsRepo { ref path, .. } if *path == unrelated_repo
    ));
    assert!(
        !unrelated_repo
            .join("third-party")
            .join("wrong-repo-helper")
            .exists()
    );
    let manifest = read_manifest(
        &roots
            .imports_root
            .join("wrong-repo-helper")
            .join("import.json"),
    );
    assert_eq!(manifest["promoted"], false);
}

#[test]
fn promotion_refuses_repo_with_agent_skills_files_but_wrong_remote_before_mutating() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    let fake_repo = temp.path().join("fake-agent-skills");
    write_agent_skills_identity_files(&fake_repo);
    init_git_repo(
        &fake_repo,
        "https://github.com/example/not-agent-skills.git",
    );
    import_markdown(&roots, "wrong-remote-helper");

    let error = promote(
        &roots,
        "wrong-remote-helper",
        &fake_repo,
        &RecordingLauncher,
    )
    .expect_err("wrong remote fails");

    assert!(matches!(
        error.error,
        SkillOperationError::InvalidSkillsRepo { ref path, .. } if *path == fake_repo
    ));
    assert!(
        !fake_repo
            .join("third-party")
            .join("wrong-remote-helper")
            .exists()
    );
    let manifest = read_manifest(
        &roots
            .imports_root
            .join("wrong-remote-helper")
            .join("import.json"),
    );
    assert_eq!(manifest["promoted"], false);
}

#[test]
fn promotion_refuses_lookalike_agent_skills_remote_before_mutating() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    let fake_repo = temp.path().join("lookalike-agent-skills");
    write_agent_skills_identity_files(&fake_repo);
    init_git_repo(
        &fake_repo,
        "https://github.com/example/brian-bell/agent-skills-fake.git",
    );
    import_markdown(&roots, "lookalike-remote-helper");

    let error = promote(
        &roots,
        "lookalike-remote-helper",
        &fake_repo,
        &RecordingLauncher,
    )
    .expect_err("lookalike remote fails");

    assert!(matches!(
        error.error,
        SkillOperationError::InvalidSkillsRepo { ref path, .. } if *path == fake_repo
    ));
    assert!(
        !fake_repo
            .join("third-party")
            .join("lookalike-remote-helper")
            .exists()
    );
}

#[test]
fn promotion_refuses_repeated_git_suffix_agent_skills_remote_before_mutating() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    let fake_repo = temp.path().join("repeated-suffix-agent-skills");
    write_agent_skills_identity_files(&fake_repo);
    init_git_repo(
        &fake_repo,
        "https://github.com/brian-bell/agent-skills.git.git",
    );
    import_markdown(&roots, "repeated-suffix-helper");

    let error = promote(
        &roots,
        "repeated-suffix-helper",
        &fake_repo,
        &RecordingLauncher,
    )
    .expect_err("repeated suffix remote fails");

    assert!(matches!(
        error.error,
        SkillOperationError::InvalidSkillsRepo { ref path, .. } if *path == fake_repo
    ));
    assert!(
        !fake_repo
            .join("third-party")
            .join("repeated-suffix-helper")
            .exists()
    );
}

#[test]
fn unpromotion_refuses_manifest_promoted_path_outside_third_party_without_deleting_it() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    import_markdown(&roots, "malicious-helper");
    let protected_dir = temp.path().join("protected");
    fs::create_dir_all(&protected_dir).expect("protected dir");
    fs::write(protected_dir.join("keep.txt"), "keep").expect("protected file");
    write_import_manifest(
        &roots,
        "malicious-helper",
        true,
        Some(protected_dir.clone()),
        None,
        None,
    );

    let error = unpromote_imported_skill(
        &roots,
        UnpromoteSkillRequest {
            skill_name: "malicious-helper",
        },
    )
    .expect_err("malicious path rejected");

    assert!(matches!(
        error.error,
        SkillOperationError::InvalidSkillsRepo { .. }
    ));
    assert_eq!(
        fs::read_to_string(protected_dir.join("keep.txt")).expect("protected file"),
        "keep"
    );
}

#[test]
fn unpromotion_refuses_manifest_promoted_path_in_fake_third_party_repo_without_deleting_it() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    import_markdown(&roots, "fake-repo-helper");
    let fake_repo = temp.path().join("fake-repo");
    write_agent_skills_identity_files(&fake_repo);
    init_git_repo(
        &fake_repo,
        "https://github.com/example/not-agent-skills.git",
    );
    let fake_promoted = write_skill(&fake_repo.join("third-party"), "fake-repo-helper");
    write_import_manifest(
        &roots,
        "fake-repo-helper",
        true,
        Some(fake_promoted.clone()),
        Some(fake_repo.clone()),
        Some("fake-promotion-id"),
    );

    let error = unpromote_imported_skill(
        &roots,
        UnpromoteSkillRequest {
            skill_name: "fake-repo-helper",
        },
    )
    .expect_err("fake repo path rejected");

    assert!(matches!(
        error.error,
        SkillOperationError::InvalidSkillsRepo { .. }
    ));
    assert!(
        fake_promoted.join("SKILL.md").exists(),
        "fake third-party directory must not be deleted"
    );
}

#[test]
fn unpromotion_refuses_real_repo_path_without_matching_ownership_record() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    let skills_repo = skills_repo(temp.path());
    import_markdown(&roots, "unowned-helper");
    let promoted = write_skill(&skills_repo.join("third-party"), "unowned-helper");
    write_import_manifest(
        &roots,
        "unowned-helper",
        true,
        Some(promoted.clone()),
        Some(skills_repo.clone()),
        Some("tampered-promotion-id"),
    );

    let error = unpromote_imported_skill(
        &roots,
        UnpromoteSkillRequest {
            skill_name: "unowned-helper",
        },
    )
    .expect_err("unowned path rejected");

    assert!(matches!(
        error.error,
        SkillOperationError::InvalidSkillsRepo { .. }
    ));
    assert!(
        promoted.join("SKILL.md").exists(),
        "unowned third-party directory must not be deleted"
    );
}

#[test]
fn unpromotion_rolls_back_directory_and_symlinks_when_manifest_write_fails() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    let skills_repo = skills_repo(temp.path());
    let import = import_markdown(&roots, "rollback-helper");
    enable_skill(
        &roots,
        EnableSkillRequest {
            skill_name: "rollback-helper",
            agents: &[SkillAgent::ClaudeCode, SkillAgent::Codex],
        },
    )
    .expect("enable both");
    promote(&roots, "rollback-helper", &skills_repo, &RecordingLauncher).expect("promote succeeds");
    let promoted = fs::canonicalize(skills_repo.join("third-party").join("rollback-helper"))
        .expect("promoted path");
    let manifest_path = roots
        .imports_root
        .join("rollback-helper")
        .join("import.json");
    let original_permissions = fs::metadata(&manifest_path)
        .expect("manifest metadata")
        .permissions();
    let mut readonly = original_permissions.clone();
    readonly.set_mode(0o400);
    fs::set_permissions(&manifest_path, readonly).expect("readonly manifest");

    let error = unpromote_imported_skill(
        &roots,
        UnpromoteSkillRequest {
            skill_name: "rollback-helper",
        },
    )
    .expect_err("manifest write fails");

    fs::set_permissions(&manifest_path, original_permissions).expect("restore permissions");
    assert!(matches!(error.error, SkillOperationError::Io(_)));
    assert!(promoted.join("SKILL.md").exists());
    assert_eq!(
        fs::canonicalize(roots.claude_code_root.join("rollback-helper")).expect("claude target"),
        promoted
    );
    assert_eq!(
        fs::canonicalize(roots.codex_root.join("rollback-helper")).expect("codex target"),
        promoted
    );
    let manifest = read_manifest(&manifest_path);
    assert_eq!(manifest["promoted"], true);
    assert_eq!(
        fs::canonicalize(import.skill_path).expect("import still exists"),
        fs::canonicalize(roots.imports_root.join("rollback-helper")).expect("import dir")
    );
}

#[test]
fn promotion_reports_unknown_unsupported_and_already_promoted_sources() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    let skills_repo = skills_repo(temp.path());

    let unknown =
        promote(&roots, "missing-helper", &skills_repo, &RecordingLauncher).expect_err("unknown");
    assert!(matches!(
        unknown.error,
        SkillOperationError::UnknownSkill { name } if name == "missing-helper"
    ));

    write_skill(&roots.canonical_root, "canonical-helper");
    let canonical = promote(&roots, "canonical-helper", &skills_repo, &RecordingLauncher)
        .expect_err("canonical unsupported");
    assert!(matches!(
        canonical.error,
        SkillOperationError::UnsupportedSkillSource { name } if name == "canonical-helper"
    ));

    let agent_only = write_skill(&temp.path().join("external"), "agent-helper");
    fs::create_dir_all(&roots.claude_code_root).expect("claude root");
    unix_fs::symlink(agent_only, roots.claude_code_root.join("agent-helper"))
        .expect("agent symlink");
    let agent = promote(&roots, "agent-helper", &skills_repo, &RecordingLauncher)
        .expect_err("agent unsupported");
    assert!(matches!(
        agent.error,
        SkillOperationError::UnsupportedSkillSource { name } if name == "agent-helper"
    ));

    import_markdown(&roots, "promoted-helper");
    promote(&roots, "promoted-helper", &skills_repo, &RecordingLauncher).expect("first promote");
    let already = promote(&roots, "promoted-helper", &skills_repo, &RecordingLauncher)
        .expect_err("already promoted");
    assert!(matches!(
        already.error,
        SkillOperationError::AlreadyPromoted { name } if name == "promoted-helper"
    ));
}

#[test]
fn promotion_leaves_repo_documentation_and_installer_files_untouched() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    let skills_repo = skills_repo(temp.path());
    for file in ["CLAUDE.md", "AGENTS.md", "README.md", "install.sh"] {
        fs::write(temp.path().join(file), format!("{file} sentinel\n")).expect("sentinel");
    }
    import_markdown(&roots, "scoped-helper");

    promote(&roots, "scoped-helper", &skills_repo, &RecordingLauncher).expect("promote");

    for file in ["CLAUDE.md", "AGENTS.md", "README.md", "install.sh"] {
        assert_eq!(
            fs::read_to_string(temp.path().join(file)).expect("sentinel"),
            format!("{file} sentinel\n")
        );
    }
}

#[test]
fn promote_command_emits_action_json_and_reports_collisions() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    let skills_repo = skills_repo(temp.path());
    import_markdown(&roots, "command-promote");

    let output = skill_importer_command()
        .args(["promote", "--json", "--skill", "command-promote"])
        .arg("--skills-repo")
        .arg(&skills_repo)
        .args(root_args(&roots))
        .env("SKILL_IMPORTER_PROMOTION_PR_DRY_RUN", "1")
        .output()
        .expect("run promote");
    assert!(
        output.status.success(),
        "promote failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("promote json");
    assert_eq!(json["skill_name"], "command-promote");
    assert!(
        json["actions"]
            .as_array()
            .expect("actions")
            .iter()
            .any(|action| action["action"] == "copy_file")
    );

    import_markdown(&roots, "command-collision");
    write_skill(&skills_repo.join("third-party"), "command-collision");
    let output = skill_importer_command()
        .args(["promote", "--json", "--skill", "command-collision"])
        .arg("--skills-repo")
        .arg(&skills_repo)
        .args(root_args(&roots))
        .env("SKILL_IMPORTER_PROMOTION_PR_DRY_RUN", "1")
        .output()
        .expect("run failing promote");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to promote skill"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("already exists"), "stderr: {stderr}");
}

#[test]
fn promote_command_canonicalizes_relative_skills_repo_for_launcher() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    let skills_repo = skills_repo(temp.path());
    import_markdown(&roots, "relative-repo-helper");

    let output = skill_importer_command()
        .current_dir(temp.path())
        .args(["promote", "--json", "--skill", "relative-repo-helper"])
        .arg("--skills-repo")
        .arg("agent-skills")
        .args(root_args(&roots))
        .env("SKILL_IMPORTER_PROMOTION_PR_DRY_RUN", "1")
        .output()
        .expect("run promote");
    assert!(
        output.status.success(),
        "promote failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("promote json");
    let canonical_repo = fs::canonicalize(skills_repo).expect("canonical repo");
    let launch_action = json["actions"]
        .as_array()
        .expect("actions")
        .iter()
        .find(|action| action["action"] == "launch_promotion_pr_workflow")
        .expect("launch action");
    assert!(
        launch_action["path"]
            .as_str()
            .expect("script path")
            .starts_with(&canonical_repo.to_string_lossy().to_string()),
        "launch action should use canonical repo path: {launch_action}"
    );
}

#[test]
fn promotion_rolls_back_copy_and_manifest_when_launcher_fails() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    let skills_repo = skills_repo(temp.path());
    import_markdown(&roots, "launch-failure-helper");

    let error = promote(
        &roots,
        "launch-failure-helper",
        &skills_repo,
        &FailingLauncher,
    )
    .expect_err("launcher failure");

    assert!(matches!(
        error.error,
        SkillOperationError::PromotionPrLaunch { .. }
    ));
    assert!(
        !skills_repo
            .join("third-party")
            .join("launch-failure-helper")
            .exists()
    );
    let manifest = read_manifest(
        &roots
            .imports_root
            .join("launch-failure-helper")
            .join("import.json"),
    );
    assert_eq!(manifest["promoted"], false);
}

#[derive(Debug, Clone, Copy)]
enum UnsafeEntry {
    Directory,
    File,
    ExternalSymlink,
    BrokenSymlink,
    WrongManagedSymlink,
}

fn roots(base: &Path) -> DiscoveryRoots {
    DiscoveryRoots {
        canonical_root: base.join("canonical"),
        imports_root: base.join("imports"),
        claude_code_root: base.join("claude"),
        codex_root: base.join("codex"),
    }
}

fn skills_repo(base: &Path) -> PathBuf {
    let repo = base.join("agent-skills");
    write_agent_skills_identity_files(&repo);
    init_git_repo(&repo, "https://github.com/brian-bell/agent-skills.git");
    repo
}

fn write_agent_skills_identity_files(repo: &Path) {
    fs::create_dir_all(repo.join("scripts")).expect("scripts dir");
    fs::create_dir_all(repo.join("third-party")).expect("third-party root");
    fs::write(
        repo.join("scripts").join("install-skills.sh"),
        "#!/bin/sh\n",
    )
    .expect("installer");
    fs::write(
        repo.join("third-party").join("ATTRIBUTION.md"),
        "# Attribution\n",
    )
    .expect("attribution");
}

fn init_git_repo(repo: &Path, origin: &str) {
    let init = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["init", "--quiet"])
        .output()
        .expect("git init");
    assert!(
        init.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    let remote = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["remote", "add", "origin", origin])
        .output()
        .expect("git remote add");
    assert!(
        remote.status.success(),
        "git remote add failed: {}",
        String::from_utf8_lossy(&remote.stderr)
    );
}

fn promote(
    roots: &DiscoveryRoots,
    name: &str,
    skills_repo: &Path,
    launcher: &impl PromotionPrLauncher,
) -> Result<skill_importer::SkillOperationResult, skill_importer::SkillOperationFailure> {
    promote_imported_skill_with_launcher(
        roots,
        PromoteSkillRequest { skill_name: name },
        PromoteSkillOptions { skills_repo },
        launcher,
    )
}

#[derive(Default)]
struct RecordingLauncher;

impl PromotionPrLauncher for RecordingLauncher {
    fn launch(&self, request: PromotePrLaunchRequest) -> Result<PromotePrLaunchResult, String> {
        Ok(PromotePrLaunchResult {
            prompt_path: request
                .skills_repo
                .join("promotion")
                .join(format!("{}-prompt.txt", request.skill_name)),
            script_path: request
                .skills_repo
                .join("promotion")
                .join(format!("{}-run.sh", request.skill_name)),
        })
    }
}

struct FailingLauncher;

impl PromotionPrLauncher for FailingLauncher {
    fn launch(&self, _request: PromotePrLaunchRequest) -> Result<PromotePrLaunchResult, String> {
        Err("terminal launch failed".to_string())
    }
}

fn import_markdown(roots: &DiscoveryRoots, name: &str) -> skill_importer::ImportResult {
    import_markdown_skill(
        roots,
        ImportMarkdownRequest {
            markdown: &skill_markdown(name),
            source_location: None,
        },
    )
    .expect("import markdown")
}

fn skill_markdown(name: &str) -> String {
    format!(
        r#"---
name: {name}
description: Promotion test skill.
---

# Promotion Test
"#
    )
}

fn write_skill(root: &Path, name: &str) -> PathBuf {
    let skill_dir = root.join(name);
    fs::create_dir_all(&skill_dir).expect("skill dir");
    fs::write(skill_dir.join("SKILL.md"), skill_markdown(name)).expect("skill file");
    fs::canonicalize(skill_dir).expect("canonical skill dir")
}

fn read_manifest(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).expect("manifest")).expect("manifest json")
}

fn write_import_manifest(
    roots: &DiscoveryRoots,
    name: &str,
    promoted: bool,
    promoted_path: Option<PathBuf>,
    promoted_repo: Option<PathBuf>,
    promotion_id: Option<&str>,
) {
    let path = roots.imports_root.join(name).join("import.json");
    let mut manifest = read_manifest(&path);
    manifest["promoted"] = Value::Bool(promoted);
    manifest["promoted_path"] = promoted_path
        .map(|path| Value::String(path.to_string_lossy().into_owned()))
        .unwrap_or(Value::Null);
    manifest["promoted_repo"] = promoted_repo
        .map(|path| Value::String(path.to_string_lossy().into_owned()))
        .unwrap_or(Value::Null);
    manifest["promotion_id"] = promotion_id
        .map(|promotion_id| Value::String(promotion_id.to_string()))
        .unwrap_or(Value::Null);
    fs::write(
        path,
        serde_json::to_vec_pretty(&manifest).expect("manifest json"),
    )
    .expect("write manifest");
}

fn place_unsafe_entry(roots: &DiscoveryRoots, name: &str, case: UnsafeEntry) {
    fs::create_dir_all(&roots.claude_code_root).expect("claude root");
    let path = roots.claude_code_root.join(name);
    match case {
        UnsafeEntry::Directory => {
            write_skill(&roots.claude_code_root, name);
        }
        UnsafeEntry::File => {
            fs::write(path, "mine").expect("regular file");
        }
        UnsafeEntry::ExternalSymlink => {
            let external = write_skill(&roots.canonical_root.join("external-root"), name);
            unix_fs::symlink(external, path).expect("external symlink");
        }
        UnsafeEntry::BrokenSymlink => {
            unix_fs::symlink(roots.claude_code_root.join("missing"), path).expect("broken symlink");
        }
        UnsafeEntry::WrongManagedSymlink => {
            let other = write_skill(&roots.imports_root, "other-helper");
            unix_fs::symlink(other, path).expect("wrong managed symlink");
        }
    }
}

fn assert_entry_still_exists(path: &Path, case: UnsafeEntry) {
    let metadata = fs::symlink_metadata(path).expect("entry still exists");
    match case {
        UnsafeEntry::Directory => assert!(metadata.is_dir()),
        UnsafeEntry::File => assert!(metadata.is_file()),
        UnsafeEntry::ExternalSymlink
        | UnsafeEntry::BrokenSymlink
        | UnsafeEntry::WrongManagedSymlink => assert!(metadata.file_type().is_symlink()),
    }
}

fn skill_importer_command() -> Command {
    Command::new(std::env::var("CARGO_BIN_EXE_skill-importer").expect("binary path"))
}

fn root_args(roots: &DiscoveryRoots) -> Vec<String> {
    vec![
        "--canonical-root".to_string(),
        roots.canonical_root.to_string_lossy().into_owned(),
        "--imports-root".to_string(),
        roots.imports_root.to_string_lossy().into_owned(),
        "--claude-code-root".to_string(),
        roots.claude_code_root.to_string_lossy().into_owned(),
        "--codex-root".to_string(),
        roots.codex_root.to_string_lossy().into_owned(),
    ]
}
