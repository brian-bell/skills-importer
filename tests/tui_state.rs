use skill_importer::{
    AgentEnablement, AgentEntries, AgentEntryStatus, RepositorySkillCandidate,
    RepositorySkillSelection, SkillAgent, SkillEntry, SkillInventory, SkillSource,
    tui::{
        AppAction, AppImportSource, AppInteractionMode, AppOperationRequest, AppOperationResult,
        AppState, ConfirmationOperation, SelectionDelta, SourceFilter,
    },
};

#[test]
fn app_state_initializes_from_inventory() {
    let state = AppState::new(inventory([
        skill("zeta", "Last by name", SkillSource::Canonical),
        skill("alpha", "First by name", SkillSource::Imported),
        skill("beta", "Agent only", SkillSource::AgentOnly),
    ]));

    assert_eq!(
        state
            .visible_skills()
            .iter()
            .map(|skill| skill.name.as_str())
            .collect::<Vec<_>>(),
        ["zeta", "alpha", "beta"]
    );
    assert!(state.visible_skills()[0].selected);
    assert_eq!(state.filter(), "");
    assert_eq!(state.source_filter(), SourceFilter::All);
    assert_eq!(state.latest_result(), None);
}

#[test]
fn filtering_matches_name_and_description_and_keeps_selection_predictable() {
    let mut state = AppState::new(inventory([
        skill("zeta", "No match", SkillSource::Canonical),
        skill("alpha", "Find me by DESCRIPTION", SkillSource::Imported),
        skill("beta", "No match", SkillSource::AgentOnly),
    ]));
    state.reduce(AppAction::MoveSelection(SelectionDelta::Next));
    state.reduce(AppAction::MoveSelection(SelectionDelta::Next));

    state.reduce(AppAction::FilterChanged("description".to_string()));

    let visible = state.visible_skills();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].name, "alpha");
    assert!(visible[0].selected);
    assert_eq!(
        state.selected_detail().expect("selected").name.as_str(),
        "alpha"
    );

    state.reduce(AppAction::FilterChanged("missing".to_string()));
    assert!(state.visible_skills().is_empty());
    assert_eq!(state.selected_detail(), None);

    state.reduce(AppAction::FilterChanged(String::new()));
    assert_eq!(
        state
            .visible_skills()
            .iter()
            .map(|skill| skill.name.as_str())
            .collect::<Vec<_>>(),
        ["zeta", "alpha", "beta"]
    );

    state.reduce(AppAction::AppendFilter('A'));
    state.reduce(AppAction::AppendFilter('L'));
    assert_eq!(state.filter(), "AL");
    assert_eq!(selected_name(&state).as_deref(), Some("alpha"));
    state.reduce(AppAction::DeleteFilterChar);
    assert_eq!(state.filter(), "A");
}

#[test]
fn source_filter_can_show_imported_skills_only() {
    let mut state = AppState::new(inventory([
        skill("alpha", "Canonical", SkillSource::Canonical),
        skill("beta", "Imported", SkillSource::Imported),
        skill("gamma", "Agent only", SkillSource::AgentOnly),
    ]));

    state.reduce(AppAction::ToggleSourceFilter);

    assert_eq!(state.source_filter(), SourceFilter::Imported);
    assert_eq!(visible_names(&state), ["beta"]);
    assert_eq!(selected_name(&state).as_deref(), Some("beta"));
}

#[test]
fn source_filter_preserves_selected_import_by_name_when_visible() {
    let mut state = AppState::new(inventory([
        skill("alpha", "Canonical", SkillSource::Canonical),
        skill("beta", "Imported", SkillSource::Imported),
        skill("gamma", "Other import", SkillSource::Imported),
    ]));
    state.reduce(AppAction::MoveSelection(SelectionDelta::Next));

    state.reduce(AppAction::ToggleSourceFilter);

    assert_eq!(visible_names(&state), ["beta", "gamma"]);
    assert_eq!(selected_name(&state).as_deref(), Some("beta"));
}

