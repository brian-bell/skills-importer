SHELL := /bin/sh

ZIG ?= zig
ZIG_DIRECT_TARGET ?=
DEV_ROOT ?= $(CURDIR)/.skill-importer/dev
AGENT_SKILLS_REPO ?= $(DEV_ROOT)/agent-skills
CANONICAL_ROOT ?= $(AGENT_SKILLS_REPO)/third-party
IMPORTS_ROOT ?= $(DEV_ROOT)/v2/imports
CLAUDE_CODE_ROOT ?= $(DEV_ROOT)/claude
CODEX_ROOT ?= $(DEV_ROOT)/codex

ROOT_FLAGS := --canonical-root "$(CANONICAL_ROOT)" \
	--imports-root "$(IMPORTS_ROOT)" \
	--claude-code-root "$(CLAUDE_CODE_ROOT)" \
	--codex-root "$(CODEX_ROOT)"

.PHONY: help build test fmt fmt-check clippy check run run-tui run-list run-prod dev-roots clean

help:
	@printf '%s\n' \
		'Targets:' \
		'  make build      Build the skill-importer binary' \
		'  make test       Run the Zig test suite' \
		'  make fmt        Format Zig code' \
		'  make fmt-check  Check Zig formatting' \
		'  make clippy     Compatibility no-op; Zig has no clippy equivalent' \
		'  make check      Run fmt-check and test' \
		'  make run        Run the TUI with disposable local roots' \
		'  make run-list   Print inventory JSON with disposable local roots' \
		'  make run-prod   Run the TUI with user-level agent roots' \
		'  make clean      Remove build output and disposable local roots' \
		'' \
		'Override roots with AGENT_SKILLS_REPO=..., CANONICAL_ROOT=..., IMPORTS_ROOT=..., CLAUDE_CODE_ROOT=..., CODEX_ROOT=...' \
		'Set ZIG_DIRECT_TARGET=aarch64-macos.15.0 to bypass zig build runner issues on newer macOS hosts.'

ifeq ($(strip $(ZIG_DIRECT_TARGET)),)
build:
	$(ZIG) build

test:
	$(ZIG) build test
else
build:
	@mkdir -p zig-out/bin
	$(ZIG) build-exe -target $(ZIG_DIRECT_TARGET) --dep skill_importer -Mroot=src/main.zig -Mskill_importer=src/root.zig -femit-bin=zig-out/bin/skill-importer

test:
	$(ZIG) test -target $(ZIG_DIRECT_TARGET) --dep skill_importer -Mroot=src/root_test.zig -Mskill_importer=src/root.zig
endif

fmt:
	$(ZIG) fmt build.zig src

fmt-check:
	$(ZIG) fmt --check build.zig src

clippy:
	@printf '%s\n' 'Zig has no clippy equivalent; use make test for compile-time checks.'

check: fmt-check test

dev-roots:
	@mkdir -p "$(CANONICAL_ROOT)" "$(IMPORTS_ROOT)" "$(CLAUDE_CODE_ROOT)" "$(CODEX_ROOT)"

run: run-tui

run-tui: dev-roots build
	@./zig-out/bin/skill-importer tui $(ROOT_FLAGS)

run-list: dev-roots build
	@./zig-out/bin/skill-importer list --json $(ROOT_FLAGS)

run-prod: build
	./zig-out/bin/skill-importer tui

clean:
	rm -rf .zig-cache zig-out "$(DEV_ROOT)"
