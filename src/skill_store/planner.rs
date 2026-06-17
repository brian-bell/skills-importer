use crate::{
    AgentMutationState, DiscoveryRoots, SkillAgent, SkillOperationError, SkillOperationFailure,
    SkillSource, canonicalize_existing_ancestor, discover_skills, empty_operation_failure,
    exact_managed_symlink_state,
};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EnablePlan {
    pub(super) skill_name: String,
    pub(super) source_path: PathBuf,
    pub(super) entries: Vec<EnableEntryPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DisablePlan {
    pub(super) skill_name: String,
    pub(super) source_path: PathBuf,
    pub(super) entries: Vec<DisableEntryPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EnableEntryPlan {
    pub(super) agent: SkillAgent,
    pub(super) root: PathBuf,
    pub(super) path: PathBuf,
    pub(super) work: EnableEntryWork,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DisableEntryPlan {
    pub(super) agent: SkillAgent,
    pub(super) path: PathBuf,
    pub(super) work: DisableEntryWork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EnableEntryWork {
    CreateLink,
    SkipUnchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DisableEntryWork {
    RemoveLink,
    SkipUnchanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OwnedSkillSource {
    name: String,
    path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DraftImportPolicy {
    Allow,
    Reject,
}

pub(super) fn plan_enable(
    roots: &DiscoveryRoots,
    skill_name: &str,
    agents: &[SkillAgent],
) -> Result<EnablePlan, SkillOperationFailure> {
    let source = resolve_owned_skill_source(roots, skill_name, DraftImportPolicy::Reject)?;
    let mut entries = Vec::new();

    for agent in unique_agents(agents) {
        let root = agent_root(roots, agent);
        let path = root.join(&source.name);
        let state =
            exact_managed_symlink_state(&path, &source.path).map_err(empty_operation_failure)?;
        let work = match state {
            AgentMutationState::Missing => EnableEntryWork::CreateLink,
            AgentMutationState::AlreadyCorrect => EnableEntryWork::SkipUnchanged,
        };
        entries.push(EnableEntryPlan {
            agent,
            root,
            path,
            work,
        });
    }

    Ok(EnablePlan {
        skill_name: source.name,
        source_path: source.path,
        entries,
    })
}

pub(super) fn plan_disable(
    roots: &DiscoveryRoots,
    skill_name: &str,
    agents: &[SkillAgent],
) -> Result<DisablePlan, SkillOperationFailure> {
    let source = resolve_owned_skill_source(roots, skill_name, DraftImportPolicy::Allow)?;
    let mut entries = Vec::new();

    for agent in unique_agents(agents) {
        let path = agent_root(roots, agent).join(&source.name);
        let state =
            exact_managed_symlink_state(&path, &source.path).map_err(empty_operation_failure)?;
        let work = match state {
            AgentMutationState::Missing => DisableEntryWork::SkipUnchanged,
            AgentMutationState::AlreadyCorrect => DisableEntryWork::RemoveLink,
        };
        entries.push(DisableEntryPlan { agent, path, work });
    }

    Ok(DisablePlan {
        skill_name: source.name,
        source_path: source.path,
        entries,
    })
}

fn resolve_owned_skill_source(
    roots: &DiscoveryRoots,
    skill_name: &str,
    draft_import_policy: DraftImportPolicy,
) -> Result<OwnedSkillSource, SkillOperationFailure> {
    let inventory = discover_skills(roots)
        .map_err(SkillOperationError::Io)
        .map_err(empty_operation_failure)?;
    let Some(skill) = inventory
        .skills
        .iter()
        .find(|skill| skill.name == skill_name)
    else {
        return Err(empty_operation_failure(SkillOperationError::UnknownSkill {
            name: skill_name.to_string(),
        }));
    };

    let source_path = match skill.source {
        SkillSource::Canonical => roots.canonical_root.join(skill_name),
        SkillSource::Imported if skill.promoted => roots.canonical_root.join(skill_name),
        SkillSource::Imported if draft_import_policy == DraftImportPolicy::Allow => {
            canonicalize_existing_ancestor(&roots.imports_root)
                .map_err(SkillOperationError::Io)
                .map_err(empty_operation_failure)?
                .join(skill_name)
        }
        SkillSource::Imported => {
            return Err(empty_operation_failure(SkillOperationError::NotPromoted {
                name: skill_name.to_string(),
            }));
        }
        SkillSource::AgentOnly => {
            return Err(empty_operation_failure(
                SkillOperationError::UnsupportedSkillSource {
                    name: skill_name.to_string(),
                },
            ));
        }
    };

    let source_path = fs::canonicalize(&source_path)
        .map_err(SkillOperationError::Io)
        .map_err(empty_operation_failure)?;

    Ok(OwnedSkillSource {
        name: skill.name.clone(),
        path: source_path,
    })
}

fn unique_agents(agents: &[SkillAgent]) -> Vec<SkillAgent> {
    let mut unique = Vec::new();
    for agent in agents {
        if !unique.contains(agent) {
            unique.push(*agent);
        }
    }
    unique
}

fn agent_root(roots: &DiscoveryRoots, agent: SkillAgent) -> PathBuf {
    match agent {
        SkillAgent::ClaudeCode => roots.claude_code_root.clone(),
        SkillAgent::Codex => roots.codex_root.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ImportMarkdownRequest, PromoteSkillRequest, import_markdown_skill, promote_imported_skill,
    };
    use std::fs;
    use std::os::unix::fs as unix_fs;
    use std::path::Path;

    #[test]
    fn canonical_skill_source_plans_enable_link_to_canonical_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let roots = roots(temp.path());
        let canonical_skill = write_skill(&roots.canonical_root, "helper");

        let plan = plan_enable(&roots, "helper", &[SkillAgent::Codex]).expect("plan enable");

        assert_eq!(plan.skill_name, "helper");
        assert_eq!(plan.source_path, canonical_skill);
        assert_eq!(plan.entries.len(), 1);
        assert_eq!(plan.entries[0].agent, SkillAgent::Codex);
        assert_eq!(plan.entries[0].root, roots.codex_root);
        assert_eq!(plan.entries[0].path, roots.codex_root.join("helper"));
        assert_eq!(plan.entries[0].work, EnableEntryWork::CreateLink);
    }

    #[test]
    fn promoted_import_source_plans_enable_link_to_third_party_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let roots = roots(temp.path());
        import_skill(&roots, "helper");
        promote_imported_skill(
            &roots,
            PromoteSkillRequest {
                skill_name: "helper",
                overwrite: false,
            },
        )
        .expect("promote");
        let third_party =
            fs::canonicalize(roots.canonical_root.join("helper")).expect("third-party skill");

        let plan = plan_enable(&roots, "helper", &[SkillAgent::ClaudeCode]).expect("plan enable");

        assert_eq!(plan.source_path, third_party);
        assert_eq!(plan.entries[0].path, roots.claude_code_root.join("helper"));
        assert_eq!(plan.entries[0].work, EnableEntryWork::CreateLink);
    }

    #[test]
    fn requested_agents_are_deduplicated_in_first_seen_order() {
        let temp = tempfile::tempdir().expect("tempdir");
        let roots = roots(temp.path());
        write_skill(&roots.canonical_root, "helper");

        let plan = plan_enable(
            &roots,
            "helper",
            &[SkillAgent::Codex, SkillAgent::ClaudeCode, SkillAgent::Codex],
        )
        .expect("plan enable");

        let agents = plan
            .entries
            .iter()
            .map(|entry| entry.agent)
            .collect::<Vec<_>>();
        assert_eq!(agents, vec![SkillAgent::Codex, SkillAgent::ClaudeCode]);
    }

    #[test]
    fn enable_planning_maps_missing_and_correct_entries_to_work() {
        let temp = tempfile::tempdir().expect("tempdir");
        let roots = roots(temp.path());
        let source = write_skill(&roots.canonical_root, "helper");
        fs::create_dir_all(&roots.codex_root).expect("codex root");
        unix_fs::symlink(&source, roots.codex_root.join("helper")).expect("codex link");

        let plan = plan_enable(
            &roots,
            "helper",
            &[SkillAgent::ClaudeCode, SkillAgent::Codex],
        )
        .expect("plan enable");

        assert_eq!(plan.entries[0].work, EnableEntryWork::CreateLink);
        assert_eq!(plan.entries[1].work, EnableEntryWork::SkipUnchanged);
    }

    #[test]
    fn disable_planning_maps_correct_and_missing_entries_to_work() {
        let temp = tempfile::tempdir().expect("tempdir");
        let roots = roots(temp.path());
        let source = write_skill(&roots.canonical_root, "helper");
        fs::create_dir_all(&roots.claude_code_root).expect("claude root");
        unix_fs::symlink(&source, roots.claude_code_root.join("helper")).expect("claude link");

        let plan = plan_disable(
            &roots,
            "helper",
            &[SkillAgent::ClaudeCode, SkillAgent::Codex],
        )
        .expect("plan disable");

        assert_eq!(plan.entries[0].work, DisableEntryWork::RemoveLink);
        assert_eq!(plan.entries[1].work, DisableEntryWork::SkipUnchanged);
    }

    #[test]
    fn unsafe_agent_entries_are_rejected_before_returning_a_plan() {
        for case in [
            UnsafeEntry::Directory,
            UnsafeEntry::File,
            UnsafeEntry::ExternalSymlink,
            UnsafeEntry::BrokenSymlink,
            UnsafeEntry::WrongManagedSymlink,
        ] {
            let temp = tempfile::tempdir().expect("case tempdir");
            let roots = roots(temp.path());
            write_skill(&roots.canonical_root, "helper");
            place_unsafe_entry(&roots, "helper", case);

            let failure = plan_enable(&roots, "helper", &[SkillAgent::ClaudeCode])
                .expect_err("unsafe entry fails planning");

            assert!(matches!(
                failure.error,
                SkillOperationError::UnsafeAgentEntry { .. }
            ));
            assert!(failure.actions.is_empty());
        }
    }

    #[test]
    fn unknown_and_agent_only_skills_return_operation_errors() {
        let temp = tempfile::tempdir().expect("tempdir");
        let roots = roots(temp.path());

        let unknown =
            plan_enable(&roots, "missing", &[SkillAgent::Codex]).expect_err("unknown skill");
        assert!(matches!(
            unknown.error,
            SkillOperationError::UnknownSkill { name } if name == "missing"
        ));

        let external = write_skill(&temp.path().join("external"), "agent-only");
        fs::create_dir_all(&roots.claude_code_root).expect("claude root");
        unix_fs::symlink(&external, roots.claude_code_root.join("agent-only"))
            .expect("agent-only link");

        let unsupported = plan_disable(&roots, "agent-only", &[SkillAgent::ClaudeCode])
            .expect_err("agent-only unsupported");
        assert!(matches!(
            unsupported.error,
            SkillOperationError::UnsupportedSkillSource { name } if name == "agent-only"
        ));
    }

    #[test]
    fn malformed_import_manifest_error_is_preserved_by_discovery() {
        let temp = tempfile::tempdir().expect("tempdir");
        let roots = roots(temp.path());
        let skill_dir = roots.imports_root.join("helper");
        fs::create_dir_all(&skill_dir).expect("skill dir");
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: helper\ndescription: bad manifest\n---\n",
        )
        .expect("skill file");
        fs::write(skill_dir.join("import.json"), "{not json").expect("manifest");

        let failure =
            plan_enable(&roots, "helper", &[SkillAgent::Codex]).expect_err("manifest failure");

        assert!(matches!(failure.error, SkillOperationError::Io(_)));
        assert!(failure.actions.is_empty());
    }

    fn roots(base: &Path) -> DiscoveryRoots {
        DiscoveryRoots {
            canonical_root: base.join("canonical"),
            imports_root: base.join("imports"),
            claude_code_root: base.join("claude"),
            codex_root: base.join("codex"),
        }
    }

    fn write_skill(root: &Path, name: &str) -> PathBuf {
        let skill_dir = root.join(name);
        fs::create_dir_all(&skill_dir).expect("skill dir");
        fs::write(
            skill_dir.join("SKILL.md"),
            format!(
                r#"---
name: {name}
description: Test skill.
---
"#
            ),
        )
        .expect("skill file");
        fs::canonicalize(skill_dir).expect("canonical skill dir")
    }

    fn import_skill(roots: &DiscoveryRoots, name: &str) -> crate::ImportResult {
        import_markdown_skill(
            roots,
            ImportMarkdownRequest {
                markdown: &format!(
                    r#"---
name: {name}
description: Imported skill.
---

# Imported
"#
                ),
                source_location: None,
            },
        )
        .expect("import skill")
    }

    #[derive(Debug, Clone, Copy)]
    enum UnsafeEntry {
        Directory,
        File,
        ExternalSymlink,
        BrokenSymlink,
        WrongManagedSymlink,
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
                unix_fs::symlink(roots.claude_code_root.join("missing"), path)
                    .expect("broken symlink");
            }
            UnsafeEntry::WrongManagedSymlink => {
                let other = write_skill(&roots.imports_root, "other-helper");
                unix_fs::symlink(other, path).expect("wrong managed symlink");
            }
        }
    }
}