#[test]
fn source_filter_empty_state_has_no_selection() {
    let mut state = AppState::new(inventory([
        skill("alpha", "Canonical", SkillSource::Canonical),
        skill("gamma", "Agent only", SkillSource::AgentOnly),
    ]));

    state.reduce(AppAction::ToggleSourceFilter);

    assert!(state.visible_skills().is_empty());
    assert_eq!(state.selected_detail(), None);
}

#[test]
fn source_filter_composes_with_text_filter_and_navigation() {
    let mut state = AppState::new(inventory([
        skill("canonical-match", "Shared topic", SkillSource::Canonical),
        skill("imported-match", "Shared topic", SkillSource::Imported),
        skill("imported-other", "Different topic", SkillSource::Imported),
        skill("agent-match", "Shared topic", SkillSource::AgentOnly),
    ]));

    state.reduce(AppAction::FilterChanged("shared".to_string()));
    state.reduce(AppAction::ToggleSourceFilter);

    assert_eq!(visible_names(&state), ["imported-match"]);
    assert_eq!(selected_name(&state).as_deref(), Some("imported-match"));
    state.reduce(AppAction::MoveSelection(SelectionDelta::Previous));
    assert_eq!(selected_name(&state).as_deref(), Some("imported-match"));
    state.reduce(AppAction::MoveSelection(SelectionDelta::Next));
    assert_eq!(selected_name(&state).as_deref(), Some("imported-match"));

    state.reduce(AppAction::ToggleSourceFilter);

    assert_eq!(
        visible_names(&state),
        ["canonical-match", "imported-match", "agent-match"]
    );
    assert_eq!(selected_name(&state).as_deref(), Some("imported-match"));
}

#[test]
fn source_filter_refresh_preserves_selected_import_by_name() {
    let mut state = AppState::new(inventory([
        skill("alpha", "Canonical", SkillSource::Canonical),
        skill("beta", "Imported", SkillSource::Imported),
        skill("gamma", "Other import", SkillSource::Imported),
    ]));
    state.reduce(AppAction::MoveSelection(SelectionDelta::Next));
    state.reduce(AppAction::ToggleSourceFilter);

    state.update_inventory(inventory([
        skill("gamma", "Other import refreshed", SkillSource::Imported),
        skill("alpha", "Canonical refreshed", SkillSource::Canonical),
        skill("beta", "Imported refreshed", SkillSource::Imported),
    ]));

    assert_eq!(visible_names(&state), ["gamma", "beta"]);
    assert_eq!(selected_name(&state).as_deref(), Some("beta"));
    assert!(!state.needs_refresh());
}

#[test]
fn source_and_text_filter_refresh_preserves_selected_import_by_name() {
    let mut state = AppState::new(inventory([
        skill("canonical-match", "Shared topic", SkillSource::Canonical),
        skill("imported-match", "Shared topic", SkillSource::Imported),
        skill("imported-other", "Shared topic", SkillSource::Imported),
        skill("imported-skip", "Different topic", SkillSource::Imported),
    ]));
    state.reduce(AppAction::FilterChanged("shared".to_string()));
    state.reduce(AppAction::ToggleSourceFilter);
    state.reduce(AppAction::MoveSelection(SelectionDelta::Next));

    state.update_inventory(inventory([
        skill("imported-skip", "Different topic", SkillSource::Imported),
        skill(
            "imported-other",
            "Shared topic refreshed",
            SkillSource::Imported,
        ),
        skill(
            "canonical-match",
            "Shared topic refreshed",
            SkillSource::Canonical,
        ),
        skill(
            "imported-match",
            "Shared topic refreshed",
            SkillSource::Imported,
        ),
    ]));

    assert_eq!(visible_names(&state), ["imported-other", "imported-match"]);
    assert_eq!(selected_name(&state).as_deref(), Some("imported-other"));
}

