use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{ImportManifest, analyzer::shell_quote};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotePrLaunchRequest {
    pub skill_name: String,
    pub skills_repo: PathBuf,
    pub promoted_skill_path: PathBuf,
    pub import_manifest: ImportManifest,
    pub analysis_reports: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotePrLaunchResult {
    pub prompt_path: PathBuf,
    pub script_path: PathBuf,
}

pub trait PromotionPrLauncher {
    fn launch(&self, request: PromotePrLaunchRequest) -> Result<PromotePrLaunchResult, String>;
}

#[derive(Debug, Clone, Default)]
pub struct NoopPromotionPrLauncher;

impl PromotionPrLauncher for NoopPromotionPrLauncher {
    fn launch(&self, request: PromotePrLaunchRequest) -> Result<PromotePrLaunchResult, String> {
        Ok(PromotePrLaunchResult {
            prompt_path: request
                .skills_repo
                .join("PROMOTION_PR_PROMPT_NOT_LAUNCHED.txt"),
            script_path: request
                .skills_repo
                .join("PROMOTION_PR_SCRIPT_NOT_LAUNCHED.sh"),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct TerminalPromotionPrLauncher;

impl PromotionPrLauncher for TerminalPromotionPrLauncher {
    fn launch(&self, request: PromotePrLaunchRequest) -> Result<PromotePrLaunchResult, String> {
        if std::env::var_os("SKILL_IMPORTER_PROMOTION_PR_DRY_RUN").is_some() {
            ensure_agent_skills_checkout(&request.skills_repo)?;
            let workspace = request
                .skills_repo
                .join(".skill-importer")
                .join("promotion-pr");
            fs::create_dir_all(&workspace).map_err(|error| {
                format!("failed to create dry-run promotion PR workspace: {error}")
            })?;
            let prompt_path = workspace.join(format!("{}-prompt.txt", request.skill_name));
            let script_path = workspace.join(format!("{}-run.sh", request.skill_name));
            fs::write(&prompt_path, build_promotion_pr_prompt(&request))
                .map_err(|error| format!("failed to write promotion PR prompt: {error}"))?;
            fs::write(
                &script_path,
                render_promotion_pr_script(&request.skills_repo, &prompt_path),
            )
            .map_err(|error| format!("failed to write promotion PR script: {error}"))?;
            return Ok(PromotePrLaunchResult {
                prompt_path,
                script_path,
            });
        }
        if !cfg!(target_os = "macos") {
            return Err("promotion PR launch is currently supported only on macOS".to_string());
        }
        ensure_codex_available()?;
        ensure_agent_skills_checkout(&request.skills_repo)?;

        let workspace = allocate_workspace_dir(&promotion_parent_dir()?, &request.skill_name)?;
        fs::create_dir_all(&workspace)
            .map_err(|error| format!("failed to create promotion PR workspace: {error}"))?;
        let prompt_path = workspace.join("prompt.txt");
        let script_path = workspace.join("run-promotion-pr.sh");
        let prompt = build_promotion_pr_prompt(&request);
        fs::write(&prompt_path, prompt)
            .map_err(|error| format!("failed to write promotion PR prompt: {error}"))?;
        fs::write(
            &script_path,
            render_promotion_pr_script(&request.skills_repo, &prompt_path),
        )
        .map_err(|error| format!("failed to write promotion PR script: {error}"))?;
        launch_terminal_script(&script_path)?;

        Ok(PromotePrLaunchResult {
            prompt_path,
            script_path,
        })
    }
}

pub fn default_skills_repo() -> PathBuf {
    std::env::var_os("SKILL_IMPORTER_SKILLS_REPO")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/Users/brian/dev/agent-skills"))
}

pub fn build_promotion_pr_prompt(request: &PromotePrLaunchRequest) -> String {
    let mut prompt = format!(
        r#"You are preparing a pull request in brian-bell/agent-skills for the imported skill {skill_name:?}.

The skill has already been copied into:
{promoted_path}

Work in this checkout:
{skills_repo}

Create or update a topic branch for this promotion. Verify the copied skill files before editing repo metadata.

Required updates:
- README.md
- AGENTS.md
- scripts/install-skills.sh
- third-party/ATTRIBUTION.md

Preserve upstream attribution from the import manifest:
- source_type: {source_type:?}
- source_location: {source_location}
- source_repository: {source_repository}
- imported_at: {imported_at}
- content_hash: {content_hash}

Run the available checks for this repository, then commit, push, and open a pull request. Do not include unrelated work.
"#,
        skill_name = request.skill_name,
        promoted_path = request.promoted_skill_path.display(),
        skills_repo = request.skills_repo.display(),
        source_type = request.import_manifest.source_type,
        source_location = request
            .import_manifest
            .source_location
            .as_deref()
            .unwrap_or("none"),
        source_repository = request
            .import_manifest
            .source_repository
            .as_ref()
            .map(|repository| { format!("{}#{}", repository.repository, repository.skill_path) })
            .unwrap_or_else(|| "none".to_string()),
        imported_at = request.import_manifest.imported_at,
        content_hash = request.import_manifest.content_hash,
    );

    if request.analysis_reports.is_empty() {
        prompt.push_str("\nNo existing skill analysis report was found. Continue without one.\n");
    } else {
        prompt.push_str("\nInclude these existing skill analysis findings in the PR context:\n");
        for report in &request.analysis_reports {
            prompt.push_str("- ");
            prompt.push_str(&report.display().to_string());
            prompt.push('\n');
        }
    }

    prompt
}

pub fn render_promotion_pr_script(skills_repo: &Path, prompt_path: &Path) -> String {
    format!(
        r#"#!/bin/sh
set -eu

if [ ! -d {skills_repo} ]; then
  echo "agent-skills checkout was not found: {skills_repo}" >&2
  exit 1
fi

if ! command -v codex >/dev/null 2>&1; then
  echo "codex CLI was not found on PATH" >&2
  exit 127
fi

cd {skills_repo}
codex exec --skip-git-repo-check - < {prompt_path}
"#,
        skills_repo = shell_quote_path(skills_repo),
        prompt_path = shell_quote_path(prompt_path),
    )
}

pub fn discover_analysis_reports(skill_name: &str) -> Vec<PathBuf> {
    let parent = match promotion_analysis_parent_dir() {
        Some(parent) => parent,
        None => return Vec::new(),
    };
    let mut reports = Vec::new();
    let entries = match fs::read_dir(parent) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        if !file_name.starts_with(skill_name) {
            continue;
        }
        for report in [
            entry.path().join("report").join("report.json"),
            entry.path().join("report").join("index.html"),
        ] {
            if report.is_file() {
                reports.push(report);
            }
        }
    }
    reports.sort();
    reports
}

fn promotion_parent_dir() -> Result<PathBuf, String> {
    if cfg!(target_os = "macos") {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| "HOME must be set to launch promotion PR workflow".to_string())?;
        Ok(home
            .join("Library")
            .join("Caches")
            .join("skill-importer-promotion-pr"))
    } else {
        Ok(std::env::temp_dir().join("skill-importer-promotion-pr"))
    }
}

fn promotion_analysis_parent_dir() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(PathBuf::from).map(|home| {
            home.join("Library")
                .join("Caches")
                .join("skill-importer-analysis")
        })
    } else {
        Some(std::env::temp_dir().join("skill-importer-analysis"))
    }
}

fn allocate_workspace_dir(parent: &Path, skill_name: &str) -> Result<PathBuf, String> {
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create promotion PR cache root: {error}"))?;
    let slug = sanitize_name(skill_name);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
        .as_millis();
    for attempt in 0..100 {
        let candidate = parent.join(format!("{slug}-{now}-{attempt}"));
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!("failed to create promotion PR workspace: {error}"));
            }
        }
    }
    Err("failed to allocate a unique promotion PR workspace after 100 attempts".to_string())
}

