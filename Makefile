SHELL := /bin/sh

CARGO ?= cargo
SKILLS_REPO ?= $(abspath $(CURDIR)/../skills)
DEV_ROOT ?= $(CURDIR)/.skill-importer/dev
CANONICAL_ROOT ?= $(SKILLS_REPO)/catalog/portable
IMPORTS_ROOT ?= $(DEV_ROOT)/imports
CLAUDE_CODE_ROOT ?= $(DEV_ROOT)/claude
CODEX_ROOT ?= $(DEV_ROOT)/codex

ROOT_FLAGS := --canonical-root "$(CANONICAL_ROOT)" \
	--imports-root "$(IMPORTS_ROOT)" \
	--claude-code-root "$(CLAUDE_CODE_ROOT)" \
	--codex-root "$(CODEX_ROOT)"

.PHONY: help build test fmt fmt-check clippy check run run-tui run-list dev-roots clean

help:
	@printf '%s\n' \
		'Targets:' \
		'  make build      Build the skill-importer crate' \
		'  make test       Run the full test suite' \
		'  make fmt        Format Rust code' \
		'  make fmt-check  Check Rust formatting' \
		'  make clippy     Run clippy with warnings denied' \
		'  make check      Run fmt-check, clippy, and test' \
		'  make run        Run the TUI with disposable local roots' \
		'  make run-list   Print inventory JSON with disposable local roots' \
		'  make clean      Remove build output and disposable local roots' \
		'' \
		'Override roots with SKILLS_REPO=..., CANONICAL_ROOT=..., IMPORTS_ROOT=..., CLAUDE_CODE_ROOT=..., CODEX_ROOT=...'

build:
	$(CARGO) build

test:
	$(CARGO) test

fmt:
	$(CARGO) fmt

fmt-check:
	$(CARGO) fmt --check

clippy:
	$(CARGO) clippy --all-targets -- -D warnings

check: fmt-check clippy test

dev-roots:
	@mkdir -p "$(IMPORTS_ROOT)" "$(CLAUDE_CODE_ROOT)" "$(CODEX_ROOT)"

run: run-tui

run-tui: dev-roots
	@$(CARGO) run -- tui $(ROOT_FLAGS)

run-list: dev-roots
	@$(CARGO) run -- list --json $(ROOT_FLAGS)

clean:
	$(CARGO) clean
	rm -rf "$(DEV_ROOT)"
