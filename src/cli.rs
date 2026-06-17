use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand};
use skill_importer::{DiscoveryRoots, SkillAgent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Command {
    Help {
        message: String,
    },
    List {
        roots: DiscoveryRoots,
    },
    ImportMarkdown {
        roots: DiscoveryRoots,
        source_location: Option<String>,
    },
    ImportPath {
        roots: DiscoveryRoots,
        path: PathBuf,
    },
    ImportUrl {
        roots: DiscoveryRoots,
        url: String,
    },
    Enable {
        roots: DiscoveryRoots,
        skill_name: String,
        agents: Vec<SkillAgent>,
    },
    Disable {
        roots: DiscoveryRoots,
        skill_name: String,
        agents: Vec<SkillAgent>,
    },
    Promote {
        roots: DiscoveryRoots,
        skill_name: String,
    },
    Delete {
        roots: DiscoveryRoots,
        skill_name: String,
    },
    RenderAnalysisReport {
        input: PathBuf,
        output: PathBuf,
    },
    Tui {
        roots: DiscoveryRoots,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RootDefaults {
    pub(crate) home: Option<OsString>,
}

impl RootDefaults {
    pub(crate) fn current_process() -> Self {
        Self {
            home: env::var_os("HOME"),
        }
    }
}

pub(crate) fn parse_command(
    args: impl IntoIterator<Item = OsString>,
    defaults: &RootDefaults,
) -> Result<Command, String> {
    let args = std::iter::once(OsString::from("skill-importer")).chain(args);
    let parsed = match CliArgs::try_parse_from(args) {
        Ok(parsed) => parsed,
        Err(error) if error.kind() == clap::error::ErrorKind::DisplayHelp => {
            return Ok(Command::Help {
                message: error.to_string(),
            });
        }
        Err(error) => return Err(error.to_string()),
    };

    parsed.command.into_command(defaults)
}

#[derive(Debug, Parser)]
#[command(name = "skill-importer", color = clap::ColorChoice::Never)]
struct CliArgs {
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    List(JsonRootArgs),
    Import {
        #[command(subcommand)]
        kind: ImportCommand,
    },
    Enable(SkillAgentsArgs),
    Disable(SkillAgentsArgs),
    Promote(SkillNameArgs),
    Delete(SkillNameArgs),
    #[command(hide = true)]
    RenderAnalysisReport(RenderAnalysisReportArgs),
    Tui(RootArgs),
}

impl CliCommand {
    fn into_command(self, defaults: &RootDefaults) -> Result<Command, String> {
        match self {
            Self::List(args) => {
                args.require_json("list")?;
                Ok(Command::List {
                    roots: args.roots.into_discovery_roots(defaults)?,
                })
            }
            Self::Import { kind } => kind.into_command(defaults),
            Self::Enable(args) => args.into_command(defaults, EnableDisableCommand::Enable),
            Self::Disable(args) => args.into_command(defaults, EnableDisableCommand::Disable),
            Self::Promote(args) => args.into_command(defaults, SkillCommand::Promote),
            Self::Delete(args) => args.into_command(defaults, SkillCommand::Delete),
            Self::RenderAnalysisReport(args) => args.into_command(),
            Self::Tui(args) => Ok(Command::Tui {
                roots: args.into_discovery_roots(defaults)?,
            }),
        }
    }
}

#[derive(Debug, Subcommand)]
enum ImportCommand {
    Markdown(ImportMarkdownArgs),
    Path(ImportPathArgs),
    Url(ImportUrlArgs),
}

impl ImportCommand {
    fn into_command(self, defaults: &RootDefaults) -> Result<Command, String> {
        match self {
            Self::Markdown(args) => {
                args.json_roots.require_json("import markdown")?;
                Ok(Command::ImportMarkdown {
                    roots: args.json_roots.roots.into_discovery_roots(defaults)?,
                    source_location: last_string(&args.source_location),
                })
            }
            Self::Path(args) => {
                args.json_roots.require_json("import path")?;
                Ok(Command::ImportPath {
                    roots: args.json_roots.roots.into_discovery_roots(defaults)?,
                    path: last_path(&args.path)
                        .ok_or_else(|| "import path requires --path".to_string())?,
                })
            }
            Self::Url(args) => {
                args.json_roots.require_json("import url")?;
                Ok(Command::ImportUrl {
                    roots: args.json_roots.roots.into_discovery_roots(defaults)?,
                    url: last_string(&args.url)
                        .ok_or_else(|| "import url requires --url".to_string())?,
                })
            }
        }
    }
}

#[derive(Debug, Args)]
struct JsonRootArgs {
    #[arg(long, action = ArgAction::Count)]
    json: u8,
    #[command(flatten)]
    roots: RootArgs,
}

impl JsonRootArgs {
    fn require_json(&self, command_name: &str) -> Result<(), String> {
        if self.json > 0 {
            Ok(())
        } else {
            Err(format!("{command_name} currently requires --json"))
        }
    }
}

#[derive(Debug, Args)]
struct ImportMarkdownArgs {
    #[command(flatten)]
    json_roots: JsonRootArgs,
    #[arg(long, value_name = "VALUE", allow_hyphen_values = true)]
    source_location: Vec<OsString>,
}

#[derive(Debug, Args)]
struct ImportPathArgs {
    #[command(flatten)]
    json_roots: JsonRootArgs,
    #[arg(long, value_name = "PATH", allow_hyphen_values = true)]
    path: Vec<PathBuf>,
}

#[derive(Debug, Args)]
struct ImportUrlArgs {
    #[command(flatten)]
    json_roots: JsonRootArgs,
    #[arg(long, value_name = "URL", allow_hyphen_values = true)]
    url: Vec<OsString>,
}

#[derive(Debug, Args)]
struct RenderAnalysisReportArgs {
    #[arg(long, value_name = "PATH", allow_hyphen_values = true)]
    input: Vec<PathBuf>,
    #[arg(long, value_name = "PATH", allow_hyphen_values = true)]
    output: Vec<PathBuf>,
}

impl RenderAnalysisReportArgs {
    fn into_command(self) -> Result<Command, String> {
        Ok(Command::RenderAnalysisReport {
            input: last_path(&self.input)
                .ok_or_else(|| "render-analysis-report requires --input".to_string())?,
            output: last_path(&self.output)
                .ok_or_else(|| "render-analysis-report requires --output".to_string())?,
        })
    }
}

#[derive(Debug, Args)]
struct SkillAgentsArgs {
    #[command(flatten)]
    json_roots: JsonRootArgs,
    #[arg(long, value_name = "NAME", allow_hyphen_values = true)]
    skill: Vec<OsString>,
    #[arg(long, value_name = "claude-code|codex", allow_hyphen_values = true)]
    agent: Vec<OsString>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnableDisableCommand {
    Enable,
    Disable,
}

impl SkillAgentsArgs {
    fn into_command(
        self,
        defaults: &RootDefaults,
        command: EnableDisableCommand,
    ) -> Result<Command, String> {
        let command_name = match command {
            EnableDisableCommand::Enable => "enable",
            EnableDisableCommand::Disable => "disable",
        };
        self.json_roots.require_json(command_name)?;

        if self.agent.is_empty() {
            return Err(format!("{command_name} requires at least one --agent"));
        }

        let agents = self
            .agent
            .iter()
            .map(|agent| parse_agent(&display_arg(agent)))
            .collect::<Result<Vec<_>, _>>()?;
        let roots = self.json_roots.roots.into_discovery_roots(defaults)?;
        let skill_name =
            last_string(&self.skill).ok_or_else(|| format!("{command_name} requires --skill"))?;

        match command {
            EnableDisableCommand::Enable => Ok(Command::Enable {
                roots,
                skill_name,
                agents,
            }),
            EnableDisableCommand::Disable => Ok(Command::Disable {
                roots,
                skill_name,
                agents,
            }),
        }
    }
}

#[derive(Debug, Args)]
struct SkillNameArgs {
    #[command(flatten)]
    json_roots: JsonRootArgs,
    #[arg(long, value_name = "NAME", allow_hyphen_values = true)]
    skill: Vec<OsString>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkillCommand {
    Promote,
    Delete,
}

impl SkillNameArgs {
    fn into_command(
        self,
        defaults: &RootDefaults,
        command: SkillCommand,
    ) -> Result<Command, String> {
        let command_name = match command {
            SkillCommand::Promote => "promote",
            SkillCommand::Delete => "delete",
        };
        self.json_roots.require_json(command_name)?;

        let roots = self.json_roots.roots.into_discovery_roots(defaults)?;
        let skill_name =
            last_string(&self.skill).ok_or_else(|| format!("{command_name} requires --skill"))?;

        match command {
            SkillCommand::Promote => Ok(Command::Promote { roots, skill_name }),
            SkillCommand::Delete => Ok(Command::Delete { roots, skill_name }),
        }
    }
}

#[derive(Debug, Clone, Default, Args, PartialEq, Eq)]
struct RootArgs {
    #[arg(long, value_name = "PATH", allow_hyphen_values = true)]
    canonical_root: Vec<PathBuf>,
    #[arg(long, value_name = "PATH", allow_hyphen_values = true)]
    imports_root: Vec<PathBuf>,
    #[arg(long, value_name = "PATH", allow_hyphen_values = true)]
    claude_code_root: Vec<PathBuf>,
    #[arg(long, value_name = "PATH", allow_hyphen_values = true)]
    codex_root: Vec<PathBuf>,
}

impl RootArgs {
    fn into_discovery_roots(self, defaults: &RootDefaults) -> Result<DiscoveryRoots, String> {
        let canonical_root = last_path(&self.canonical_root);
        let imports_root = last_path(&self.imports_root);
        let claude_code_root = last_path(&self.claude_code_root);
        let codex_root = last_path(&self.codex_root);

        let data_dir = match (
            &canonical_root,
            &imports_root,
            &claude_code_root,
            &codex_root,
        ) {
            (Some(_), Some(_), Some(_), Some(_)) => None,
            _ => Some(default_data_dir(defaults.home.clone())?),
        };

        Ok(DiscoveryRoots {
            canonical_root: canonical_root.unwrap_or_else(|| {
                data_dir
                    .as_ref()
                    .expect("data dir resolved when canonical root is defaulted")
                    .join("catalog")
                    .join("portable")
            }),
            imports_root: imports_root.unwrap_or_else(|| {
                data_dir
                    .as_ref()
                    .expect("data dir resolved when imports root is defaulted")
                    .join("imports")
            }),
            claude_code_root: claude_code_root.unwrap_or_else(|| {
                data_dir
                    .as_ref()
                    .expect("data dir resolved when Claude Code root is defaulted")
                    .join("claude-code")
            }),
            codex_root: codex_root.unwrap_or_else(|| {
                data_dir
                    .as_ref()
                    .expect("data dir resolved when Codex root is defaulted")
                    .join("codex")
            }),
        })
    }
}

pub(crate) fn default_data_dir(home: Option<OsString>) -> Result<PathBuf, String> {
    Ok(home_dir_from(home)?.join(".skills-source"))
}

pub(crate) fn home_dir_from(home: Option<OsString>) -> Result<PathBuf, String> {
    let home =
        home.ok_or_else(|| "failed to resolve home directory: HOME is not set".to_string())?;
    let home = PathBuf::from(home);
    if !home.is_absolute() {
        return Err(format!(
            "failed to resolve home directory: HOME must be an absolute path, got `{}`",
            home.display()
        ));
    }
    Ok(home)
}

fn parse_agent(value: &str) -> Result<SkillAgent, String> {
    match value {
        "claude-code" => Ok(SkillAgent::ClaudeCode),
        "codex" => Ok(SkillAgent::Codex),
        _ => Err(format!(
            "unknown agent `{value}`; expected `claude-code` or `codex`"
        )),
    }
}

fn last_path(paths: &[PathBuf]) -> Option<PathBuf> {
    paths.last().cloned()
}

fn last_string(values: &[OsString]) -> Option<String> {
    values.last().map(display_arg)
}

fn display_arg(arg: &OsString) -> String {
    arg.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    use std::os::unix::ffi::OsStringExt;
    use std::path::Path;

    fn defaults(root: &Path) -> RootDefaults {
        RootDefaults {
            home: Some(root.join("home").into_os_string()),
        }
    }

    fn explicit_roots_args(root: &Path) -> Vec<OsString> {
        vec![
            OsString::from("--canonical-root"),
            root.join("canonical").into_os_string(),
            OsString::from("--imports-root"),
            root.join("imports").into_os_string(),
            OsString::from("--claude-code-root"),
            root.join("claude").into_os_string(),
            OsString::from("--codex-root"),
            root.join("codex").into_os_string(),
        ]
    }

    fn explicit_roots(root: &Path) -> DiscoveryRoots {
        DiscoveryRoots {
            canonical_root: root.join("canonical"),
            imports_root: root.join("imports"),
            claude_code_root: root.join("claude"),
            codex_root: root.join("codex"),
        }
    }

    fn default_roots(root: &Path) -> DiscoveryRoots {
        let data_dir = root.join("home").join(".skills-source");
        DiscoveryRoots {
            canonical_root: data_dir.join("catalog").join("portable"),
            imports_root: data_dir.join("imports"),
            claude_code_root: data_dir.join("claude-code"),
            codex_root: data_dir.join("codex"),
        }
    }

    #[test]
    fn shared_root_flags_resolve_for_list_import_path_and_tui() {
        let temp = tempfile::tempdir().expect("tempdir");

        let mut list_args = vec![OsString::from("list"), OsString::from("--json")];
        list_args.extend(explicit_roots_args(temp.path()));
        assert_eq!(
            parse_command(list_args, &defaults(temp.path())).expect("list parses"),
            Command::List {
                roots: explicit_roots(temp.path()),
            }
        );

        let import_path = temp.path().join("skill.md");
        let mut import_args = vec![
            OsString::from("import"),
            OsString::from("path"),
            OsString::from("--json"),
            OsString::from("--path"),
            import_path.clone().into_os_string(),
        ];
        import_args.extend(explicit_roots_args(temp.path()));
        assert_eq!(
            parse_command(import_args, &defaults(temp.path())).expect("import path parses"),
            Command::ImportPath {
                roots: explicit_roots(temp.path()),
                path: import_path,
            }
        );

        let mut tui_args = vec![OsString::from("tui")];
        tui_args.extend(explicit_roots_args(temp.path()));
        assert_eq!(
            parse_command(tui_args, &defaults(temp.path())).expect("tui parses"),
            Command::Tui {
                roots: explicit_roots(temp.path()),
            }
        );
    }

    #[test]
    fn repeated_root_flags_use_the_last_value() {
        let temp = tempfile::tempdir().expect("tempdir");
        let first = temp.path().join("first");
        let second = temp.path().join("second");

        assert_eq!(
            parse_command(
                [
                    OsString::from("list"),
                    OsString::from("--json"),
                    OsString::from("--canonical-root"),
                    first.join("canonical").into_os_string(),
                    OsString::from("--canonical-root"),
                    second.join("canonical").into_os_string(),
                    OsString::from("--imports-root"),
                    first.join("imports").into_os_string(),
                    OsString::from("--imports-root"),
                    second.join("imports").into_os_string(),
                    OsString::from("--claude-code-root"),
                    first.join("claude").into_os_string(),
                    OsString::from("--claude-code-root"),
                    second.join("claude").into_os_string(),
                    OsString::from("--codex-root"),
                    first.join("codex").into_os_string(),
                    OsString::from("--codex-root"),
                    second.join("codex").into_os_string(),
                ],
                &RootDefaults { home: None },
            )
            .expect("repeated roots parse"),
            Command::List {
                roots: DiscoveryRoots {
                    canonical_root: second.join("canonical"),
                    imports_root: second.join("imports"),
                    claude_code_root: second.join("claude"),
                    codex_root: second.join("codex"),
                },
            }
        );
    }

    #[test]
    fn repeated_json_flags_are_idempotent() {
        let temp = tempfile::tempdir().expect("tempdir");

        assert_eq!(
            parse_command(
                [
                    OsString::from("list"),
                    OsString::from("--json"),
                    OsString::from("--json"),
                ],
                &defaults(temp.path()),
            )
            .expect("repeated json parses"),
            Command::List {
                roots: default_roots(temp.path()),
            }
        );
    }

    #[test]
    fn hyphen_prefixed_option_values_remain_supported() {
        let temp = tempfile::tempdir().expect("tempdir");

        assert_eq!(
            parse_command(
                [
                    OsString::from("import"),
                    OsString::from("path"),
                    OsString::from("--json"),
                    OsString::from("--path"),
                    OsString::from("-skill.md"),
                    OsString::from("--canonical-root"),
                    OsString::from("-canonical"),
                    OsString::from("--imports-root"),
                    temp.path().join("imports").into_os_string(),
                    OsString::from("--claude-code-root"),
                    temp.path().join("claude").into_os_string(),
                    OsString::from("--codex-root"),
                    temp.path().join("codex").into_os_string(),
                ],
                &defaults(temp.path()),
            )
            .expect("hyphen-prefixed path values parse"),
            Command::ImportPath {
                roots: DiscoveryRoots {
                    canonical_root: PathBuf::from("-canonical"),
                    imports_root: temp.path().join("imports"),
                    claude_code_root: temp.path().join("claude"),
                    codex_root: temp.path().join("codex"),
                },
                path: PathBuf::from("-skill.md"),
            }
        );

        assert_eq!(
            parse_command(
                [
                    OsString::from("enable"),
                    OsString::from("--json"),
                    OsString::from("--skill"),
                    OsString::from("-skill"),
                    OsString::from("--agent"),
                    OsString::from("codex"),
                ],
                &defaults(temp.path()),
            )
            .expect("hyphen-prefixed string values parse"),
            Command::Enable {
                roots: default_roots(temp.path()),
                skill_name: "-skill".to_string(),
                agents: vec![SkillAgent::Codex],
            }
        );
    }

    #[test]
    fn automation_commands_require_json_centrally() {
        let temp = tempfile::tempdir().expect("tempdir");

        assert_eq!(
            parse_command([OsString::from("list")], &defaults(temp.path()))
                .expect_err("list requires json"),
            "list currently requires --json"
        );
        assert_eq!(
            parse_command(
                [OsString::from("import"), OsString::from("path")],
                &defaults(temp.path()),
            )
            .expect_err("missing json wins over missing path"),
            "import path currently requires --json"
        );
        assert!(
            matches!(
                parse_command([OsString::from("tui")], &defaults(temp.path())),
                Ok(Command::Tui { .. })
            ),
            "tui should not require --json"
        );
        assert!(
            parse_command(
                [OsString::from("tui"), OsString::from("--json")],
                &defaults(temp.path()),
            )
            .expect_err("tui rejects json")
            .contains("unexpected argument '--json'")
        );
    }

    #[test]
    fn command_specific_options_are_required_after_json() {
        let temp = tempfile::tempdir().expect("tempdir");

        assert_eq!(
            parse_command(
                [
                    OsString::from("import"),
                    OsString::from("path"),
                    OsString::from("--json"),
                ],
                &defaults(temp.path()),
            )
            .expect_err("path requires path"),
            "import path requires --path"
        );
        assert_eq!(
            parse_command(
                [
                    OsString::from("enable"),
                    OsString::from("--json"),
                    OsString::from("--skill"),
                    OsString::from("skill-name"),
                ],
                &defaults(temp.path()),
            )
            .expect_err("enable requires agent"),
            "enable requires at least one --agent"
        );
    }

    #[test]
    fn repeated_singleton_options_use_the_last_value_and_agents_append() {
        let temp = tempfile::tempdir().expect("tempdir");

        assert_eq!(
            parse_command(
                [
                    OsString::from("import"),
                    OsString::from("path"),
                    OsString::from("--json"),
                    OsString::from("--path"),
                    temp.path().join("first.md").into_os_string(),
                    OsString::from("--path"),
                    temp.path().join("second.md").into_os_string(),
                ],
                &defaults(temp.path()),
            )
            .expect("path parses"),
            Command::ImportPath {
                roots: default_roots(temp.path()),
                path: temp.path().join("second.md"),
            }
        );

        assert_eq!(
            parse_command(
                [
                    OsString::from("enable"),
                    OsString::from("--json"),
                    OsString::from("--skill"),
                    OsString::from("first"),
                    OsString::from("--skill"),
                    OsString::from("second"),
                    OsString::from("--agent"),
                    OsString::from("claude-code"),
                    OsString::from("--agent"),
                    OsString::from("codex"),
                ],
                &defaults(temp.path()),
            )
            .expect("enable parses"),
            Command::Enable {
                roots: default_roots(temp.path()),
                skill_name: "second".to_string(),
                agents: vec![SkillAgent::ClaudeCode, SkillAgent::Codex],
            }
        );
    }

    #[test]
    fn import_markdown_and_url_parse_with_last_repeated_string_options() {
        let temp = tempfile::tempdir().expect("tempdir");

        assert_eq!(
            parse_command(
                [
                    OsString::from("import"),
                    OsString::from("markdown"),
                    OsString::from("--json"),
                    OsString::from("--source-location"),
                    OsString::from("first.md"),
                    OsString::from("--source-location"),
                    OsString::from("second.md"),
                ],
                &defaults(temp.path()),
            )
            .expect("markdown parses"),
            Command::ImportMarkdown {
                roots: default_roots(temp.path()),
                source_location: Some("second.md".to_string()),
            }
        );

        assert_eq!(
            parse_command(
                [
                    OsString::from("import"),
                    OsString::from("url"),
                    OsString::from("--json"),
                    OsString::from("--url"),
                    OsString::from("https://example.test/first.md"),
                    OsString::from("--url"),
                    OsString::from("https://example.test/second.md"),
                ],
                &defaults(temp.path()),
            )
            .expect("url parses"),
            Command::ImportUrl {
                roots: default_roots(temp.path()),
                url: "https://example.test/second.md".to_string(),
            }
        );
    }

    #[test]
    fn skill_commands_parse_successes_and_missing_skill_errors() {
        let temp = tempfile::tempdir().expect("tempdir");

        assert_eq!(
            parse_command(
                [
                    OsString::from("disable"),
                    OsString::from("--json"),
                    OsString::from("--skill"),
                    OsString::from("skill-name"),
                    OsString::from("--agent"),
                    OsString::from("codex"),
                ],
                &defaults(temp.path()),
            )
            .expect("disable parses"),
            Command::Disable {
                roots: default_roots(temp.path()),
                skill_name: "skill-name".to_string(),
                agents: vec![SkillAgent::Codex],
            }
        );

        assert_eq!(
            parse_command(
                [
                    OsString::from("promote"),
                    OsString::from("--json"),
                    OsString::from("--skill"),
                    OsString::from("skill-name"),
                ],
                &defaults(temp.path()),
            )
            .expect("promote parses"),
            Command::Promote {
                roots: default_roots(temp.path()),
                skill_name: "skill-name".to_string(),
            }
        );

        assert_eq!(
            parse_command(
                [
                    OsString::from("delete"),
                    OsString::from("--json"),
                    OsString::from("--skill"),
                    OsString::from("skill-name"),
                ],
                &defaults(temp.path()),
            )
            .expect("delete parses"),
            Command::Delete {
                roots: default_roots(temp.path()),
                skill_name: "skill-name".to_string(),
            }
        );

        assert_eq!(
            parse_command(
                [
                    OsString::from("disable"),
                    OsString::from("--json"),
                    OsString::from("--agent"),
                    OsString::from("codex"),
                ],
                &defaults(temp.path()),
            )
            .expect_err("disable requires skill"),
            "disable requires --skill"
        );
        assert_eq!(
            parse_command(
                [OsString::from("promote"), OsString::from("--json")],
                &defaults(temp.path()),
            )
            .expect_err("promote requires skill"),
            "promote requires --skill"
        );
    }

    #[test]
    fn invalid_agents_are_rejected() {
        let temp = tempfile::tempdir().expect("tempdir");

        assert_eq!(
            parse_command(
                [
                    OsString::from("enable"),
                    OsString::from("--json"),
                    OsString::from("--skill"),
                    OsString::from("skill-name"),
                    OsString::from("--agent"),
                    OsString::from("unknown"),
                ],
                &defaults(temp.path()),
            )
            .expect_err("agent is invalid"),
            "unknown agent `unknown`; expected `claude-code` or `codex`"
        );
    }

    #[test]
    fn help_is_a_successful_parse_result() {
        let temp = tempfile::tempdir().expect("tempdir");

        let command = parse_command([OsString::from("--help")], &defaults(temp.path()))
            .expect("help parses as success");

        match command {
            Command::Help { message } => {
                assert!(message.contains("Usage: skill-importer"));
                assert!(message.contains("Commands:"));
            }
            other => panic!("expected help command, got {other:?}"),
        }

        let command = parse_command(
            [
                OsString::from("import"),
                OsString::from("path"),
                OsString::from("--help"),
            ],
            &defaults(temp.path()),
        )
        .expect("subcommand help parses as success");

        match command {
            Command::Help { message } => {
                assert!(message.contains("Usage: skill-importer import path"));
                assert!(message.contains("--path <PATH>"));
            }
            other => panic!("expected help command, got {other:?}"),
        }
    }

    #[test]
    fn no_args_and_unknown_commands_remain_errors() {
        let temp = tempfile::tempdir().expect("tempdir");

        assert!(
            parse_command(Vec::<OsString>::new(), &defaults(temp.path()))
                .expect_err("no args errors")
                .contains("Usage: skill-importer")
        );
        assert!(
            parse_command([OsString::from("nope")], &defaults(temp.path()))
                .expect_err("unknown command errors")
                .contains("unrecognized subcommand 'nope'")
        );
    }

    #[test]
    fn default_data_dir_uses_home_skills_source() {
        let temp = tempfile::tempdir().expect("tempdir");

        assert_eq!(
            default_data_dir(Some(temp.path().join("home").into_os_string())).expect("data dir"),
            temp.path().join("home").join(".skills-source")
        );
    }

    #[test]
    fn home_dir_requires_home_to_be_set_and_absolute() {
        assert_eq!(
            home_dir_from(None).expect_err("missing HOME should fail"),
            "failed to resolve home directory: HOME is not set"
        );

        assert_eq!(
            home_dir_from(Some(OsString::from("relative-home")))
                .expect_err("relative HOME should fail"),
            "failed to resolve home directory: HOME must be an absolute path, got `relative-home`"
        );

        assert_eq!(
            home_dir_from(Some(OsString::from("/tmp/home"))).expect("absolute HOME"),
            PathBuf::from("/tmp/home")
        );
    }

    #[test]
    fn partial_root_overrides_still_require_home_for_defaulted_roots() {
        let temp = tempfile::tempdir().expect("tempdir");

        assert_eq!(
            parse_command(
                [
                    OsString::from("list"),
                    OsString::from("--json"),
                    OsString::from("--claude-code-root"),
                    temp.path().join("claude").into_os_string(),
                ],
                &RootDefaults { home: None },
            )
            .expect_err("codex root needs data dir"),
            "failed to resolve home directory: HOME is not set"
        );
        assert_eq!(
            parse_command(
                [
                    OsString::from("list"),
                    OsString::from("--json"),
                    OsString::from("--canonical-root"),
                    temp.path().join("canonical").into_os_string(),
                    OsString::from("--imports-root"),
                    temp.path().join("imports").into_os_string(),
                ],
                &RootDefaults { home: None },
            )
            .expect_err("defaulted roots need data dir"),
            "failed to resolve home directory: HOME is not set"
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_paths_parse_for_roots_and_import_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut non_utf8_name = b"skill-\xFF.md".to_vec();
        let mut non_utf8_path = temp.path().as_os_str().as_encoded_bytes().to_vec();
        non_utf8_path.push(b'/');
        non_utf8_path.append(&mut non_utf8_name);
        let import_path = PathBuf::from(OsString::from_vec(non_utf8_path));
        let canonical_root = PathBuf::from(OsString::from_vec(b"/tmp/canonical-\xFF".to_vec()));
        let imports_root = PathBuf::from(OsString::from_vec(b"/tmp/imports-\xFF".to_vec()));
        let claude_root = PathBuf::from(OsString::from_vec(b"/tmp/claude-\xFF".to_vec()));
        let codex_root = PathBuf::from(OsString::from_vec(b"/tmp/codex-\xFF".to_vec()));

        assert_eq!(
            parse_command(
                [
                    OsString::from("import"),
                    OsString::from("path"),
                    OsString::from("--json"),
                    OsString::from("--path"),
                    import_path.clone().into_os_string(),
                    OsString::from("--canonical-root"),
                    canonical_root.clone().into_os_string(),
                    OsString::from("--imports-root"),
                    imports_root.clone().into_os_string(),
                    OsString::from("--claude-code-root"),
                    claude_root.clone().into_os_string(),
                    OsString::from("--codex-root"),
                    codex_root.clone().into_os_string(),
                ],
                &RootDefaults { home: None },
            )
            .expect("non-UTF-8 paths parse"),
            Command::ImportPath {
                roots: DiscoveryRoots {
                    canonical_root,
                    imports_root,
                    claude_code_root: claude_root,
                    codex_root,
                },
                path: import_path,
            }
        );
    }
}
