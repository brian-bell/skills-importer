use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use skill_importer::{
    DiscoveryRoots, ImportMarkdownRequest, ImportRepositoryRequest, ImportSourceType,
    ImportUrlRequest, PromoteSkillRequest, RepositoryImportResult, SkillAgent,
    SkillRepositoryCheckout, SkillRepositoryFetchError, SkillRepositoryProvider,
    SkillUrlFetchError, SkillUrlFetcher, json_adapter, workflow,
};

#[test]
fn workflow_list_outcome_renders_existing_inventory_json_shape() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    write_skill(
        &roots.canonical_root,
        "workflow-helper",
        "Listed through workflow.",
    );

    let outcome = workflow::execute(
        &roots,
        workflow::OperationRequest::List,
        &UnusedFetcher,
        &UnusedRepositoryProvider,
    )
    .expect("workflow list succeeds");

    let json = json_adapter::outcome_to_value(&outcome).expect("json outcome");
    let skills = json["skills"].as_array().expect("skills array");
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0]["name"], "workflow-helper");
    assert_eq!(skills[0]["description"], "Listed through workflow.");
    assert_eq!(skills[0]["source"], "canonical");
    assert_eq!(skills[0]["agent_entries"]["claude_code"], "missing");
}

#[test]
fn workflow_dispatches_effectful_operations_before_json_rendering() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    workflow::execute(
        &roots,
        workflow::OperationRequest::ImportMarkdown(ImportMarkdownRequest {
            markdown: r#"---
name: workflow-import
description: Imported through workflow.
---

# Workflow Import
"#,
            source_location: Some("test"),
        }),
        &UnusedFetcher,
        &UnusedRepositoryProvider,
    )
    .expect("workflow import succeeds");
    workflow::execute(
        &roots,
        workflow::OperationRequest::Promote(PromoteSkillRequest {
            skill_name: "workflow-import",
            overwrite: false,
        }),
        &UnusedFetcher,
        &UnusedRepositoryProvider,
    )
    .expect("workflow promote succeeds");

    let outcome = workflow::execute(
        &roots,
        workflow::OperationRequest::Enable {
            skill_name: "workflow-import",
            agents: &[SkillAgent::Codex],
        },
        &UnusedFetcher,
        &UnusedRepositoryProvider,
    )
    .expect("workflow enable succeeds");

    let json = json_adapter::outcome_to_value(&outcome).expect("json outcome");
    assert_eq!(json["skill_name"], "workflow-import");
    assert_eq!(json["actions"][1]["action"], "create_symlink");
    assert_eq!(json["actions"][1]["agent"], "codex");
    assert!(roots.codex_root.join("workflow-import").exists());
}

#[test]
fn json_adapter_writes_pretty_json_with_trailing_newline() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    write_skill(&roots.canonical_root, "pretty-helper", "Pretty JSON.");
    let outcome = workflow::execute(
        &roots,
        workflow::OperationRequest::List,
        &UnusedFetcher,
        &UnusedRepositoryProvider,
    )
    .expect("workflow list succeeds");

    let mut output = Vec::new();
    json_adapter::write_outcome(&mut output, &outcome).expect("write json");

    assert!(output.ends_with(b"\n"));
    let rendered = String::from_utf8(output).expect("utf8");
    assert!(
        rendered.contains("\n  \"skills\": ["),
        "expected pretty JSON, got {rendered}"
    );
    let json: Value = serde_json::from_str(&rendered).expect("valid json");
    assert_eq!(json["skills"][0]["name"], "pretty-helper");
}

#[test]
fn json_adapter_preserves_existing_byte_order_for_list_and_effectful_results() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    write_skill(
        &roots.canonical_root,
        "order-helper",
        "Byte order stays stable.",
    );
    let list_outcome = workflow::execute(
        &roots,
        workflow::OperationRequest::List,
        &UnusedFetcher,
        &UnusedRepositoryProvider,
    )
    .expect("workflow list succeeds");

    let mut list_output = Vec::new();
    json_adapter::write_outcome(&mut list_output, &list_outcome).expect("write list json");
    let list_rendered = String::from_utf8(list_output).expect("utf8 list");
    assert!(
        list_rendered.find("\"name\"").expect("name key")
            < list_rendered
                .find("\"description\"")
                .expect("description key"),
        "list key order changed: {list_rendered}"
    );
    assert!(
        list_rendered
            .find("\"description\"")
            .expect("description key")
            < list_rendered.find("\"source\"").expect("source key"),
        "list key order changed: {list_rendered}"
    );

    let import_outcome = workflow::execute(
        &roots,
        workflow::OperationRequest::ImportUrl(ImportUrlRequest {
            url: "https://example.test/order-import.md",
        }),
        &StaticFetcher {
            markdown: r#"---
name: order-import
description: Imported through URL workflow.
---

# Order Import
"#,
        },
        &UnusedRepositoryProvider,
    )
    .expect("workflow url import succeeds");
    let mut import_output = Vec::new();
    json_adapter::write_outcome(&mut import_output, &import_outcome).expect("write import json");
    let import_rendered = String::from_utf8(import_output).expect("utf8 import");
    assert!(
        import_rendered
            .find("\"skill_name\"")
            .expect("skill_name key")
            < import_rendered
                .find("\"skill_path\"")
                .expect("skill_path key"),
        "import key order changed: {import_rendered}"
    );
    assert!(
        import_rendered.find("\"manifest\"").expect("manifest key")
            < import_rendered.find("\"actions\"").expect("actions key"),
        "import key order changed: {import_rendered}"
    );
}

