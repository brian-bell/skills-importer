use std::env;
use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

mod cli;

use cli::{Command, RootDefaults};
use skill_importer::{
    DeleteImportRequest, DiscoveryRoots, ImportLocalPathRequest, ImportMarkdownRequest,
    ImportUrlRequest, PromoteSkillRequest, SkillRepositoryCheckout, SkillRepositoryFetchError,
    SkillRepositoryProvider, SkillUrlFetchError, SkillUrlFetcher, analyzer, json_adapter,
    tui::run_tui, workflow,
};

const MAX_SKILL_MARKDOWN_BYTES: u64 = 1024 * 1024;

fn main() -> ExitCode {
    match run(env::args_os().skip(1), io::stdout()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("skill-importer: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: impl IntoIterator<Item = OsString>, mut stdout: impl Write) -> Result<(), String> {
    run_with_services(args, &mut stdout, &UreqUrlFetcher, &DefaultTuiRunner)
}

#[cfg(test)]
fn run_with_url_fetcher(
    args: impl IntoIterator<Item = OsString>,
    mut stdout: impl Write,
    url_fetcher: &impl SkillUrlFetcher,
) -> Result<(), String> {
    run_with_services(args, &mut stdout, url_fetcher, &DisabledTuiRunner)
}

fn run_with_services(
    args: impl IntoIterator<Item = OsString>,
    mut stdout: impl Write,
    url_fetcher: &impl SkillUrlFetcher,
    tui_runner: &impl TuiRunner,
) -> Result<(), String> {
    let defaults = RootDefaults::current_process()?;
    run_with_services_with_defaults(args, &mut stdout, url_fetcher, tui_runner, &defaults)
}

fn run_with_services_with_defaults(
    args: impl IntoIterator<Item = OsString>,
    mut stdout: impl Write,
    url_fetcher: &impl SkillUrlFetcher,
    tui_runner: &impl TuiRunner,
    defaults: &RootDefaults,
) -> Result<(), String> {
    let command = cli::parse_command(args, defaults)?;
    let repository_provider = UnavailableRepositoryProvider;

    match command {
        Command::Help { message } => stdout
            .write_all(message.as_bytes())
            .map_err(|error| format!("failed to write help: {error}")),
        Command::List { roots } => {
            let outcome = workflow::execute(
                &roots,
                workflow::OperationRequest::List,
                url_fetcher,
                &repository_provider,
            )
            .map_err(|error| format!("failed to discover skills: {error}"))?;
            write_json_outcome(&mut stdout, &outcome)
        }
        Command::ImportMarkdown {
            roots,
            source_location,
        } => {
            let mut markdown = String::new();
            io::stdin()
                .read_to_string(&mut markdown)
                .map_err(|error| format!("failed to read Markdown from stdin: {error}"))?;
            let outcome = workflow::execute(
                &roots,
                workflow::OperationRequest::ImportMarkdown(ImportMarkdownRequest {
                    markdown: &markdown,
                    source_location: source_location.as_deref(),
                }),
                url_fetcher,
                &repository_provider,
            )
            .map_err(|error| format!("failed to import Markdown: {error}"))?;
            write_json_outcome(&mut stdout, &outcome)
        }
        Command::ImportPath { roots, path } => {
            let outcome = workflow::execute(
                &roots,
                workflow::OperationRequest::ImportLocalPath(ImportLocalPathRequest {
                    path: path.as_path(),
                }),
                url_fetcher,
                &repository_provider,
            )
            .map_err(|error| format!("failed to import path: {error}"))?;
            write_json_outcome(&mut stdout, &outcome)
        }
        Command::ImportUrl { roots, url } => {
            let outcome = workflow::execute(
                &roots,
                workflow::OperationRequest::ImportUrl(ImportUrlRequest { url: url.as_str() }),
                url_fetcher,
                &repository_provider,
            )
            .map_err(|error| format!("failed to import URL: {error}"))?;
            write_json_outcome(&mut stdout, &outcome)
        }
        Command::Enable {
            roots,
            skill_name,
            agents,
        } => {
            let outcome = workflow::execute(
                &roots,
                workflow::OperationRequest::Enable {
                    skill_name: skill_name.as_str(),
                    agents: &agents,
                },
                url_fetcher,
                &repository_provider,
            )
            .map_err(|error| format!("failed to enable skill: {error}"))?;
            write_json_outcome(&mut stdout, &outcome)
        }
        Command::Disable {
            roots,
            skill_name,
            agents,
        } => {
            let outcome = workflow::execute(
                &roots,
                workflow::OperationRequest::Disable {
                    skill_name: skill_name.as_str(),
                    agents: &agents,
                },
                url_fetcher,
                &repository_provider,
            )
            .map_err(|error| format!("failed to disable skill: {error}"))?;
            write_json_outcome(&mut stdout, &outcome)
        }
        Command::Promote { roots, skill_name } => {
            let outcome = workflow::execute(
                &roots,
                workflow::OperationRequest::Promote(PromoteSkillRequest {
                    skill_name: skill_name.as_str(),
                }),
                url_fetcher,
                &repository_provider,
            )
            .map_err(|error| format!("failed to promote skill: {error}"))?;
            write_json_outcome(&mut stdout, &outcome)
        }
        Command::Delete { roots, skill_name } => {
            let outcome = workflow::execute(
                &roots,
                workflow::OperationRequest::Delete(DeleteImportRequest {
                    skill_name: skill_name.as_str(),
                }),
                url_fetcher,
                &repository_provider,
            )
            .map_err(|error| format!("failed to delete import: {error}"))?;
            write_json_outcome(&mut stdout, &outcome)
        }
        Command::RenderAnalysisReport { input, output } => {
            analyzer::render_analysis_report_file(&input, &output)
        }
        Command::Tui { roots } => tui_runner.run(&roots),
    }
}

trait TuiRunner {
    fn run(&self, roots: &DiscoveryRoots) -> Result<(), String>;
}

struct DefaultTuiRunner;

impl TuiRunner for DefaultTuiRunner {
    fn run(&self, roots: &DiscoveryRoots) -> Result<(), String> {
        run_tui(roots, &UreqUrlFetcher).map_err(|error| format!("failed to run TUI: {error}"))
    }
}

#[cfg(test)]
struct DisabledTuiRunner;

#[cfg(test)]
impl TuiRunner for DisabledTuiRunner {
    fn run(&self, _roots: &DiscoveryRoots) -> Result<(), String> {
        Err("TUI runner was not configured".to_string())
    }
}

fn write_json_outcome(
    stdout: &mut impl Write,
    outcome: &workflow::OperationOutcome,
) -> Result<(), String> {
    json_adapter::write_outcome(stdout, outcome)
        .map_err(|error| format!("failed to write JSON: {error}"))
}

struct UreqUrlFetcher;

impl SkillUrlFetcher for UreqUrlFetcher {
    fn fetch_skill_markdown(&self, url: &str) -> Result<String, SkillUrlFetchError> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
            .into();
        let response: ureq::http::Response<ureq::Body> =
            agent.get(url).call().map_err(|error| SkillUrlFetchError {
                message: error.to_string(),
            })?;
        read_limited_skill_markdown(response.into_body().into_reader())
    }
}

struct UnavailableRepositoryProvider;

impl SkillRepositoryProvider for UnavailableRepositoryProvider {
    type Checkout = UnavailableRepositoryCheckout;

    fn fetch_repository(
        &self,
        repository: &str,
    ) -> Result<Self::Checkout, SkillRepositoryFetchError> {
        Err(SkillRepositoryFetchError {
            message: format!("repository import is not available for `{repository}`"),
        })
    }
}

struct UnavailableRepositoryCheckout;

impl SkillRepositoryCheckout for UnavailableRepositoryCheckout {
    fn path(&self) -> &Path {
        Path::new("")
    }
}

fn read_limited_skill_markdown(reader: impl Read) -> Result<String, SkillUrlFetchError> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_SKILL_MARKDOWN_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| SkillUrlFetchError {
            message: error.to_string(),
        })?;

    if bytes.len() as u64 > MAX_SKILL_MARKDOWN_BYTES {
        return Err(SkillUrlFetchError {
            message: format!(
                "skill Markdown response exceeds the {} byte limit",
                MAX_SKILL_MARKDOWN_BYTES
            ),
        });
    }

    String::from_utf8(bytes).map_err(|error| SkillUrlFetchError {
        message: format!("skill Markdown response is not valid UTF-8: {error}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use skill_importer::{SkillUrlFetchError, SkillUrlFetcher};
    use std::cell::RefCell;

    #[test]
    fn import_url_command_fetches_with_injected_fetcher_and_outputs_action_json() {
        let temp = tempfile::tempdir().expect("tempdir");
        let canonical_root = temp.path().join("canonical");
        let imports_root = temp.path().join("imports");
        let mut stdout = Vec::new();
        let fetcher = StaticFetcher {
            markdown: r#"---
name: command-url-import
description: Imported from a URL through the command.
---

# Command URL Import
"#,
        };

        run_with_url_fetcher(
            [
                OsString::from("import"),
                OsString::from("url"),
                OsString::from("--json"),
                OsString::from("--url"),
                OsString::from("https://example.test/command-url-import.md"),
                OsString::from("--canonical-root"),
                canonical_root.clone().into_os_string(),
                OsString::from("--imports-root"),
                imports_root.clone().into_os_string(),
            ],
            &mut stdout,
            &fetcher,
        )
        .expect("command succeeds");

        let json: serde_json::Value = serde_json::from_slice(&stdout).expect("valid json output");
        assert_eq!(json["skill_name"], "command-url-import");
        assert_eq!(json["manifest"]["source_type"], "url");
        assert_eq!(
            json["manifest"]["source_location"],
            "https://example.test/command-url-import.md"
        );
        assert!(
            imports_root
                .join("command-url-import")
                .join("SKILL.md")
                .exists()
        );
    }

    #[test]
    fn import_url_command_reports_fetch_failures_without_partial_storage() {
        let temp = tempfile::tempdir().expect("tempdir");
        let canonical_root = temp.path().join("canonical");
        let imports_root = temp.path().join("imports");
        let mut stdout = Vec::new();

        let error = run_with_url_fetcher(
            [
                OsString::from("import"),
                OsString::from("url"),
                OsString::from("--json"),
                OsString::from("--url"),
                OsString::from("https://example.test/missing.md"),
                OsString::from("--canonical-root"),
                canonical_root.into_os_string(),
                OsString::from("--imports-root"),
                imports_root.clone().into_os_string(),
            ],
            &mut stdout,
            &FailingFetcher,
        )
        .expect_err("command fails");

        assert!(
            error.contains("failed to import URL"),
            "error should name the failing operation: {error}"
        );
        assert!(
            error.contains("https://example.test/missing.md"),
            "error should include the URL: {error}"
        );
        assert!(
            error.contains("HTTP 404"),
            "error should include the fetch failure: {error}"
        );
        assert!(
            !imports_root.exists(),
            "failed URL command should not create storage"
        );
    }

    #[test]
    fn tui_command_routes_to_injected_runner_and_bare_command_remains_usage() {
        let temp = tempfile::tempdir().expect("tempdir");
        let canonical_root = temp.path().join("canonical");
        let imports_root = temp.path().join("imports");
        let claude_root = temp.path().join("claude");
        let codex_root = temp.path().join("codex");
        let runner = RecordingTuiRunner::default();
        let mut stdout = Vec::new();

        run_with_services(
            [
                OsString::from("tui"),
                OsString::from("--canonical-root"),
                canonical_root.clone().into_os_string(),
                OsString::from("--imports-root"),
                imports_root.clone().into_os_string(),
                OsString::from("--claude-code-root"),
                claude_root.clone().into_os_string(),
                OsString::from("--codex-root"),
                codex_root.clone().into_os_string(),
            ],
            &mut stdout,
            &StaticFetcher { markdown: "" },
            &runner,
        )
        .expect("tui command routes to runner");

        assert_eq!(
            runner.roots.borrow().as_ref(),
            Some(&DiscoveryRoots {
                canonical_root,
                imports_root,
                claude_code_root: claude_root,
                codex_root,
            })
        );
        assert!(stdout.is_empty(), "tui runner should own terminal output");

        let error = run_with_services(
            Vec::<OsString>::new(),
            &mut Vec::new(),
            &StaticFetcher { markdown: "" },
            &runner,
        )
        .expect_err("bare command still reports usage");
        assert!(error.contains("Usage: skill-importer"));
        assert!(error.contains("Commands:"));
        assert_eq!(
            runner.calls(),
            1,
            "bare command must not launch the TUI runner"
        );
    }

    #[test]
    fn tui_command_without_root_overrides_uses_user_level_agent_roots() {
        let temp = tempfile::tempdir().expect("tempdir");
        let catalog_repo = temp.path().join("skills-repo");
        let catalog_root = catalog_repo.join("catalog").join("portable");
        let home = temp.path().join("home");
        std::fs::create_dir_all(&catalog_repo).expect("catalog repo");
        std::fs::write(catalog_repo.join("AGENTS.md"), "# Test catalog\n").expect("agents");
        std::fs::create_dir_all(&catalog_root).expect("catalog root");
        let runner = RecordingTuiRunner::default();
        let mut stdout = Vec::new();

        run_with_services_with_defaults(
            [OsString::from("tui")],
            &mut stdout,
            &StaticFetcher { markdown: "" },
            &runner,
            &RootDefaults {
                current_dir: catalog_repo.clone(),
                home: Some(home.clone().into_os_string()),
            },
        )
        .expect("tui command routes to runner");

        assert_eq!(
            runner.roots.borrow().as_ref(),
            Some(&DiscoveryRoots {
                canonical_root: catalog_root,
                imports_root: catalog_repo.join(".skill-importer").join("imports"),
                claude_code_root: home.join(".claude").join("skills"),
                codex_root: home.join(".agents").join("skills"),
            })
        );
        assert!(stdout.is_empty(), "tui runner should own terminal output");
    }

    #[test]
    fn root_overrides_do_not_require_home() {
        let temp = tempfile::tempdir().expect("tempdir");
        let canonical_root = temp.path().join("canonical");
        let imports_root = temp.path().join("imports");
        let claude_root = temp.path().join("claude");
        let codex_root = temp.path().join("codex");

        for home in [None, Some(OsString::from("relative-home"))] {
            let mut stdout = Vec::new();
            run_with_services_with_defaults(
                [
                    OsString::from("list"),
                    OsString::from("--json"),
                    OsString::from("--canonical-root"),
                    canonical_root.clone().into_os_string(),
                    OsString::from("--imports-root"),
                    imports_root.clone().into_os_string(),
                    OsString::from("--claude-code-root"),
                    claude_root.clone().into_os_string(),
                    OsString::from("--codex-root"),
                    codex_root.clone().into_os_string(),
                ],
                &mut stdout,
                &StaticFetcher { markdown: "" },
                &DisabledTuiRunner,
                &RootDefaults {
                    current_dir: temp.path().to_path_buf(),
                    home,
                },
            )
            .expect("all root overrides should not require HOME");

            let json: serde_json::Value =
                serde_json::from_slice(&stdout).expect("valid list json output");
            assert_eq!(json["skills"].as_array().expect("skills array").len(), 0);
        }
    }

    #[test]
    fn help_command_writes_to_stdout_without_running_tui() {
        let runner = RecordingTuiRunner::default();
        let mut stdout = Vec::new();

        run_with_services(
            [OsString::from("--help")],
            &mut stdout,
            &StaticFetcher { markdown: "" },
            &runner,
        )
        .expect("help succeeds");

        let help = String::from_utf8(stdout).expect("help output is utf8");
        assert!(help.contains("Usage: skill-importer"));
        assert!(help.contains("Commands:"));
        assert_eq!(runner.calls(), 0, "help must not launch the TUI runner");
    }

    #[derive(Default)]
    struct RecordingTuiRunner {
        roots: RefCell<Option<DiscoveryRoots>>,
        calls: RefCell<usize>,
    }

    impl RecordingTuiRunner {
        fn calls(&self) -> usize {
            *self.calls.borrow()
        }
    }

    impl TuiRunner for RecordingTuiRunner {
        fn run(&self, roots: &DiscoveryRoots) -> Result<(), String> {
            *self.calls.borrow_mut() += 1;
            *self.roots.borrow_mut() = Some(roots.clone());
            Ok(())
        }
    }

    struct StaticFetcher {
        markdown: &'static str,
    }

    impl SkillUrlFetcher for StaticFetcher {
        fn fetch_skill_markdown(&self, _url: &str) -> Result<String, SkillUrlFetchError> {
            Ok(self.markdown.to_string())
        }
    }

    struct FailingFetcher;

    impl SkillUrlFetcher for FailingFetcher {
        fn fetch_skill_markdown(&self, _url: &str) -> Result<String, SkillUrlFetchError> {
            Err(SkillUrlFetchError {
                message: "HTTP 404".to_string(),
            })
        }
    }
}
