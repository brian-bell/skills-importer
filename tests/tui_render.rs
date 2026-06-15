use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier},
    widgets::{Block, Borders},
};
use skill_importer::{
    AgentEnablement, AgentEntries, AgentEntryStatus, RepositorySkillCandidate, SkillAgent,
    SkillEntry, SkillInventory, SkillSource,
    tui::{
        AppAction, AppImportSource, AppOperationResult, AppState, ConfirmationOperation,
        SelectionDelta, render_app,
    },
};

#[test]
fn main_screen_renders_user_visible_sections() {
    let mut state = AppState::new(inventory(vec![
        skill("alpha", "First skill", SkillSource::Canonical),
        skill("beta", "Second skill", SkillSource::Imported),
    ]));
    state.reduce(AppAction::MoveSelection(SelectionDelta::Next));
    state.reduce(AppAction::SwitchTarget(SkillAgent::ClaudeCode));
    state.reduce(AppAction::OperationFinished(AppOperationResult::success(
        "enable",
        Some("beta".to_string()),
        2,
    )));

    let text = render_text(&state, 140, 24);

    for expected in [
        "Skill Importer TUI",
        "Skill list",
        "Selected detail",
        "Active target: Claude Code",
        "Filter: (none) | Source: all | Active target: Claude Code",
        "Keyboard hints",
        "i toggle source: all",
        "Status: enable (beta) - success: 2 actions",
        "beta",
        "Source:",
        "Enablement:",
    ] {
        assert!(text.contains(expected), "missing `{expected}` in:\n{text}");
    }
}

#[test]
fn promoted_skill_row_marker_and_name_render_yellow_when_unselected() {
    let mut promoted = skill("promoted", "Promoted skill", SkillSource::Canonical);
    promoted.promoted = true;
    let state = AppState::new(inventory(vec![
        skill("alpha", "First skill", SkillSource::Canonical),
        promoted,
    ]));

    let buffer = render_buffer(&state, 90, 24);

    assert_row_fg(&buffer, "  promoted", Color::Yellow);
    assert_skill_list_text_dimmed(&buffer, "  promoted", true);
    assert_row_fg(&buffer, "> alpha", Color::Reset);
}

#[test]
fn promoted_skill_row_marker_and_name_render_yellow_when_selected() {
    let mut promoted = skill("promoted", "Promoted skill", SkillSource::Canonical);
    promoted.promoted = true;
    let mut state = AppState::new(inventory(vec![
        skill("alpha", "First skill", SkillSource::Canonical),
        promoted,
    ]));
    state.reduce(AppAction::MoveSelection(SelectionDelta::Next));

    let buffer = render_buffer(&state, 90, 24);

    assert_row_fg(&buffer, "> promoted", Color::Yellow);
}

#[test]
fn header_shows_imported_source_filter_when_active() {
    let mut state = AppState::new(inventory(vec![
        skill("alpha", "First skill", SkillSource::Canonical),
        skill("beta", "Second skill", SkillSource::Imported),
    ]));
    state.reduce(AppAction::ToggleSourceFilter);

    let text = render_text(&state, 90, 24);

    assert!(
        text.contains("Filter: (none) | Source: imported | Active target: Codex"),
        "missing source scope in:\n{text}"
    );
    assert!(text.contains("beta"), "missing imported row in:\n{text}");
    assert!(
        text.contains("i toggle source: imported"),
        "missing source hint in:\n{text}"
    );
    assert!(
        !text.contains("> alpha") && !text.contains("  alpha"),
        "canonical row should be filtered from list:\n{text}"
    );
}

#[test]
fn selected_detail_renders_repository_source_metadata_when_present() {
    let mut imported = skill("repo-alpha", "Repository skill", SkillSource::Imported);
    imported.source_repository = Some(skill_importer::ImportSourceRepository {
        repository: "https://example.test/skills.git".to_string(),
        skill_path: "skills/repo-alpha".to_string(),
    });
    let state = AppState::new(inventory(vec![imported]));

    let text = render_text(&state, 120, 24);

    for expected in [
        "Repository: https://example.test/skills.git",
        "Repository path: skills/repo-alpha",
    ] {
        assert!(text.contains(expected), "missing `{expected}` in:\n{text}");
    }
}