#[test]
fn source_filter_refresh_falls_back_when_selected_import_disappears() {
    let mut state = AppState::new(inventory([
        skill("alpha", "Canonical", SkillSource::Canonical),
        skill("beta", "Imported", SkillSource::Imported),
        skill("gamma", "Other import", SkillSource::Imported),
    ]));
    state.reduce(AppAction::MoveSelection(SelectionDelta::Next));
    state.reduce(AppAction::ToggleSourceFilter);

    state.update_inventory(inventory([
        skill("alpha", "Canonical refreshed", SkillSource::Canonical),
        skill("gamma", "Other import refreshed", SkillSource::Imported),
        skill("delta", "New import", SkillSource::Imported),
    ]));

    assert_eq!(visible_names(&state), ["gamma", "delta"]);
    assert_eq!(selected_name(&state).as_deref(), Some("gamma"));
}

#[test]
fn source_filter_refresh_to_no_imports_clears_selection() {
    let mut state = AppState::new(inventory([
        skill("alpha", "Canonical", SkillSource::Canonical),
        skill("beta", "Imported", SkillSource::Imported),
    ]));
    state.reduce(AppAction::MoveSelection(SelectionDelta::Next));
    state.reduce(AppAction::ToggleSourceFilter);

    state.update_inventory(inventory([
        skill("alpha", "Canonical refreshed", SkillSource::Canonical),
        skill("agent", "Agent only", SkillSource::AgentOnly),
    ]));

    assert!(state.visible_skills().is_empty());
    assert_eq!(state.selected_detail(), None);
}

#[test]
fn keyboard_navigation_is_bounded_and_uses_filtered_rows() {
    let mut state = AppState::new(inventory([
        skill("alpha", "match", SkillSource::Canonical),
        skill("beta", "skip", SkillSource::Imported),
        skill("gamma", "match", SkillSource::AgentOnly),
    ]));

    state.reduce(AppAction::MoveSelection(SelectionDelta::Previous));
    assert_eq!(selected_name(&state).as_deref(), Some("alpha"));

    state.reduce(AppAction::FilterChanged("match".to_string()));
    state.reduce(AppAction::MoveSelection(SelectionDelta::Next));
    assert_eq!(selected_name(&state).as_deref(), Some("gamma"));

    state.reduce(AppAction::MoveSelection(SelectionDelta::Next));
    assert_eq!(selected_name(&state).as_deref(), Some("gamma"));
}

#[test]
fn selected_skill_detail_projects_core_status_fields() {
    let mut entry = skill("alpha", "Detailed skill", SkillSource::Imported);
    entry.source_repository = Some(skill_importer::ImportSourceRepository {
        repository: "https://example.test/skills.git".to_string(),
        skill_path: "skills/alpha".to_string(),
    });
    entry.enablement = AgentEnablement::Both;
    entry.agent_entries = AgentEntries {
        claude_code: AgentEntryStatus::ImportedSymlink,
        codex: AgentEntryStatus::SkillDirectory,
    };
    let state = AppState::new(inventory([entry]));

    let detail = state.selected_detail().expect("selected detail");
    assert_eq!(detail.name, "alpha");
    assert_eq!(detail.description.as_deref(), Some("Detailed skill"));
    assert_eq!(detail.source, SkillSource::Imported);
    assert_eq!(
        detail.source_repository.as_ref(),
        Some(&skill_importer::ImportSourceRepository {
            repository: "https://example.test/skills.git".to_string(),
            skill_path: "skills/alpha".to_string(),
        })
    );
    assert_eq!(detail.enablement, AgentEnablement::Both);
    assert_eq!(
        detail.agent_entries.claude_code,
        AgentEntryStatus::ImportedSymlink
    );
    assert_eq!(detail.agent_entries.codex, AgentEntryStatus::SkillDirectory);
}

#[test]
fn visible_skills_project_promoted_status_from_inventory() {
    let mut promoted_canonical = skill(
        "promoted-canonical",
        "Canonical after promotion",
        SkillSource::Canonical,
    );
    promoted_canonical.promoted = true;
    let imported_unpromoted = skill("imported", "Imported only", SkillSource::Imported);
    let canonical_unpromoted = skill("canonical", "Canonical only", SkillSource::Canonical);
    let agent_only = skill("agent-only", "Agent only", SkillSource::AgentOnly);

    let state = AppState::new(inventory([
        promoted_canonical,
        imported_unpromoted,
        canonical_unpromoted,
        agent_only,
    ]));

    assert_eq!(
        state
            .visible_skills()
            .iter()
            .map(|skill| (skill.name.as_str(), skill.promoted))
            .collect::<Vec<_>>(),
        [
            ("promoted-canonical", true),
            ("imported", false),
            ("canonical", false),
            ("agent-only", false),
        ]
    );
}