#[test]
fn workflow_import_url_uses_injected_fetcher() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    let fetcher = RecordingFetcher {
        calls: RefCell::new(Vec::new()),
        markdown: r#"---
name: fetched-helper
description: Fetched through workflow.
---

# Fetched Helper
"#,
    };

    let outcome = workflow::execute(
        &roots,
        workflow::OperationRequest::ImportUrl(ImportUrlRequest {
            url: "https://example.test/fetched-helper.md",
        }),
        &fetcher,
        &UnusedRepositoryProvider,
    )
    .expect("workflow url import succeeds");

    assert_eq!(
        fetcher.calls.borrow().as_slice(),
        ["https://example.test/fetched-helper.md"]
    );
    match outcome {
        workflow::OperationOutcome::Import(import) => {
            assert_eq!(import.skill_name, "fetched-helper");
            assert_eq!(import.manifest.source_type, ImportSourceType::Url);
        }
        outcome => panic!("expected import outcome, got {outcome:?}"),
    }
}

#[test]
fn workflow_import_repository_uses_injected_provider_and_preserves_selection() {
    let temp = tempfile::tempdir().expect("tempdir");
    let roots = roots(temp.path());
    let repository = temp.path().join("repo");
    write_skill(&repository, "repo-alpha", "First repository skill.");
    write_skill(&repository, "repo-beta", "Second repository skill.");
    let provider = StaticRepositoryProvider {
        calls: RefCell::new(Vec::new()),
        repository_path: repository,
    };

    let selection = workflow::execute(
        &roots,
        workflow::OperationRequest::ImportRepository(ImportRepositoryRequest {
            repository: "https://example.test/repo.git",
            selected_skill_paths: &[],
        }),
        &UnusedFetcher,
        &provider,
    )
    .expect("workflow repository selection succeeds");

    assert_eq!(
        provider.calls.borrow().as_slice(),
        ["https://example.test/repo.git"]
    );
    match selection {
        workflow::OperationOutcome::RepositoryImport(RepositoryImportResult::Selection(
            selection,
        )) => {
            assert_eq!(selection.repository, "https://example.test/repo.git");
            assert_eq!(selection.skills.len(), 2);
            assert_eq!(selection.skills[0].name, "repo-alpha");
        }
        outcome => panic!("expected repository selection, got {outcome:?}"),
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
    let skill_dir = root.join(name);
    fs::create_dir_all(&skill_dir).expect("skill dir");
    fs::write(
        skill_dir.join("SKILL.md"),
        format!(
            r#"---
name: {name}
description: {description}
---
"#
        ),
    )
    .expect("skill file");
}

struct UnusedFetcher;

impl SkillUrlFetcher for UnusedFetcher {
    fn fetch_skill_markdown(&self, _url: &str) -> Result<String, SkillUrlFetchError> {
        panic!("workflow test should not fetch URLs")
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

struct RecordingFetcher {
    calls: RefCell<Vec<String>>,
    markdown: &'static str,
}

impl SkillUrlFetcher for RecordingFetcher {
    fn fetch_skill_markdown(&self, url: &str) -> Result<String, SkillUrlFetchError> {
        self.calls.borrow_mut().push(url.to_string());
        Ok(self.markdown.to_string())
    }
}

struct UnusedRepositoryProvider;

impl SkillRepositoryProvider for UnusedRepositoryProvider {
    type Checkout = UnusedCheckout;

    fn fetch_repository(
        &self,
        _repository: &str,
    ) -> Result<Self::Checkout, SkillRepositoryFetchError> {
        panic!("workflow test should not fetch repositories")
    }
}

struct UnusedCheckout {
    path: PathBuf,
}

impl SkillRepositoryCheckout for UnusedCheckout {
    fn path(&self) -> &Path {
        &self.path
    }
}

struct StaticRepositoryProvider {
    calls: RefCell<Vec<String>>,
    repository_path: PathBuf,
}

impl SkillRepositoryProvider for StaticRepositoryProvider {
    type Checkout = StaticRepositoryCheckout;

    fn fetch_repository(
        &self,
        repository: &str,
    ) -> Result<Self::Checkout, SkillRepositoryFetchError> {
        self.calls.borrow_mut().push(repository.to_string());
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
