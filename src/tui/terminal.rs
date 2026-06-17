use std::{fs, io, path::Path, process::Command};

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
};

use crate::{
    DeleteImportRequest, DiscoveryRoots, ImportLocalPathRequest, ImportMarkdownRequest,
    ImportRepositoryRequest, ImportUrlRequest, PromoteSkillRequest, SkillRepositoryCheckout,
    SkillRepositoryFetchError, SkillRepositoryProvider, SkillUrlFetcher, UnpromoteSkillRequest,
    analyzer::{AnalyzeSkillRequest, SkillAnalyzerLauncher, TerminalSkillAnalyzerLauncher},
    discover_skills, workflow,
};

use super::{
    AppAction, AppInput, AppOperationRequest, AppOperationResult, AppState, InputOutcome,
    action_for_input, render_app,
};

pub fn run_tui(
    roots: &DiscoveryRoots,
    url_fetcher: &impl SkillUrlFetcher,
) -> Result<(), io::Error> {
    let repository_provider = GitRepositoryProvider;
    let analyzer_launcher = TerminalSkillAnalyzerLauncher;
    run_tui_with_services(roots, url_fetcher, &repository_provider, &analyzer_launcher)
}

fn run_tui_with_services(
    roots: &DiscoveryRoots,
    url_fetcher: &impl SkillUrlFetcher,
    repository_provider: &impl SkillRepositoryProvider,
    analyzer_launcher: &impl SkillAnalyzerLauncher,
) -> Result<(), io::Error> {
    let inventory = discover_skills(roots)?;
    let mut state = AppState::new(inventory);
    let mut stdout = io::stdout();
    let _cleanup = TerminalCleanup::enter(&mut stdout)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = run_event_loop(
        &mut terminal,
        roots,
        url_fetcher,
        repository_provider,
        analyzer_launcher,
        &mut state,
    );
    result.and_then(|()| terminal.show_cursor())
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    roots: &DiscoveryRoots,
    url_fetcher: &impl SkillUrlFetcher,
    repository_provider: &impl SkillRepositoryProvider,
    analyzer_launcher: &impl SkillAnalyzerLauncher,
    state: &mut AppState,
) -> Result<(), io::Error> {
    loop {
        refresh_inventory_if_needed(roots, state);
        terminal.draw(|frame| render_app(frame, state))?;
        if let Event::Key(key) = event::read()? {
            let Some(input) = input_from_key_code(key.code) else {
                continue;
            };
            match action_for_input(state.mode(), input) {
                InputOutcome::Action(action) => {
                    let action = prepare_action(roots, state, action);
                    handle_action(terminal, state, action, |request| {
                        execute_operation_request(
                            roots,
                            url_fetcher,
                            repository_provider,
                            analyzer_launcher,
                            request,
                        )
                    })?;
                }
                InputOutcome::Quit => break,
                InputOutcome::Ignored => {}
            }
        }
    }
    Ok(())
}

fn prepare_action(roots: &DiscoveryRoots, state: &AppState, action: AppAction) -> AppAction {
    if action == AppAction::BeginConfirmation(super::ConfirmationOperation::Promote)
        && let Some(skill) = state.selected_detail()
        && !skill.promoted
        && draft_import_manifest_exists(&roots.imports_root.join(&skill.name))
        && promoted_destination_is_directory(&roots.canonical_root.join(&skill.name))
    {
        return AppAction::BeginConfirmation(super::ConfirmationOperation::PromoteOverwrite);
    }

    action
}

fn promoted_destination_is_directory(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir())
}

fn draft_import_manifest_exists(import_path: &Path) -> bool {
    fs::metadata(import_path.join("import.json")).is_ok_and(|metadata| metadata.is_file())
}

fn handle_action<B: Backend>(
    terminal: &mut Terminal<B>,
    state: &mut AppState,
    action: AppAction,
    execute_request: impl FnOnce(AppOperationRequest) -> Result<TerminalOperationOutcome, String>,
) -> Result<(), <B as Backend>::Error> {
    handle_action_with_pending_observer(terminal, state, action, execute_request, || {})
}

