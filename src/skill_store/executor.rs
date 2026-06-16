use super::planner::{DisableEntryWork, DisablePlan, EnableEntryWork, EnablePlan};
use crate::{
    SkillAction, SkillActionKind, SkillOperationError, SkillOperationFailure, SkillOperationResult,
    create_symlink, operation_failure,
};
use std::fs;
use std::path::Path;

pub(super) fn execute_enable(
    plan: EnablePlan,
) -> Result<SkillOperationResult, SkillOperationFailure> {
    let mut actions = Vec::new();

    for entry in plan.entries {
        match entry.work {
            EnableEntryWork::CreateLink => {
                create_agent_root_if_missing(entry.agent, &entry.root, &mut actions)?;
                create_symlink(&plan.source_path, &entry.path)
                    .map_err(SkillOperationError::Io)
                    .map_err(|error| operation_failure(error, actions.clone()))?;
                actions.push(SkillAction {
                    action: SkillActionKind::CreateSymlink,
                    agent: Some(entry.agent),
                    path: entry.path,
                    target: Some(plan.source_path.clone()),
                    source: None,
                });
            }
            EnableEntryWork::SkipUnchanged => {
                actions.push(SkillAction {
                    action: SkillActionKind::SkipUnchanged,
                    agent: Some(entry.agent),
                    path: entry.path,
                    target: Some(plan.source_path.clone()),
                    source: None,
                });
            }
        }
    }

    Ok(SkillOperationResult {
        skill_name: plan.skill_name,
        actions,
    })
}

pub(super) fn execute_disable(
    plan: DisablePlan,
) -> Result<SkillOperationResult, SkillOperationFailure> {
    let mut actions = Vec::new();

    for entry in plan.entries {
        match entry.work {
            DisableEntryWork::RemoveLink => {
                fs::remove_file(&entry.path)
                    .map_err(SkillOperationError::Io)
                    .map_err(|error| operation_failure(error, actions.clone()))?;
                actions.push(SkillAction {
                    action: SkillActionKind::RemoveSymlink,
                    agent: Some(entry.agent),
                    path: entry.path,
                    target: Some(plan.source_path.clone()),
                    source: None,
                });
            }
            DisableEntryWork::SkipUnchanged => {
                actions.push(SkillAction {
                    action: SkillActionKind::SkipUnchanged,
                    agent: Some(entry.agent),
                    path: entry.path,
                    target: Some(plan.source_path.clone()),
                    source: None,
                });
            }
        }
    }

    Ok(SkillOperationResult {
        skill_name: plan.skill_name,
        actions,
    })
}