#[test]
fn visible_skill_rows_project_enablement_in_visible_order() {
    let state = AppState::new(inventory([
        skill_with_enablement("disabled", AgentEnablement::Neither),
        skill_with_enablement("claude", AgentEnablement::ClaudeCode),
        skill_with_enablement("codex", AgentEnablement::Codex),
        skill_with_enablement("both", AgentEnablement::Both),
    ]));

    assert_eq!(
        state
            .visible_skills()
            .iter()
            .map(|skill| skill.enablement)
            .collect::<Vec<_>>(),
        [
            AgentEnablement::Neither,
            AgentEnablement::ClaudeCode,
            AgentEnablement::Codex,
            AgentEnablement::Both,
        ]
    );
}

#[test]
fn main_mode_hints_show_context_aware_agent_toggles() {
    let mut state = AppState::new(inventory([
        skill_with_agent_entries(
            "alpha",
            AgentEntryStatus::Missing,
            AgentEntryStatus::CanonicalSymlink,
        ),
        skill_with_agent_entries(
            "beta",
            AgentEntryStatus::ImportedSymlink,
            AgentEntryStatus::Missing,
        ),
    ]));

    assert_eq!(
        state.action_hints()[1..3],
        [
            "c enable Claude Code".to_string(),
            "x disable Codex".to_string()
        ]
    );

    state.reduce(AppAction::MoveSelection(SelectionDelta::Next));

    assert_eq!(
        state.action_hints()[1..3],
        [
            "c disable Claude Code".to_string(),
            "x enable Codex".to_string()
        ]
    );
}

#[test]
fn main_mode_hints_are_neutral_when_no_skill_is_selected() {
    let mut state = AppState::new(inventory([skill("alpha", "First", SkillSource::Canonical)]));

    state.reduce(AppAction::FilterChanged("missing".to_string()));

    assert_eq!(
        state.action_hints()[1..3],
        ["c Claude Code".to_string(), "x Codex".to_string()]
    );
}

#[test]
fn visible_results_replace_success_with_failure_without_stale_text() {
    let mut state = AppState::new(inventory([skill("alpha", "First", SkillSource::Canonical)]));

    state.reduce(AppAction::OperationFinished(AppOperationResult::success(
        "enable",
        Some("alpha".to_string()),
        2,
    )));
    let success = state.status_view().expect("success status");
    assert!(success.success);
    assert_eq!(success.operation, "enable");
    assert_eq!(success.skill_name.as_deref(), Some("alpha"));
    assert_eq!(success.message, "success: 2 actions");

    state.reduce(AppAction::OperationFinished(AppOperationResult::failure(
        "disable",
        Some("alpha".to_string()),
        "unsafe entry",
    )));
    let failure = state.status_view().expect("failure status");
    assert!(!failure.success);
    assert_eq!(failure.operation, "disable");
    assert_eq!(failure.message, "failed: unsafe entry");
    assert!(!failure.message.contains("2 actions"));
}

#[test]
fn operation_failure_status_preserves_pending_request_context() {
    let mut state = AppState::new(inventory([skill("alpha", "First", SkillSource::Canonical)]));
    state.reduce(AppAction::ToggleSelectedForAgent(SkillAgent::Codex));

    state.reduce(AppAction::CompletePendingOperation(Err(
        "unsafe entry".to_string()
    )));

    let failure = state.status_view().expect("failure status");
    assert_eq!(failure.operation, "enable");
    assert_eq!(failure.skill_name.as_deref(), Some("alpha"));
    assert_eq!(failure.message, "failed: unsafe entry");
}

