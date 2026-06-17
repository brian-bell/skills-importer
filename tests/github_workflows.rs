use std::fs;

fn workflow(name: &str) -> String {
    fs::read_to_string(format!(".github/workflows/{name}.yml")).unwrap()
}

fn goreleaser_config() -> String {
    fs::read_to_string(".goreleaser.yml").unwrap()
}

#[test]
fn pull_request_workflow_runs_rust_quality_gates() {
    let yaml = workflow("pull-request");

    assert!(yaml.contains("pull_request:"));
    assert!(yaml.contains("cargo fmt --check"));
    assert!(yaml.contains("cargo clippy --all-targets -- -D warnings"));
    assert!(yaml.contains("cargo test"));
}

#[test]
fn main_workflow_runs_on_main_pushes() {
    let yaml = workflow("main");

    assert!(yaml.contains("push:"));
    assert!(yaml.contains("branches: [main]"));
    assert!(yaml.contains("make check"));
}

#[test]
fn workflows_avoid_node20_action_runtime_warnings() {
    for workflow_name in ["main", "pull-request", "release", "release-snapshot"] {
        let yaml = workflow(workflow_name);

        assert!(!yaml.contains("actions/checkout@v4"));
        assert!(!yaml.contains("mlugg/setup-zig@"));
    }

    assert!(workflow("main").contains("actions/checkout@v6"));
    assert!(workflow("pull-request").contains("actions/checkout@v6"));
    assert!(workflow("release").contains("brew install zig"));
    assert!(workflow("release-snapshot").contains("brew install zig"));
}

#[test]
fn release_workflow_builds_tagged_releases() {
    let yaml = workflow("release");

    assert!(yaml.contains("tags: ['v*']"));
    assert!(yaml.contains("workflow_dispatch:"));
    assert!(yaml.contains("runs-on: macos-latest"));
    assert!(yaml.contains("goreleaser/goreleaser-action"));
    assert!(yaml.contains("args: release --clean"));
    assert!(yaml.contains("HOMEBREW_TAP_GITHUB_TOKEN"));
}

#[test]
fn release_snapshot_workflow_verifies_packaging_without_publishing() {
    let yaml = workflow("release-snapshot");

    assert!(yaml.contains("branches: [main]"));
    assert!(yaml.contains("runs-on: macos-latest"));
    assert!(yaml.contains("goreleaser/goreleaser-action"));
    assert!(yaml.contains("args: release --snapshot --clean --skip=publish"));
}

#[test]
fn goreleaser_config_builds_rust_archives_for_homebrew_tap() {
    let yaml = goreleaser_config();

    assert!(yaml.contains("project_name: skill-importer"));
    assert!(yaml.contains("builder: rust"));
    assert!(yaml.contains("binary: skill-importer"));
    assert!(yaml.contains("x86_64-apple-darwin"));
    assert!(yaml.contains("aarch64-apple-darwin"));
    assert!(yaml.contains("x86_64-unknown-linux-gnu"));
    assert!(yaml.contains("aarch64-unknown-linux-gnu"));
    assert!(
        yaml.contains("name_template: \"{{ .ProjectName }}_{{ .Version }}_{{ .Os }}_{{ .Arch }}\"")
    );
    assert!(yaml.contains("homebrew_casks:"));
    assert!(yaml.contains("directory: Casks"));
    assert!(yaml.contains("caveats:"));
    assert!(yaml.contains("xattr -dr com.apple.quarantine"));
    assert!(yaml.contains("owner: brian-bell"));
    assert!(yaml.contains("name: homebrew-tap"));
}