#[test]
fn repository_selection_render_shows_candidates_and_confirm_cancel_hints() {
    let mut state = AppState::new(inventory(Vec::new()));
    state.reduce(AppAction::RepositorySelectionLoaded(
        skill_importer::RepositorySkillSelection {
            repository: "https://example.test/repo.git".to_string(),
            skills: vec![
                candidate("repo-alpha", "First repo", "skills/repo-alpha"),
                candidate("repo-beta", "Second repo", "skills/repo-beta"),
            ],
        },
    ));
    state.reduce(AppAction::ToggleRepositoryCandidate);

    let buffer = render_buffer(&state, 90, 20);
    let text = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    for expected in [
        "Repository selection",
        "> [x] repo-alpha",
        "  [ ] repo-beta",
        "repo-alpha",
        "skills/repo-alpha",
        "repo-beta",
        "space select",
        "enter import",
        "esc cancel",
    ] {
        assert!(text.contains(expected), "missing `{expected}` in:\n{text}");
    }
    assert_repository_selection_text_dimmed(&buffer, "> [x] repo-alpha", false);
    assert_repository_selection_text_dimmed(&buffer, "  [ ] repo-beta", false);
}

#[test]
fn success_and_failure_status_states_are_visible_without_stale_text() {
    let mut state = AppState::new(inventory(vec![skill(
        "alpha",
        "First",
        SkillSource::Canonical,
    )]));
    state.reduce(AppAction::OperationFinished(AppOperationResult::success(
        "enable",
        Some("alpha".to_string()),
        3,
    )));
    assert!(render_text(&state, 80, 20).contains("success: 3 actions"));

    state.reduce(AppAction::OperationFinished(AppOperationResult::failure(
        "disable",
        Some("alpha".to_string()),
        "unsafe entry",
    )));
    let text = render_text(&state, 80, 20);
    assert!(text.contains("failed: unsafe entry"));
    assert!(!text.contains("success: 3 actions"));
}

#[test]
fn pending_request_status_uses_human_label_instead_of_debug_struct() {
    let mut state = AppState::new(inventory(vec![skill(
        "alpha",
        "First",
        SkillSource::Canonical,
    )]));
    state.reduce(AppAction::RequestEnableSelected);

    let text = render_text(&state, 80, 20);

    assert!(text.contains("Status: pending enable (alpha)"));
    assert!(!text.contains("EnableSkill"));

    let mut import_state = AppState::new(inventory(Vec::new()));
    import_state.reduce(AppAction::BeginImportPrompt(AppImportSource::Url));
    import_state.reduce(AppAction::PromptChanged(
        "https://example.test/skill.md".to_string(),
    ));
    import_state.reduce(AppAction::SubmitPrompt);
    let import_text = render_text(&import_state, 80, 20);
    assert!(import_text.contains("Status: pending import url"));
    assert!(!import_text.contains("ImportUrl"));

    let mut delete_state = AppState::new(inventory(vec![skill(
        "alpha",
        "First",
        SkillSource::Imported,
    )]));
    delete_state.reduce(AppAction::BeginConfirmation(ConfirmationOperation::Delete));
    delete_state.reduce(AppAction::ConfirmPending);
    let delete_text = render_text(&delete_state, 80, 20);
    assert!(delete_text.contains("Status: pending delete (alpha)"));
    assert!(!delete_text.contains("DeleteImport"));
}

#[test]
fn pending_request_status_takes_precedence_over_previous_result() {
    let mut state = AppState::new(inventory(vec![skill(
        "alpha",
        "First",
        SkillSource::Canonical,
    )]));
    state.reduce(AppAction::OperationFinished(AppOperationResult::success(
        "disable",
        Some("alpha".to_string()),
        1,
    )));
    state.reduce(AppAction::RequestEnableSelected);

    let text = render_text(&state, 80, 20);

    assert!(text.contains("Status: pending enable (alpha)"));
    assert!(!text.contains("Status: disable (alpha) - success: 1 actions"));
}