#[test]
fn operation_failure_status_preserves_context_after_terminal_takes_request() {
    let mut state = AppState::new(inventory([skill_with_agent_entries(
        "alpha",
        AgentEntryStatus::Missing,
        AgentEntryStatus::SkillDirectory,
    )]));
    state.reduce(AppAction::ToggleSelectedForAgent(SkillAgent::Codex));
    let request = state.take_pending_request().expect("pending request");

    state.reduce(AppAction::CompleteOperation {
        request: Some(request),
        result: Err("unsafe entry".to_string()),
    });

    let failure = state.status_view().expect("failure status");
    assert_eq!(failure.operation, "disable");
    assert_eq!(failure.skill_name.as_deref(), Some("alpha"));
    assert_eq!(failure.message, "failed: unsafe entry");
}

#[test]
fn repository_selection_mode_shows_candidates_without_completed_result() {
    let mut state = AppState::new(inventory([skill("alpha", "First", SkillSource::Canonical)]));

    state.reduce(AppAction::RepositorySelectionLoaded(repository_selection()));

    assert!(matches!(
        state.mode(),
        AppInteractionMode::RepositorySelection { .. }
    ));
    let candidates = state.repository_candidates();
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].name, "repo-alpha");
    assert_eq!(candidates[0].description.as_deref(), Some("First repo"));
    assert_eq!(candidates[0].relative_path, "skills/repo-alpha");
    assert!(candidates[0].selected);
    assert!(candidates[0].focused);
    assert!(!candidates[0].checked);
    assert_eq!(state.latest_result(), None);
}

#[test]
fn repository_candidate_toggle_tracks_checked_state_without_dispatching() {
    let mut state = AppState::new(inventory([]));
    state.reduce(AppAction::RepositorySelectionLoaded(repository_selection()));
    state.reduce(AppAction::MoveRepositoryCandidate(SelectionDelta::Next));

    state.reduce(AppAction::ToggleRepositoryCandidate);

    assert_eq!(state.pending_request(), None);
    let candidates = state.repository_candidates();
    assert!(!candidates[0].focused);
    assert!(!candidates[0].checked);
    assert!(candidates[1].focused);
    assert!(candidates[1].checked);

    state.reduce(AppAction::ToggleRepositoryCandidate);
    assert!(!state.repository_candidates()[1].checked);
}

#[test]
fn repository_candidate_choice_dispatches_focused_request_without_storage_mutation() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut state = AppState::new(inventory([]));
    state.reduce(AppAction::RepositorySelectionLoaded(repository_selection()));
    state.reduce(AppAction::MoveRepositoryCandidate(SelectionDelta::Next));

    state.reduce(AppAction::ChooseRepositoryCandidate);

    assert_eq!(
        state.pending_request(),
        Some(&AppOperationRequest::RepositoryImport {
            repository: "https://example.test/repo.git".to_string(),
            selected_skill_paths: vec!["skills/repo-beta".to_string()],
        })
    );
    assert!(
        !temp.path().join("repo-beta").exists(),
        "reducer should only dispatch a request"
    );
}

#[test]
fn repository_candidate_choice_dispatches_checked_paths_in_selection_order() {
    let mut state = AppState::new(inventory([]));
    state.reduce(AppAction::RepositorySelectionLoaded(repository_selection()));
    state.reduce(AppAction::ToggleRepositoryCandidate);
    state.reduce(AppAction::MoveRepositoryCandidate(SelectionDelta::Next));
    state.reduce(AppAction::ToggleRepositoryCandidate);

    state.reduce(AppAction::ChooseRepositoryCandidate);

    assert_eq!(
        state.pending_request(),
        Some(&AppOperationRequest::RepositoryImport {
            repository: "https://example.test/repo.git".to_string(),
            selected_skill_paths: vec![
                "skills/repo-alpha".to_string(),
                "skills/repo-beta".to_string(),
            ],
        })
    );
}

