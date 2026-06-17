use std::fs;

fn workflow(name: &str) -> String {
    fs::read_to_string(format!(".github/workflows/{name}.yml")).unwrap()
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
fn release_workflow_builds_tagged_releases() {
    let yaml = workflow("release");

    assert!(yaml.contains("tags: ['v*']"));
    assert!(yaml.contains("workflow_dispatch:"));
    assert!(yaml.contains("cargo metadata --no-deps --format-version 1"));
    assert!(yaml.contains("cargo build --release"));
    assert!(yaml.contains("softprops/action-gh-release"));
}
