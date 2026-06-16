SHELL := /bin/sh

CARGO ?= cargo
DATA_DIR ?= $(HOME)/.skills-source
CANONICAL_ROOT ?= $(DATA_DIR)/catalog/portable
IMPORTS_ROOT ?= $(DATA_DIR)/imports
CLAUDE_CODE_ROOT ?= $(DATA_DIR)/claude-code
CODEX_ROOT ?= $(DATA_DIR)/codex

ROOT_FLAGS := --canonical-root "$(CANONICAL_ROOT)" \
	--imports-root "$(IMPORTS_ROOT)" \
	--claude-code-root "$(CLAUDE_CODE_ROOT)" \
	--codex-root "$(CODEX_ROOT)"

.PHONY: help build test fmt fmt-check clippy check run run-tui run-list data-roots clean

help:
	@printf '%s\n' \
		'Targets:' \
		'  make build      Build the skill-importer crate' \
		'  make test       Run the full test suite' \
		'  make fmt        Format Rust code' \
		'  make fmt-check  Check Rust formatting' \
		'  make clippy     Run clippy with warnings denied' \
		'  make check      Run fmt-check, clippy, and test' \
		'  make run        Run the TUI with the shared data dir' \
		'  make run-list   Print inventory JSON with the shared data dir' \
		'  make clean      Remove build output' \
		'' \
		'Override roots with DATA_DIR=..., CANONICAL_ROOT=..., IMPORTS_ROOT=..., CLAUDE_CODE_ROOT=..., CODEX_ROOT=...'

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

data-roots:
	@mkdir -p "$(CANONICAL_ROOT)" "$(IMPORTS_ROOT)" "$(CLAUDE_CODE_ROOT)" "$(CODEX_ROOT)"

run: run-tui

run-tui: data-roots
	@$(CARGO) run -- tui $(ROOT_FLAGS)

run-list: data-roots
	@$(CARGO) run -- list --json $(ROOT_FLAGS)

clean:
	$(CARGO) clean