#[test]
fn repository_completion_success_exits_selection_and_failure_preserves_retry_context() {
    let mut state = AppState::new(inventory([]));
    state.reduce(AppAction::RepositorySelectionLoaded(repository_selection()));
    state.reduce(AppAction::ToggleRepositoryCandidate);
    state.reduce(AppAction::MoveRepositoryCandidate(SelectionDelta::Next));
    state.reduce(AppAction::ToggleRepositoryCandidate);
    state.reduce(AppAction::ChooseRepositoryCandidate);

    state.reduce(AppAction::CompletePendingOperation(Err(
        "collision for repo-beta".to_string(),
    )));

    assert_eq!(state.pending_request(), None);
    assert!(matches!(
        state.mode(),
        AppInteractionMode::RepositorySelection { .. }
    ));
    let candidates = state.repository_candidates();
    assert_eq!(candidates[1].relative_path, "skills/repo-beta");
    assert!(candidates[1].focused);
    assert!(candidates[0].checked);
    assert!(candidates[1].checked);
    let failure = state.status_view().expect("failure");
    assert!(!failure.success);
    assert!(failure.message.contains("collision for repo-beta"));

    state.reduce(AppAction::ChooseRepositoryCandidate);
    state.reduce(AppAction::CompletePendingOperation(Ok(
        AppOperationResult::success("repository import", Some("repo-beta".to_string()), 4),
    )));

    assert_eq!(state.pending_request(), None);
    assert!(matches!(state.mode(), AppInteractionMode::Main));
    assert!(state.needs_refresh());
    assert!(state.status_view().expect("success").success);
}

#[test]
fn enable_disable_and_import_intents_become_pending_requests() {
    let mut state = AppState::new(inventory([skill("alpha", "First", SkillSource::Canonical)]));

    state.reduce(AppAction::ToggleSelectedForAgent(SkillAgent::ClaudeCode));
    assert_eq!(
        state.pending_request(),
        Some(&AppOperationRequest::EnableSkill {
            skill_name: "alpha".to_string(),
            agent: SkillAgent::ClaudeCode,
        })
    );

    state.reduce(AppAction::ClearPendingRequest);
    state.update_inventory(inventory([skill_with_agent_entries(
        "alpha",
        AgentEntryStatus::ImportedSymlink,
        AgentEntryStatus::Missing,
    )]));
    state.reduce(AppAction::ToggleSelectedForAgent(SkillAgent::ClaudeCode));
    assert_eq!(
        state.pending_request(),
        Some(&AppOperationRequest::DisableSkill {
            skill_name: "alpha".to_string(),
            agent: SkillAgent::ClaudeCode,
        })
    );

    state.reduce(AppAction::ClearPendingRequest);
    state.reduce(AppAction::BeginImportPrompt(AppImportSource::Url));
    state.reduce(AppAction::PromptChanged(
        "https://example.test/skill.md".to_string(),
    ));
    state.reduce(AppAction::SubmitPrompt);
    assert_eq!(
        state.pending_request(),
        Some(&AppOperationRequest::ImportUrl {
            url: "https://example.test/skill.md".to_string(),
        })
    );
}

#[test]
fn agent_toggles_dispatch_enable_when_agent_entry_is_disabled() {
    for disabled_status in [AgentEntryStatus::Missing, AgentEntryStatus::BrokenSymlink] {
        let mut state = AppState::new(inventory([skill_with_agent_entries(
            "alpha",
            disabled_status,
            disabled_status,
        )]));

        state.reduce(AppAction::ToggleSelectedForAgent(SkillAgent::ClaudeCode));
        assert_eq!(
            state.pending_request(),
            Some(&AppOperationRequest::EnableSkill {
                skill_name: "alpha".to_string(),
                agent: SkillAgent::ClaudeCode,
            }),
            "Claude Code status {disabled_status:?} should enable"
        );

        state.reduce(AppAction::ToggleSelectedForAgent(SkillAgent::Codex));
        assert_eq!(
            state.pending_request(),
            Some(&AppOperationRequest::EnableSkill {
                skill_name: "alpha".to_string(),
                agent: SkillAgent::Codex,
            }),
            "Codex status {disabled_status:?} should enable"
        );
    }
}