#[test]
fn failure_status_clears_on_next_ui_action() {
    let mut state = AppState::new(inventory(vec![
        skill("alpha", "First", SkillSource::Canonical),
        skill("beta", "Second", SkillSource::Canonical),
    ]));
    state.reduce(AppAction::OperationFinished(AppOperationResult::failure(
        "promote",
        Some("alpha".to_string()),
        "unsafe entry",
    )));
    assert!(render_text(&state, 80, 20).contains("failed: unsafe entry"));

    state.reduce(AppAction::MoveSelection(SelectionDelta::Next));

    let text = render_text(&state, 80, 20);
    assert!(text.contains("Status: ready"));
    assert!(!text.contains("unsafe entry"));
}

#[test]
fn import_prompt_replaces_previous_failure_in_input_area() {
    let mut state = AppState::new(inventory(Vec::new()));
    state.reduce(AppAction::BeginImportPrompt(AppImportSource::Url));
    state.reduce(AppAction::PromptChanged(
        "https://example.test/missing.md".to_string(),
    ));
    state.reduce(AppAction::SubmitPrompt);
    state.reduce(AppAction::CompletePendingOperation(Err(
        "HTTP 404".to_string()
    )));
    assert!(render_text(&state, 80, 20).contains("HTTP 404"));

    state.reduce(AppAction::DeletePromptChar);
    state.reduce(AppAction::PromptChanged(
        "https://example.test/skill.md".to_string(),
    ));

    let text = render_text(&state, 80, 20);
    assert!(text.contains("https://example.test/skill.md"));
    assert!(!text.contains("HTTP 404"));
}

#[test]
fn constrained_terminal_render_does_not_panic_and_preserves_essential_labels() {
    let state = AppState::new(inventory(vec![skill(
        "alpha",
        "First",
        SkillSource::Canonical,
    )]));

    let text = render_text(&state, 42, 9);

    assert!(text.contains("Skill"));
    assert!(text.contains("Status"));
}

#[test]
fn disabled_skill_rows_are_dimmed_in_skill_list() {
    let disabled_selected = AppState::new(inventory(vec![
        skill_with_enablement("disabled", AgentEnablement::Neither),
        skill_with_enablement("claude", AgentEnablement::ClaudeCode),
        skill_with_enablement("enabled", AgentEnablement::Codex),
        skill_with_enablement("both", AgentEnablement::Both),
    ]));
    let buffer = render_buffer(&disabled_selected, 90, 24);
    assert_skill_list_text_dimmed(&buffer, "> disabled", true);
    assert_skill_list_text_dimmed(&buffer, "  claude", false);
    assert_skill_list_text_dimmed(&buffer, "  enabled", false);
    assert_skill_list_text_dimmed(&buffer, "  both", false);

    let mut enabled_selected = AppState::new(inventory(vec![
        skill_with_enablement("disabled", AgentEnablement::Neither),
        skill_with_enablement("claude", AgentEnablement::ClaudeCode),
        skill_with_enablement("enabled", AgentEnablement::Codex),
        skill_with_enablement("both", AgentEnablement::Both),
    ]));
    enabled_selected.reduce(AppAction::MoveSelection(SelectionDelta::Next));
    let buffer = render_buffer(&enabled_selected, 90, 24);
    assert_skill_list_text_dimmed(&buffer, "  disabled", true);
    assert_skill_list_text_dimmed(&buffer, "> claude", false);
}