fn create_agent_root_if_missing(
    agent: crate::SkillAgent,
    root: &Path,
    actions: &mut Vec<SkillAction>,
) -> Result<(), SkillOperationFailure> {
    if root.exists() {
        return Ok(());
    }

    fs::create_dir_all(root)
        .map_err(SkillOperationError::Io)
        .map_err(|error| operation_failure(error, actions.clone()))?;
    actions.push(SkillAction {
        action: SkillActionKind::CreateDirectory,
        agent: Some(agent),
        path: root.to_path_buf(),
        target: None,
        source: None,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SkillAgent;
    use std::os::unix::fs as unix_fs;
    use std::path::PathBuf;

    #[test]
    fn enable_execution_creates_root_before_symlink() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = write_source(temp.path(), "source");
        let root = temp.path().join("agent-root");
        let link = root.join("helper");
        let plan = enable_plan(
            "helper",
            source.clone(),
            vec![enable_entry(
                SkillAgent::Codex,
                root.clone(),
                link.clone(),
                EnableEntryWork::CreateLink,
            )],
        );

        let result = execute_enable(plan).expect("execute enable");

        assert!(root.is_dir());
        assert_eq!(fs::canonicalize(&link).expect("link target"), source);
        assert_eq!(result.actions[0].action, SkillActionKind::CreateDirectory);
        assert_eq!(result.actions[0].path, root);
        assert_eq!(result.actions[1].action, SkillActionKind::CreateSymlink);
        assert_eq!(result.actions[1].path, link);
    }

    #[test]
    fn enable_execution_records_skip_without_mutating_correct_entry() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = write_source(temp.path(), "source");
        let root = temp.path().join("agent-root");
        fs::create_dir_all(&root).expect("root");
        let link = root.join("helper");
        unix_fs::symlink(&source, &link).expect("existing link");
        let before = fs::symlink_metadata(&link).expect("before metadata");
        let plan = enable_plan(
            "helper",
            source.clone(),
            vec![enable_entry(
                SkillAgent::ClaudeCode,
                root,
                link.clone(),
                EnableEntryWork::SkipUnchanged,
            )],
        );

        let result = execute_enable(plan).expect("execute enable");

        let after = fs::symlink_metadata(&link).expect("after metadata");
        assert_eq!(
            before.file_type().is_symlink(),
            after.file_type().is_symlink()
        );
        assert_eq!(result.actions[0].action, SkillActionKind::SkipUnchanged);
        assert_eq!(result.actions[0].target, Some(source));
    }

    #[test]
    fn disable_execution_removes_planned_symlink() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = write_source(temp.path(), "source");
        let root = temp.path().join("agent-root");
        fs::create_dir_all(&root).expect("root");
        let link = root.join("helper");
        unix_fs::symlink(&source, &link).expect("existing link");
        let plan = disable_plan(
            "helper",
            source.clone(),
            vec![disable_entry(
                SkillAgent::Codex,
                link.clone(),
                DisableEntryWork::RemoveLink,
            )],
        );

        let result = execute_disable(plan).expect("execute disable");

        assert!(fs::symlink_metadata(&link).is_err());
        assert_eq!(result.actions[0].action, SkillActionKind::RemoveSymlink);
        assert_eq!(result.actions[0].target, Some(source));
    }

    #[test]
    fn disable_execution_records_skip_for_missing_entry() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = write_source(temp.path(), "source");
        let link = temp.path().join("missing").join("helper");
        let plan = disable_plan(
            "helper",
            source.clone(),
            vec![disable_entry(
                SkillAgent::ClaudeCode,
                link.clone(),
                DisableEntryWork::SkipUnchanged,
            )],
        );

        let result = execute_disable(plan).expect("execute disable");

        assert_eq!(result.actions[0].action, SkillActionKind::SkipUnchanged);
        assert_eq!(result.actions[0].path, link);
        assert_eq!(result.actions[0].target, Some(source));
    }

    #[test]
    fn partial_enable_failure_preserves_recorded_actions() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = write_source(temp.path(), "source");
        let root = temp.path().join("agent-root");
        let plan = enable_plan(
            "helper",
            source,
            vec![enable_entry(
                SkillAgent::Codex,
                root.clone(),
                root.clone(),
                EnableEntryWork::CreateLink,
            )],
        );

        let failure = execute_enable(plan).expect_err("create symlink should fail");

        assert!(matches!(failure.error, SkillOperationError::Io(_)));
        assert_eq!(failure.actions.len(), 1);
        assert_eq!(failure.actions[0].action, SkillActionKind::CreateDirectory);
        assert_eq!(failure.actions[0].path, root);
    }

    fn enable_plan(
        skill_name: &str,
        source_path: PathBuf,
        entries: Vec<super::super::planner::EnableEntryPlan>,
    ) -> EnablePlan {
        EnablePlan {
            skill_name: skill_name.to_string(),
            source_path,
            entries,
        }
    }

    fn disable_plan(
        skill_name: &str,
        source_path: PathBuf,
        entries: Vec<super::super::planner::DisableEntryPlan>,
    ) -> DisablePlan {
        DisablePlan {
            skill_name: skill_name.to_string(),
            source_path,
            entries,
        }
    }

    fn enable_entry(
        agent: SkillAgent,
        root: PathBuf,
        path: PathBuf,
        work: EnableEntryWork,
    ) -> super::super::planner::EnableEntryPlan {
        super::super::planner::EnableEntryPlan {
            agent,
            root,
            path,
            work,
        }
    }

    fn disable_entry(
        agent: SkillAgent,
        path: PathBuf,
        work: DisableEntryWork,
    ) -> super::super::planner::DisableEntryPlan {
        super::super::planner::DisableEntryPlan { agent, path, work }
    }

    fn write_source(base: &Path, name: &str) -> PathBuf {
        let path = base.join(name);
        fs::create_dir_all(&path).expect("source dir");
        fs::canonicalize(path).expect("canonical source")
    }
}