#[test]
fn agent_toggles_dispatch_disable_when_agent_entry_is_enabled() {
    for enabled_status in [
        AgentEntryStatus::SkillDirectory,
        AgentEntryStatus::CanonicalSymlink,
        AgentEntryStatus::ImportedSymlink,
        AgentEntryStatus::ExternalSymlink,
    ] {
        let mut state = AppState::new(inventory([skill_with_agent_entries(
            "alpha",
            enabled_status,
            enabled_status,
        )]));

        state.reduce(AppAction::ToggleSelectedForAgent(SkillAgent::ClaudeCode));
        assert_eq!(
            state.pending_request(),
            Some(&AppOperationRequest::DisableSkill {
                skill_name: "alpha".to_string(),
                agent: SkillAgent::ClaudeCode,
            }),
            "Claude Code status {enabled_status:?} should disable"
        );

        state.reduce(AppAction::ToggleSelectedForAgent(SkillAgent::Codex));
        assert_eq!(
            state.pending_request(),
            Some(&AppOperationRequest::DisableSkill {
                skill_name: "alpha".to_string(),
                agent: SkillAgent::Codex,
            }),
            "Codex status {enabled_status:?} should disable"
        );
    }
}

#[test]
fn both_enabled_toggle_disables_only_the_requested_agent() {
    let mut state = AppState::new(inventory([skill_with_agent_entries(
        "alpha",
        AgentEntryStatus::CanonicalSymlink,
        AgentEntryStatus::ImportedSymlink,
    )]));

    state.reduce(AppAction::ToggleSelectedForAgent(SkillAgent::Codex));

    assert_eq!(
        state.pending_request(),
        Some(&AppOperationRequest::DisableSkill {
            skill_name: "alpha".to_string(),
            agent: SkillAgent::Codex,
        })
    );
}

#[test]
fn agent_toggle_without_selection_leaves_pending_request_unchanged() {
    let mut state = AppState::new(inventory([skill("alpha", "First", SkillSource::Canonical)]));
    state.reduce(AppAction::BeginImportPrompt(AppImportSource::Url));
    state.reduce(AppAction::PromptChanged(
        "https://example.test/skill.md".to_string(),
    ));
    state.reduce(AppAction::SubmitPrompt);
    state.reduce(AppAction::FilterChanged("missing".to_string()));

    state.reduce(AppAction::ToggleSelectedForAgent(SkillAgent::Codex));

    assert_eq!(
        state.pending_request(),
        Some(&AppOperationRequest::ImportUrl {
            url: "https://example.test/skill.md".to_string(),
        })
    );
}

#[test]
fn agent_toggle_overwrites_existing_pending_request() {
    let mut state = AppState::new(inventory([skill_with_agent_entries(
        "alpha",
        AgentEntryStatus::Missing,
        AgentEntryStatus::SkillDirectory,
    )]));
    state.reduce(AppAction::BeginImportPrompt(AppImportSource::Url));
    state.reduce(AppAction::PromptChanged(
        "https://example.test/skill.md".to_string(),
    ));
    state.reduce(AppAction::SubmitPrompt);

    state.reduce(AppAction::ToggleSelectedForAgent(SkillAgent::Codex));

    assert_eq!(
        state.pending_request(),
        Some(&AppOperationRequest::DisableSkill {
            skill_name: "alpha".to_string(),
            agent: SkillAgent::Codex,
        })
    );
}

#[test]
fn prompt_cancel_and_backspace_are_distinct_from_repository_selection() {
    let mut state = AppState::new(inventory([]));
    state.reduce(AppAction::BeginImportPrompt(AppImportSource::Url));
    state.reduce(AppAction::PromptChanged("abc".to_string()));

    state.reduce(AppAction::DeletePromptChar);
    assert_eq!(state.prompt_text(), "ab");

    state.reduce(AppAction::CancelPrompt);
    assert!(matches!(state.mode(), AppInteractionMode::Main));
    assert_eq!(state.prompt_text(), "");
    assert_eq!(state.pending_request(), None);
}