fn sanitize_name(name: &str) -> String {
    let slug = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if slug.is_empty() {
        "skill".to_string()
    } else {
        slug
    }
}

fn ensure_agent_skills_checkout(path: &Path) -> Result<(), String> {
    if !path.is_dir() {
        return Err(format!(
            "agent-skills checkout was not found: {}",
            path.display()
        ));
    }
    if !path.join("third-party").is_dir() {
        return Err(format!(
            "agent-skills checkout is missing third-party directory: {}",
            path.display()
        ));
    }
    Ok(())
}

fn ensure_codex_available() -> Result<(), String> {
    Command::new("codex")
        .arg("--version")
        .output()
        .map(|_| ())
        .map_err(|error| format!("codex CLI was not found or could not be executed: {error}"))
}

fn launch_terminal_script(script_path: &Path) -> Result<(), String> {
    let command = format!("sh {}", shell_quote_path(script_path));
    let output = Command::new("osascript")
        .args([
            "-e",
            &format!(
                "tell application \"Terminal\" to do script {}",
                applescript_quote(&command)
            ),
            "-e",
            "tell application \"Terminal\" to activate",
        ])
        .output()
        .map_err(|error| format!("failed to launch Terminal: {error}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn shell_quote_path(path: &Path) -> String {
    shell_quote(&path.to_string_lossy())
}

fn applescript_quote(value: &str) -> String {
    let mut quoted = String::from("\"");
    for character in value.chars() {
        match character {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            _ => quoted.push(character),
        }
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ImportManifest, ImportSourceType};

    #[test]
    fn promotion_prompt_instructs_codex_to_prepare_agent_skills_pr() {
        let request = PromotePrLaunchRequest {
            skill_name: "demo-helper".to_string(),
            skills_repo: PathBuf::from("/tmp/agent-skills"),
            promoted_skill_path: PathBuf::from("/tmp/agent-skills/third-party/demo-helper"),
            import_manifest: ImportManifest {
                source_type: ImportSourceType::Repository,
                source_location: Some("https://example.test/repo#skills/demo".to_string()),
                source_repository: Some(crate::ImportSourceRepository {
                    repository: "https://example.test/repo".to_string(),
                    skill_path: "skills/demo".to_string(),
                }),
                imported_at: 10,
                content_hash: "sha256:abc".to_string(),
                promoted: false,
            },
            analysis_reports: vec![PathBuf::from("/tmp/analysis/report.json")],
        };

        let prompt = build_promotion_pr_prompt(&request);

        assert!(prompt.contains("brian-bell/agent-skills"));
        assert!(prompt.contains("Create or update a topic branch"));
        assert!(prompt.contains("README.md"));
        assert!(prompt.contains("AGENTS.md"));
        assert!(prompt.contains("scripts/install-skills.sh"));
        assert!(prompt.contains("third-party/ATTRIBUTION.md"));
        assert!(prompt.contains("https://example.test/repo#skills/demo"));
        assert!(prompt.contains("/tmp/analysis/report.json"));
        assert!(prompt.contains("commit, push, and open a pull request"));
    }

    #[test]
    fn promotion_script_runs_headless_codex_from_agent_skills_checkout() {
        let script = render_promotion_pr_script(
            Path::new("/tmp/agent skills"),
            Path::new("/tmp/prompt's.txt"),
        );

        assert!(script.contains("cd '/tmp/agent skills'"));
        assert!(script.contains("command -v codex"));
        assert!(script.contains("codex exec --skip-git-repo-check - < '/tmp/prompt'\\''s.txt'"));
    }
}