fn handle_action_with_pending_observer<B: Backend>(
    terminal: &mut Terminal<B>,
    state: &mut AppState,
    action: AppAction,
    execute_request: impl FnOnce(AppOperationRequest) -> Result<TerminalOperationOutcome, String>,
    mut after_pending_draw: impl FnMut(),
) -> Result<(), <B as Backend>::Error> {
    state.reduce(action);
    if state.pending_request().is_some() {
        terminal.draw(|frame| render_app(frame, state))?;
        after_pending_draw();
    }
    if let Some(request) = state.take_pending_request() {
        let request_context = request.clone();
        match execute_request(request) {
            Ok(TerminalOperationOutcome::Completed(result)) => {
                state.reduce(AppAction::CompleteOperation {
                    request: Some(request_context),
                    result: Ok(result),
                });
            }
            Ok(TerminalOperationOutcome::RepositorySelection(selection)) => {
                state.reduce(AppAction::RepositorySelectionLoaded(selection));
            }
            Err(reason) => {
                state.reduce(AppAction::CompleteOperation {
                    request: Some(request_context),
                    result: Err(reason),
                });
            }
        }
    }
    Ok(())
}

fn input_from_key_code(code: KeyCode) -> Option<AppInput> {
    match code {
        KeyCode::Up => Some(AppInput::Up),
        KeyCode::Down => Some(AppInput::Down),
        KeyCode::Enter => Some(AppInput::Enter),
        KeyCode::Esc => Some(AppInput::Escape),
        KeyCode::Backspace => Some(AppInput::Backspace),
        KeyCode::Char(character) => Some(AppInput::Char(character)),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TerminalOperationOutcome {
    Completed(AppOperationResult),
    RepositorySelection(crate::RepositorySkillSelection),
}

fn execute_operation_request(
    roots: &DiscoveryRoots,
    url_fetcher: &impl SkillUrlFetcher,
    repository_provider: &impl SkillRepositoryProvider,
    analyzer_launcher: &impl SkillAnalyzerLauncher,
    request: AppOperationRequest,
) -> Result<TerminalOperationOutcome, String> {
    match request {
        AppOperationRequest::EnableSkill { skill_name, agent } => {
            let agents = [agent];
            execute_workflow_request(
                roots,
                url_fetcher,
                repository_provider,
                "enable",
                workflow::OperationRequest::Enable {
                    skill_name: &skill_name,
                    agents: &agents,
                },
            )
        }
        AppOperationRequest::DisableSkill { skill_name, agent } => {
            let agents = [agent];
            execute_workflow_request(
                roots,
                url_fetcher,
                repository_provider,
                "disable",
                workflow::OperationRequest::Disable {
                    skill_name: &skill_name,
                    agents: &agents,
                },
            )
        }
        AppOperationRequest::PromoteSkill {
            skill_name,
            overwrite,
        } => execute_workflow_request(
            roots,
            url_fetcher,
            repository_provider,
            "promote",
            workflow::OperationRequest::Promote(PromoteSkillRequest {
                skill_name: &skill_name,
                overwrite,
            }),
        ),
        AppOperationRequest::UnpromoteSkill { skill_name } => execute_workflow_request(
            roots,
            url_fetcher,
            repository_provider,
            "unpromote",
            workflow::OperationRequest::Unpromote(UnpromoteSkillRequest {
                skill_name: &skill_name,
            }),
        ),
        AppOperationRequest::DeleteImport { skill_name } => execute_workflow_request(
            roots,
            url_fetcher,
            repository_provider,
            "delete",
            workflow::OperationRequest::Delete(DeleteImportRequest {
                skill_name: &skill_name,
            }),
        ),
        AppOperationRequest::ImportMarkdown { markdown } => execute_workflow_request(
            roots,
            url_fetcher,
            repository_provider,
            "import markdown",
            workflow::OperationRequest::ImportMarkdown(ImportMarkdownRequest {
                markdown: &markdown,
                source_location: Some("tui"),
            }),
        ),
        AppOperationRequest::ImportPath { path } => execute_workflow_request(
            roots,
            url_fetcher,
            repository_provider,
            "import path",
            workflow::OperationRequest::ImportLocalPath(ImportLocalPathRequest {
                path: path.as_path(),
            }),
        ),
        AppOperationRequest::ImportUrl { url } => execute_workflow_request(
            roots,
            url_fetcher,
            repository_provider,
            "import url",
            workflow::OperationRequest::ImportUrl(ImportUrlRequest { url: &url }),
        ),
        AppOperationRequest::RepositoryImport {
            repository,
            selected_skill_paths,
        } => {
            let selected_skill_paths = selected_skill_paths
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            execute_workflow_request(
                roots,
                url_fetcher,
                repository_provider,
                "repository import",
                workflow::OperationRequest::ImportRepository(ImportRepositoryRequest {
                    repository: &repository,
                    selected_skill_paths: &selected_skill_paths,
                }),
            )
        }
        AppOperationRequest::AnalyzeSkill {
            skill_name,
            skill_dir,
        } => {
            let result = analyzer_launcher.launch(AnalyzeSkillRequest {
                skill_name: skill_name.clone(),
                skill_dir,
            })?;
            Ok(TerminalOperationOutcome::Completed(
                AppOperationResult::launched_analysis(skill_name, result.report_html),
            ))
        }
    }
}

fn execute_workflow_request(
    roots: &DiscoveryRoots,
    url_fetcher: &impl SkillUrlFetcher,
    repository_provider: &impl SkillRepositoryProvider,
    operation: &'static str,
    request: workflow::OperationRequest<'_>,
) -> Result<TerminalOperationOutcome, String> {
    let outcome = workflow::execute(roots, request, url_fetcher, repository_provider)
        .map_err(|error| error.to_string())?;
    terminal_outcome_from_workflow(operation, outcome)
}

fn terminal_outcome_from_workflow(
    operation: &'static str,
    outcome: workflow::OperationOutcome,
) -> Result<TerminalOperationOutcome, String> {
    match outcome {
        workflow::OperationOutcome::Import(import) => Ok(TerminalOperationOutcome::Completed(
            AppOperationResult::from_import(operation, &import),
        )),
        workflow::OperationOutcome::RepositoryImport(result) => match result {
            crate::RepositoryImportResult::Imported(import) => {
                Ok(TerminalOperationOutcome::Completed(
                    AppOperationResult::from_import(operation, &import),
                ))
            }
            crate::RepositoryImportResult::ImportedBatch { imports } => {
                let skill_name = match imports.as_slice() {
                    [import] => Some(import.skill_name.clone()),
                    _ => None,
                };
                Ok(TerminalOperationOutcome::Completed(
                    AppOperationResult::success(
                        operation,
                        skill_name,
                        imports.iter().map(|import| import.actions.len()).sum(),
                    ),
                ))
            }
            crate::RepositoryImportResult::Selection(selection) => {
                Ok(TerminalOperationOutcome::RepositorySelection(selection))
            }
        },
        workflow::OperationOutcome::SkillOperation(result) => {
            Ok(TerminalOperationOutcome::Completed(
                AppOperationResult::from_skill_operation(operation, &result),
            ))
        }
        workflow::OperationOutcome::Inventory(_) => {
            Err("workflow list outcome is not a terminal operation result".to_string())
        }
    }
}

fn refresh_inventory_if_needed(roots: &DiscoveryRoots, state: &mut AppState) {
    if !state.needs_refresh() {
        return;
    }

    match discover_skills(roots) {
        Ok(inventory) => state.update_inventory(inventory),
        Err(error) => {
            state.reduce(AppAction::OperationFinished(AppOperationResult::failure(
                "refresh inventory",
                None,
                error.to_string(),
            )));
            state.clear_refresh_needed();
        }
    }
}

struct TerminalCleanup {
    raw_mode: bool,
    alternate_screen: bool,
}

impl TerminalCleanup {
    fn enter(stdout: &mut io::Stdout) -> Result<Self, io::Error> {
        enable_raw_mode()?;
        let mut cleanup = Self {
            raw_mode: true,
            alternate_screen: false,
        };
        execute!(stdout, EnterAlternateScreen)?;
        cleanup.alternate_screen = true;
        Ok(cleanup)
    }
}

impl Drop for TerminalCleanup {
    fn drop(&mut self) {
        if self.alternate_screen {
            let mut stdout = io::stdout();
            let _ = execute!(stdout, LeaveAlternateScreen);
        }
        if self.raw_mode {
            let _ = disable_raw_mode();
        }
    }
}

struct GitRepositoryProvider;

struct GitRepositoryCheckout {
    _temp_dir: tempfile::TempDir,
    path: std::path::PathBuf,
}

impl SkillRepositoryCheckout for GitRepositoryCheckout {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl SkillRepositoryProvider for GitRepositoryProvider {
    type Checkout = GitRepositoryCheckout;

    fn fetch_repository(
        &self,
        repository: &str,
    ) -> Result<Self::Checkout, SkillRepositoryFetchError> {
        let temp_dir = tempfile::tempdir().map_err(|error| SkillRepositoryFetchError {
            message: error.to_string(),
        })?;
        let checkout_path = temp_dir.path().join("checkout");
        let output = Command::new("git")
            .args(["clone", "--depth", "1", repository])
            .arg(&checkout_path)
            .output()
            .map_err(|error| SkillRepositoryFetchError {
                message: format!("failed to run git clone: {error}"),
            })?;

        if !output.status.success() {
            return Err(SkillRepositoryFetchError {
                message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        Ok(GitRepositoryCheckout {
            _temp_dir: temp_dir,
            path: checkout_path,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        fs,
        path::{Path, PathBuf},
        rc::Rc,
    };

    use ratatui::backend::TestBackend;

    use crate::{
        AgentEnablement, AgentEntries, AgentEntryStatus, SkillAgent, SkillEntry, SkillInventory,
        SkillRepositoryCheckout, SkillRepositoryFetchError, SkillRepositoryProvider, SkillSource,
        SkillUrlFetchError, SkillUrlFetcher,
        analyzer::{AnalyzeLaunchResult, AnalyzeSkillRequest},
        tui::AppOperationStatus,
    };

    use super::*;

    #[test]
    fn repository_import_request_uses_core_provider_boundary_for_selection_and_import() {
        let temp = tempfile::tempdir().expect("tempdir");
        let roots = roots(temp.path());
        let repository = temp.path().join("repo");
        write_skill(&repository, "repo-alpha", "First repo skill.");
        write_skill(&repository, "repo-beta", "Second repo skill.");
        let provider = StaticRepositoryProvider {
            repository_path: repository,
        };

        let selection = execute_operation_request(
            &roots,
            &UnusedFetcher,
            &provider,
            &UnusedAnalyzer,
            AppOperationRequest::RepositoryImport {
                repository: "https://example.test/repo.git".to_string(),
                selected_skill_paths: Vec::new(),
            },
        )
        .expect("repository selection succeeds");

        match selection {
            TerminalOperationOutcome::RepositorySelection(selection) => {
                assert_eq!(selection.repository, "https://example.test/repo.git");
                assert_eq!(selection.skills.len(), 2);
                assert_eq!(selection.skills[0].name, "repo-alpha");
            }
            TerminalOperationOutcome::Completed(result) => {
                panic!("expected selection, got completed result {result:?}")
            }
        }
        assert!(
            !roots.imports_root.exists(),
            "selection should not mutate import storage"
        );

        let imported = execute_operation_request(
            &roots,
            &UnusedFetcher,
            &provider,
            &UnusedAnalyzer,
            AppOperationRequest::RepositoryImport {
                repository: "https://example.test/repo.git".to_string(),
                selected_skill_paths: vec!["repo-beta".to_string()],
            },
        )
        .expect("selected repository import succeeds");

        match imported {
            TerminalOperationOutcome::Completed(result) => {
                assert_eq!(result.operation, "repository import");
                assert_eq!(result.skill_name.as_deref(), Some("repo-beta"));
            }
            TerminalOperationOutcome::RepositorySelection(selection) => {
                panic!(
                    "expected completed import, got {} choices",
                    selection.skills.len()
                )
            }
        }
        assert!(
            roots
                .imports_root
                .join("repo-beta")
                .join("SKILL.md")
                .exists()
        );
    }

    #[test]
    fn repository_import_request_with_multiple_selected_paths_reports_aggregate_result() {
        let temp = tempfile::tempdir().expect("tempdir");
        let roots = roots(temp.path());
        let repository = temp.path().join("repo");
        write_skill(&repository, "repo-alpha", "First repo skill.");
        write_skill(&repository, "repo-beta", "Second repo skill.");
        let provider = StaticRepositoryProvider {
            repository_path: repository,
        };

        let imported = execute_operation_request(
            &roots,
            &UnusedFetcher,
            &provider,
            &UnusedAnalyzer,
            AppOperationRequest::RepositoryImport {
                repository: "https://example.test/repo.git".to_string(),
                selected_skill_paths: vec!["repo-alpha".to_string(), "repo-beta".to_string()],
            },
        )
        .expect("selected repository batch import succeeds");

        match imported {
            TerminalOperationOutcome::Completed(result) => {
                assert_eq!(result.operation, "repository import");
                assert_eq!(result.skill_name, None);
                assert_eq!(
                    result.status,
                    AppOperationStatus::Success { action_count: 6 }
                );
            }
            TerminalOperationOutcome::RepositorySelection(selection) => {
                panic!(
                    "expected completed import, got {} choices",
                    selection.skills.len()
                )
            }
        }
        assert!(
            roots
                .imports_root
                .join("repo-alpha")
                .join("SKILL.md")
                .exists()
        );
        assert!(
            roots
                .imports_root
                .join("repo-beta")
                .join("SKILL.md")
                .exists()
        );
    }

    #[test]
    fn analyze_request_uses_launcher_and_reports_async_launch_status() {
        let temp = tempfile::tempdir().expect("tempdir");
        let roots = roots(temp.path());
        let skill_dir = temp.path().join("skill");
        let report_html = temp.path().join("report").join("index.html");
        let analyzer = RecordingAnalyzer {
            result: AnalyzeLaunchResult {
                report_dir: temp.path().join("report"),
                report_html: report_html.clone(),
            },
            requests: RefCell::new(Vec::new()),
        };

        let outcome = execute_operation_request(
            &roots,
            &UnusedFetcher,
            &StaticRepositoryProvider {
                repository_path: temp.path().join("unused"),
            },
            &analyzer,
            AppOperationRequest::AnalyzeSkill {
                skill_name: "alpha".to_string(),
                skill_dir: skill_dir.clone(),
            },
        )
        .expect("analysis launch succeeds");

        assert_eq!(
            analyzer.requests.borrow().as_slice(),
            &[AnalyzeSkillRequest {
                skill_name: "alpha".to_string(),
                skill_dir,
            }]
        );
        match outcome {
            TerminalOperationOutcome::Completed(result) => {
                assert_eq!(result.operation, "analyze");
                assert_eq!(result.skill_name.as_deref(), Some("alpha"));
                assert_eq!(
                    result.status,
                    AppOperationStatus::Launched {
                        report_path: report_html
                    }
                );
            }
            TerminalOperationOutcome::RepositorySelection(_) => {
                panic!("analyze should not return repository selection")
            }
        }
    }

    #[test]
    fn prepare_action_uses_overwrite_confirmation_for_import_with_existing_destination() {
        let temp = tempfile::tempdir().expect("tempdir");
        let roots = roots(temp.path());
        write_skill(&roots.canonical_root, "alpha", "Existing promoted skill.");
        write_skill(&roots.imports_root, "alpha", "Imported draft.");
        write_import_manifest(&roots.imports_root.join("alpha"), false);
        let state = AppState::new(SkillInventory {
            skills: vec![skill("alpha", "Imported draft.", SkillSource::Imported)],
            source_repositories: Vec::new(),
        });

        let action = prepare_action(
            &roots,
            &state,
            AppAction::BeginConfirmation(crate::tui::ConfirmationOperation::Promote),
        );

        assert_eq!(
            action,
            AppAction::BeginConfirmation(crate::tui::ConfirmationOperation::PromoteOverwrite)
        );
    }

    #[test]
    fn prepare_action_uses_overwrite_confirmation_for_canonical_precedence_draft_import() {
        let temp = tempfile::tempdir().expect("tempdir");
        let roots = roots(temp.path());
        write_skill(&roots.canonical_root, "alpha", "Existing promoted skill.");
        let imported = roots.imports_root.join("alpha");
        write_skill(&roots.imports_root, "alpha", "Imported draft.");
        write_import_manifest(&imported, false);
        let inventory = discover_skills(&roots).expect("inventory");
        assert_eq!(inventory.skills.len(), 1);
        assert_eq!(inventory.skills[0].source, SkillSource::Canonical);
        assert!(!inventory.skills[0].promoted);
        let state = AppState::new(inventory);

        let action = prepare_action(
            &roots,
            &state,
            AppAction::BeginConfirmation(crate::tui::ConfirmationOperation::Promote),
        );

        assert_eq!(
            action,
            AppAction::BeginConfirmation(crate::tui::ConfirmationOperation::PromoteOverwrite)
        );
    }

    #[test]
    fn prepare_action_does_not_infer_overwrite_for_canonical_skill() {
        let temp = tempfile::tempdir().expect("tempdir");
        let roots = roots(temp.path());
        write_skill(&roots.canonical_root, "alpha", "Canonical skill.");
        let state = AppState::new(SkillInventory {
            skills: vec![skill("alpha", "Canonical skill.", SkillSource::Canonical)],
            source_repositories: Vec::new(),
        });

        let action = prepare_action(
            &roots,
            &state,
            AppAction::BeginConfirmation(crate::tui::ConfirmationOperation::Promote),
        );

        assert_eq!(
            action,
            AppAction::BeginConfirmation(crate::tui::ConfirmationOperation::Promote)
        );
    }

    #[test]
    fn prepare_action_does_not_offer_overwrite_for_file_destination() {
        let temp = tempfile::tempdir().expect("tempdir");
        let roots = roots(temp.path());
        fs::create_dir_all(&roots.canonical_root).expect("canonical root");
        fs::write(roots.canonical_root.join("alpha"), "not a skill dir").expect("file collision");
        write_skill(&roots.imports_root, "alpha", "Imported draft.");
        write_import_manifest(&roots.imports_root.join("alpha"), false);
        let state = AppState::new(SkillInventory {
            skills: vec![skill("alpha", "Imported draft.", SkillSource::Imported)],
            source_repositories: Vec::new(),
        });

        let action = prepare_action(
            &roots,
            &state,
            AppAction::BeginConfirmation(crate::tui::ConfirmationOperation::Promote),
        );

        assert_eq!(
            action,
            AppAction::BeginConfirmation(crate::tui::ConfirmationOperation::Promote)
        );
    }

    #[cfg(unix)]
    #[test]
    fn prepare_action_does_not_offer_overwrite_for_broken_symlink_destination() {
        let temp = tempfile::tempdir().expect("tempdir");
        let roots = roots(temp.path());
        fs::create_dir_all(&roots.canonical_root).expect("canonical root");
        std::os::unix::fs::symlink(
            temp.path().join("missing-destination"),
            roots.canonical_root.join("alpha"),
        )
        .expect("broken symlink");
        write_skill(&roots.imports_root, "alpha", "Imported draft.");
        write_import_manifest(&roots.imports_root.join("alpha"), false);
        let state = AppState::new(SkillInventory {
            skills: vec![skill("alpha", "Imported draft.", SkillSource::Imported)],
            source_repositories: Vec::new(),
        });

        let action = prepare_action(
            &roots,
            &state,
            AppAction::BeginConfirmation(crate::tui::ConfirmationOperation::Promote),
        );

        assert_eq!(
            action,
            AppAction::BeginConfirmation(crate::tui::ConfirmationOperation::Promote)
        );
    }

    #[test]
    fn pending_frame_is_drawn_before_executing_enable_request_from_toggle() {
        let mut state = AppState::new(SkillInventory {
            skills: vec![skill("alpha", "First", SkillSource::Canonical)],
            source_repositories: Vec::new(),
        });
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let pending_drawn = Rc::new(Cell::new(false));
        let request_executed = Rc::new(Cell::new(false));

        handle_action_with_pending_observer(
            &mut terminal,
            &mut state,
            AppAction::ToggleSelectedForAgent(SkillAgent::Codex),
            {
                let pending_drawn = Rc::clone(&pending_drawn);
                let request_executed = Rc::clone(&request_executed);
                move |request| {
                    assert!(
                        pending_drawn.get(),
                        "pending frame should be drawn before execution starts"
                    );
                    request_executed.set(true);
                    assert_eq!(
                        request,
                        AppOperationRequest::EnableSkill {
                            skill_name: "alpha".to_string(),
                            agent: SkillAgent::Codex,
                        }
                    );
                    Ok(TerminalOperationOutcome::Completed(
                        AppOperationResult::success("enable", Some("alpha".to_string()), 1),
                    ))
                }
            },
            {
                let pending_drawn = Rc::clone(&pending_drawn);
                move || pending_drawn.set(true)
            },
        )
        .expect("handle action");

        assert!(request_executed.get());
        let rendered = terminal_text(&terminal);
        assert!(rendered.contains("Status: pending enable (alpha)"));
        assert!(state.status_view().expect("completion status").success);
    }

    #[test]
    fn pending_frame_is_drawn_before_executing_disable_request_from_toggle() {
        let mut enabled = skill("alpha", "First", SkillSource::Canonical);
        enabled.agent_entries.codex = AgentEntryStatus::CanonicalSymlink;
        let mut state = AppState::new(SkillInventory {
            skills: vec![enabled],
            source_repositories: Vec::new(),
        });
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let pending_drawn = Rc::new(Cell::new(false));
        let request_executed = Rc::new(Cell::new(false));

        handle_action_with_pending_observer(
            &mut terminal,
            &mut state,
            AppAction::ToggleSelectedForAgent(SkillAgent::Codex),
            {
                let pending_drawn = Rc::clone(&pending_drawn);
                let request_executed = Rc::clone(&request_executed);
                move |request| {
                    assert!(
                        pending_drawn.get(),
                        "pending frame should be drawn before execution starts"
                    );
                    request_executed.set(true);
                    assert_eq!(
                        request,
                        AppOperationRequest::DisableSkill {
                            skill_name: "alpha".to_string(),
                            agent: SkillAgent::Codex,
                        }
                    );
                    Ok(TerminalOperationOutcome::Completed(
                        AppOperationResult::success("disable", Some("alpha".to_string()), 1),
                    ))
                }
            },
            {
                let pending_drawn = Rc::clone(&pending_drawn);
                move || pending_drawn.set(true)
            },
        )
        .expect("handle action");

        assert!(request_executed.get());
        let rendered = terminal_text(&terminal);
        assert!(rendered.contains("Status: pending disable (alpha)"));
        assert!(state.status_view().expect("completion status").success);
    }

    #[test]
    fn pending_frame_is_drawn_before_executing_url_import_request() {
        let mut state = AppState::new(SkillInventory {
            skills: Vec::new(),
            source_repositories: Vec::new(),
        });
        state.reduce(AppAction::BeginImportPrompt(
            crate::tui::AppImportSource::Url,
        ));
        state.reduce(AppAction::PromptChanged(
            "https://example.test/skill.md".to_string(),
        ));
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let pending_drawn = Rc::new(Cell::new(false));
        let request_executed = Rc::new(Cell::new(false));

        handle_action_with_pending_observer(
            &mut terminal,
            &mut state,
            AppAction::SubmitPrompt,
            {
                let pending_drawn = Rc::clone(&pending_drawn);
                let request_executed = Rc::clone(&request_executed);
                move |request| {
                    assert!(
                        pending_drawn.get(),
                        "pending frame should be drawn before URL import starts"
                    );
                    request_executed.set(true);
                    assert_eq!(
                        request,
                        AppOperationRequest::ImportUrl {
                            url: "https://example.test/skill.md".to_string(),
                        }
                    );
                    Err("network skipped".to_string())
                }
            },
            {
                let pending_drawn = Rc::clone(&pending_drawn);
                move || pending_drawn.set(true)
            },
        )
        .expect("handle action");

        assert!(request_executed.get());
        let rendered = terminal_text(&terminal);
        assert!(rendered.contains("Status: pending import url"));
        assert_eq!(
            state.status_view().expect("failure status").message,
            "failed: network skipped"
        );
    }

    struct UnusedFetcher;

    impl SkillUrlFetcher for UnusedFetcher {
        fn fetch_skill_markdown(&self, _url: &str) -> Result<String, SkillUrlFetchError> {
            panic!("repository test should not fetch URLs")
        }
    }

    struct UnusedAnalyzer;

    impl SkillAnalyzerLauncher for UnusedAnalyzer {
        fn launch(&self, _request: AnalyzeSkillRequest) -> Result<AnalyzeLaunchResult, String> {
            panic!("test should not launch analyzer")
        }
    }

    struct RecordingAnalyzer {
        result: AnalyzeLaunchResult,
        requests: RefCell<Vec<AnalyzeSkillRequest>>,
    }

    impl SkillAnalyzerLauncher for RecordingAnalyzer {
        fn launch(&self, request: AnalyzeSkillRequest) -> Result<AnalyzeLaunchResult, String> {
            self.requests.borrow_mut().push(request);
            Ok(self.result.clone())
        }
    }

    struct StaticRepositoryProvider {
        repository_path: PathBuf,
    }

    impl SkillRepositoryProvider for StaticRepositoryProvider {
        type Checkout = StaticRepositoryCheckout;

        fn fetch_repository(
            &self,
            _repository: &str,
        ) -> Result<Self::Checkout, SkillRepositoryFetchError> {
            Ok(StaticRepositoryCheckout {
                path: self.repository_path.clone(),
            })
        }
    }

    struct StaticRepositoryCheckout {
        path: PathBuf,
    }

    impl SkillRepositoryCheckout for StaticRepositoryCheckout {
        fn path(&self) -> &Path {
            &self.path
        }
    }

    fn roots(base: &Path) -> DiscoveryRoots {
        DiscoveryRoots {
            canonical_root: base.join("canonical"),
            imports_root: base.join("imports"),
            claude_code_root: base.join("claude"),
            codex_root: base.join("codex"),
        }
    }

    fn write_skill(root: &Path, name: &str, description: &str) {
        let skill_path = root.join(name);
        fs::create_dir_all(&skill_path).expect("skill dir");
        fs::write(
            skill_path.join("SKILL.md"),
            format!(
                r#"---
name: {name}
description: {description}
---

# {name}
"#
            ),
        )
        .expect("skill file");
    }

    fn write_import_manifest(skill_path: &Path, promoted: bool) {
        fs::write(
            skill_path.join("import.json"),
            format!(
                r#"{{
  "source_type": "markdown",
  "source_location": null,
  "imported_at": 1,
  "content_hash": "sha256:test",
  "promoted": {promoted}
}}"#
            ),
        )
        .expect("import manifest");
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
            analysis_skill_dir: None,
        }
    }

    fn terminal_text(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
    }
}