#[test]
fn promote_and_delete_require_confirmation_before_pending_request() {
    let mut state = AppState::new(inventory([skill("alpha", "First", SkillSource::Imported)]));

    state.reduce(AppAction::BeginConfirmation(ConfirmationOperation::Promote));
    assert!(matches!(
        state.mode(),
        AppInteractionMode::Confirm {
            operation: ConfirmationOperation::Promote,
            ..
        }
    ));
    assert_eq!(state.pending_request(), None);
    state.reduce(AppAction::ConfirmPending);
    assert_eq!(
        state.pending_request(),
        Some(&AppOperationRequest::PromoteSkill {
            skill_name: "alpha".to_string(),
        })
    );

    state.reduce(AppAction::ClearPendingRequest);
    state.reduce(AppAction::BeginConfirmation(ConfirmationOperation::Delete));
    state.reduce(AppAction::ConfirmPending);
    assert_eq!(
        state.pending_request(),
        Some(&AppOperationRequest::DeleteImport {
            skill_name: "alpha".to_string(),
        })
    );
}

#[test]
fn promoted_skill_uses_promotion_shortcut_to_queue_unpromote() {
    let mut promoted = skill("alpha", "First", SkillSource::Imported);
    promoted.promoted = true;
    let mut state = AppState::new(inventory([promoted]));

    state.reduce(AppAction::BeginConfirmation(ConfirmationOperation::Promote));
    assert!(matches!(
        state.mode(),
        AppInteractionMode::Confirm {
            operation: ConfirmationOperation::Unpromote,
            ..
        }
    ));
    state.reduce(AppAction::ConfirmPending);

    assert_eq!(
        state.pending_request(),
        Some(&AppOperationRequest::UnpromoteSkill {
            skill_name: "alpha".to_string(),
        })
    );
}

fn inventory<const N: usize>(skills: [SkillEntry; N]) -> SkillInventory {
    SkillInventory {
        skills: skills.into_iter().collect(),
        source_repositories: Vec::new(),
    }
}

fn skill(name: &str, description: &str, source: SkillSource) -> SkillEntry {
    SkillEntry {
        name: name.to_string(),
        description: Some(description.to_string()),
        source,
        source_repository: None,
        promoted: false,
        enablement: AgentEnablement::Neither,
        agent_entries: AgentEntries {
            claude_code: AgentEntryStatus::Missing,
            codex: AgentEntryStatus::Missing,
        },
    }
}

fn skill_with_enablement(name: &str, enablement: AgentEnablement) -> SkillEntry {
    let mut entry = skill(name, "Skill", SkillSource::Canonical);
    entry.enablement = enablement;
    entry
}

fn skill_with_agent_entries(
    name: &str,
    claude_code: AgentEntryStatus,
    codex: AgentEntryStatus,
) -> SkillEntry {
    let mut entry = skill(name, "Skill", SkillSource::Canonical);
    entry.agent_entries = AgentEntries { claude_code, codex };
    entry.enablement = match (
        entry_status_is_enabled(claude_code),
        entry_status_is_enabled(codex),
    ) {
        (true, true) => AgentEnablement::Both,
        (true, false) => AgentEnablement::ClaudeCode,
        (false, true) => AgentEnablement::Codex,
        (false, false) => AgentEnablement::Neither,
    };
    entry
}

fn entry_status_is_enabled(status: AgentEntryStatus) -> bool {
    matches!(
        status,
        AgentEntryStatus::SkillDirectory
            | AgentEntryStatus::CanonicalSymlink
            | AgentEntryStatus::ImportedSymlink
            | AgentEntryStatus::ExternalSymlink
    )
}

fn selected_name(state: &AppState) -> Option<String> {
    state.selected_detail().map(|detail| detail.name)
}

fn visible_names(state: &AppState) -> Vec<String> {
    state
        .visible_skills()
        .into_iter()
        .map(|skill| skill.name)
        .collect()
}

fn repository_selection() -> RepositorySkillSelection {
    RepositorySkillSelection {
        repository: "https://example.test/repo.git".to_string(),
        skills: vec![
            candidate("repo-alpha", "First repo", "skills/repo-alpha"),
            candidate("repo-beta", "Second repo", "skills/repo-beta"),
        ],
    }
}

fn candidate(name: &str, description: &str, relative_path: &str) -> RepositorySkillCandidate {
    RepositorySkillCandidate {
        name: name.to_string(),
        description: Some(description.to_string()),
        relative_path: relative_path.to_string(),
    }
}