#[test]
fn selected_skill_scrolls_into_view_in_long_skill_list() {
    let mut state = AppState::new(inventory(
        (0..20)
            .map(|index| {
                skill(
                    &format!("skill-{index:02}"),
                    "Skill",
                    SkillSource::Canonical,
                )
            })
            .collect(),
    ));
    for _ in 0..12 {
        state.reduce(AppAction::MoveSelection(SelectionDelta::Next));
    }

    let buffer = render_buffer(&state, 100, 18);
    let list_text = text_in_area(&buffer, skill_list_inner_area(*buffer.area()));

    assert!(
        list_text.contains("> skill-12"),
        "selected row should be visible in skill list:\n{list_text}"
    );
}

fn render_text(state: &AppState, width: u16, height: u16) -> String {
    render_buffer(state, width, height)
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>()
}

fn render_buffer(state: &AppState, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| render_app(frame, state))
        .expect("draw");
    terminal.backend().buffer().clone()
}

fn assert_row_fg(buffer: &Buffer, row_text: &str, expected_fg: Color) {
    let (x, y) = find_row_text(buffer, row_text);
    for offset in 0..row_text.chars().count() as u16 {
        let cell = &buffer[(x + offset, y)];
        assert_eq!(
            cell.fg, expected_fg,
            "foreground mismatch for `{row_text}` at offset {offset}"
        );
    }
}

fn find_row_text(buffer: &Buffer, row_text: &str) -> (u16, u16) {
    for y in 0..buffer.area.height {
        let line = (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect::<String>();
        if let Some(byte_start) = line.find(row_text) {
            let x = line[..byte_start].chars().count() as u16;
            return (x, y);
        }
    }

    let text = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    panic!("missing `{row_text}` in:\n{text}");
}

fn assert_skill_list_text_dimmed(buffer: &Buffer, expected: &str, dimmed: bool) {
    let area = skill_list_inner_area(*buffer.area());
    assert_text_dimmed_in_area(buffer, area, expected, dimmed);
}

fn assert_repository_selection_text_dimmed(buffer: &Buffer, expected: &str, dimmed: bool) {
    let area = repository_selection_inner_area(*buffer.area());
    assert_text_dimmed_in_area(buffer, area, expected, dimmed);
}

fn assert_text_dimmed_in_area(buffer: &Buffer, area: Rect, expected: &str, dimmed: bool) {
    let expected_width = expected.len() as u16;

    for y in area.y..area.y + area.height {
        let row = text_row_in_area(buffer, area, y);
        if let Some(offset) = row.find(expected) {
            let start = area.x + offset as u16;
            for x in start..start + expected_width {
                assert_eq!(
                    buffer[(x, y)].modifier.contains(Modifier::DIM),
                    dimmed,
                    "unexpected DIM style for `{expected}` at ({x}, {y})"
                );
            }
            return;
        }
    }

    panic!("missing `{expected}` in scoped render area");
}

fn text_in_area(buffer: &Buffer, area: Rect) -> String {
    (area.y..area.y + area.height)
        .map(|y| text_row_in_area(buffer, area, y))
        .collect::<Vec<_>>()
        .join("\n")
}

fn text_row_in_area(buffer: &Buffer, area: Rect, y: u16) -> String {
    (area.x..area.x + area.width)
        .map(|x| buffer[(x, y)].symbol())
        .collect()
}

fn skill_list_inner_area(area: Rect) -> Rect {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(area);
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(rows[1]);

    Block::default().borders(Borders::ALL).inner(columns[0])
}

fn repository_selection_inner_area(area: Rect) -> Rect {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(area);

    Block::default().borders(Borders::ALL).inner(rows[1])
}

fn inventory(skills: Vec<SkillEntry>) -> SkillInventory {
    SkillInventory {
        skills,
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
        analysis_skill_dir: None,
    }
}

fn skill_with_enablement(name: &str, enablement: AgentEnablement) -> SkillEntry {
    let mut entry = skill(name, "Skill", SkillSource::Canonical);
    entry.enablement = enablement;
    entry
}

fn candidate(name: &str, description: &str, relative_path: &str) -> RepositorySkillCandidate {
    RepositorySkillCandidate {
        name: name.to_string(),
        description: Some(description.to_string()),
        relative_path: relative_path.to_string(),
    }
}
