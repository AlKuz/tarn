MAKEFLAGS += --warn-undefined-variables
SHELL := bash
.SHELLFLAGS := -eu -o pipefail -c
.DEFAULT_GOAL := help
.DELETE_ON_ERROR:
.SUFFIXES:

# ── Configuration ────────────────────────────────────────────────────
CARGO := cargo
UV    := uv
cmd   ?=

# Benchmark settings. `dataset` selects one or more BEIR corpora by name, or
# `all` for the whole catalogue. scifact is the default: it is small, and the one
# most BEIR baseline papers report first, so a comparable number exists. Names
# are resolved against the catalogue in download_beir.py, which stays the single
# source of truth for what exists.
dataset  ?= scifact

# The configuration under test. report.py groups runs by exactly these, so
# changing one and re-running produces a comparison rather than an overwrite.
# cargo features the release binary is built with; `none` for no features at all.
# Stated explicitly rather than relying on Cargo's defaults, so the value recorded
# in the run manifest is the value the binary was actually built with.
FEATURES ?= stemming
# sections requested per query; tarn's own default is 20, too shallow for k=100
TOP_K ?= 100
# cumulative token budget over the returned sections; empty means no budget
TOKEN_LIMIT ?=
# minimum fused score; the scale caps near 0.0328, so anything higher returns nothing
SCORE_THRESHOLD ?= 0.0
# non-empty rebuilds the index from scratch instead of reusing persisted state
COLD ?=

TARN_BIN := target/release/tarn-mcp
BENCH    := $(UV) run --quiet python scripts/bench
CARGO_FEATURES = --no-default-features $(if $(filter-out none,$(FEATURES)),--features "$(FEATURES)")
SEARCH_ARGS = --tarn-bin $(TARN_BIN) --features "$(FEATURES)" --top-k $(TOP_K) \
              --score-threshold $(SCORE_THRESHOLD) \
              $(if $(TOKEN_LIMIT),--token-limit $(TOKEN_LIMIT)) $(if $(COLD),--cold)

GREEN  := \033[0;32m
YELLOW := \033[0;33m
BLUE   := \033[0;34m
RED    := \033[0;31m
NC     := \033[0m

# ── Help ─────────────────────────────────────────────────────────────
.PHONY: help
help:
	@printf "Usage: make <target> [cmd=<subcommand>]\n\n"
	@printf "$(BLUE)build$(NC)    Build operations (cmd=debug|release|check|run)\n"
	@printf "$(BLUE)test$(NC)     Test operations (cmd=all|unit|integration|verbose|coverage)\n"
	@printf "$(BLUE)lint$(NC)     Code quality (cmd=check|fix|fmt)\n"
	@printf "$(BLUE)doc$(NC)      Documentation (cmd=build|open)\n"
	@printf "$(BLUE)clean$(NC)    Remove build artifacts and coverage reports\n"
	@printf "$(BLUE)ci$(NC)       CI pipeline (cmd=full|quick)\n"
	@printf "$(BLUE)bench$(NC)    Retrieval evaluation (cmd=setup|list|download|adapt|search|score|ragas|report|clean)\n\n"
	@printf "Examples:\n"
	@printf "  make build                  Build in debug mode (default)\n"
	@printf "  make build cmd=release      Build in release mode\n"
	@printf "  make test cmd=integration   Run integration tests\n"
	@printf "  make test cmd=coverage      Generate HTML coverage report\n"
	@printf "  make bench                  Full retrieval eval on scifact\n"
	@printf "  make bench dataset=nfcorpus Another corpus\n"
	@printf "  make bench dataset=all      The whole catalogue\n"
	@printf "  make bench dataset=\"scifact nfcorpus\"\n"
	@printf "  make bench TOP_K=20         Compare a configuration against the default\n\n"
	@printf "Benchmark results land in target/benchmarks/report.md.\n"
	@printf "Note: '$(BLUE)make clean$(NC)' runs 'cargo clean', which deletes them along with target/.\n"

# ── Build ────────────────────────────────────────────────────────────
.PHONY: build
build:
	@case "$(cmd)" in \
		release) \
			printf "$(BLUE)→ Building release...$(NC)\n"; \
			$(CARGO) build --release;; \
		check) \
			printf "$(BLUE)→ Type-checking...$(NC)\n"; \
			$(CARGO) check;; \
		run) \
			printf "$(BLUE)→ Running MCP server...$(NC)\n"; \
			$(CARGO) run;; \
		""|debug) \
			printf "$(BLUE)→ Building debug...$(NC)\n"; \
			$(CARGO) build;; \
		*) \
			printf "$(RED)✗ Unknown cmd '$(cmd)'$(NC)\n"; \
			printf "Commands: debug (default), release, check, run\n"; \
			exit 1;; \
	esac

# ── Testing ──────────────────────────────────────────────────────────
.PHONY: test
test:
	@case "$(cmd)" in \
		unit) \
			printf "$(BLUE)→ Running unit tests...$(NC)\n"; \
			$(CARGO) test --lib;; \
		integration) \
			printf "$(BLUE)→ Running integration tests...$(NC)\n"; \
			$(CARGO) test --test '*';; \
		verbose) \
			printf "$(BLUE)→ Running tests with output...$(NC)\n"; \
			$(CARGO) test -- --nocapture;; \
		coverage) \
			printf "$(BLUE)→ Generating HTML coverage report...$(NC)\n"; \
			mkdir -p coverage; \
			$(CARGO) llvm-cov --all-features --html --output-dir coverage; \
			printf "$(GREEN)✓ Report: coverage/html/index.html$(NC)\n";; \
		""|all) \
			printf "$(BLUE)→ Running all tests...$(NC)\n"; \
			$(CARGO) test;; \
		*) \
			printf "$(RED)✗ Unknown cmd '$(cmd)'$(NC)\n"; \
			printf "Commands: all (default), unit, integration, verbose, coverage\n"; \
			exit 1;; \
	esac

# ── Linting & Formatting ─────────────────────────────────────────────
.PHONY: lint
lint:
	@case "$(cmd)" in \
		fix) \
			printf "$(BLUE)→ Auto-fixing...$(NC)\n"; \
			$(CARGO) clippy --fix --allow-dirty; \
			$(CARGO) fmt;; \
		fmt) \
			printf "$(BLUE)→ Formatting code...$(NC)\n"; \
			$(CARGO) fmt;; \
		""|check) \
			printf "$(BLUE)→ Running linters...$(NC)\n"; \
			$(CARGO) fmt -- --check; \
			$(CARGO) clippy -- -D warnings;; \
		*) \
			printf "$(RED)✗ Unknown cmd '$(cmd)'$(NC)\n"; \
			printf "Commands: check (default), fix, fmt\n"; \
			exit 1;; \
	esac

# ── Documentation ────────────────────────────────────────────────────
.PHONY: doc
doc:
	@case "$(cmd)" in \
		open) \
			printf "$(BLUE)→ Generating and opening docs...$(NC)\n"; \
			$(CARGO) doc --no-deps --open;; \
		""|build) \
			printf "$(BLUE)→ Generating documentation...$(NC)\n"; \
			$(CARGO) doc --no-deps;; \
		*) \
			printf "$(RED)✗ Unknown cmd '$(cmd)'$(NC)\n"; \
			printf "Commands: build (default), open\n"; \
			exit 1;; \
	esac

# ── Cleanup ──────────────────────────────────────────────────────────
.PHONY: clean
clean:
	@printf "$(BLUE)→ Cleaning build artifacts...$(NC)\n"
	$(CARGO) clean
	rm -rf coverage/
	@printf "$(GREEN)✓ Clean complete$(NC)\n"

# ── Benchmarks ───────────────────────────────────────────────────────
# Datasets (inputs) live in data/; results live in target/benchmarks/.
# The release binary is what gets measured, and the feature set it was built
# with is passed through to the run manifest so the two cannot disagree.
#
# A full run does the whole pipeline per dataset rather than each stage across
# all datasets, so an interrupted batch still leaves complete, scored results
# for the datasets it finished. One dataset failing does not abort the rest;
# the failures are collected and reported at the end.
.PHONY: bench
bench:
	@datasets=$$($(BENCH)/download_beir.py --resolve $(dataset)); \
	if [ -z "$$datasets" ]; then printf "$(RED)✗ no datasets selected$(NC)\n"; exit 1; fi; \
	case "$(cmd)" in \
		setup) \
			printf "$(BLUE)→ Syncing benchmark dependencies...$(NC)\n"; \
			$(UV) sync;; \
		list) \
			$(BENCH)/download_beir.py --list;; \
		download) \
			for d in $$datasets; do \
				printf "$(BLUE)→ Downloading $$d...$(NC)\n"; \
				$(BENCH)/download_beir.py $$d; \
			done;; \
		adapt) \
			for d in $$datasets; do \
				printf "$(BLUE)→ Building the $$d vault...$(NC)\n"; \
				$(BENCH)/beir_to_tarn.py $$d; \
			done;; \
		search) \
			for d in $$datasets; do \
				printf "$(BLUE)→ Searching $$d...$(NC)\n"; \
				$(BENCH)/run_search.py $$d $(SEARCH_ARGS); \
			done;; \
		score) \
			for d in $$datasets; do \
				printf "$(BLUE)→ Scoring $$d...$(NC)\n"; \
				$(BENCH)/evaluate.py $$d; \
			done;; \
		ragas) \
			for d in $$datasets; do \
				printf "$(BLUE)→ Chunk-level cross-check on $$d...$(NC)\n"; \
				$(BENCH)/evaluate_ragas.py $$d --tarn-bin $(TARN_BIN); \
			done;; \
		report) \
			printf "$(BLUE)→ Rebuilding the reports...$(NC)\n"; \
			$(BENCH)/report.py;; \
		clean) \
			printf "$(BLUE)→ Removing benchmark datasets and results...$(NC)\n"; \
			rm -rf target/benchmarks data; \
			printf "$(GREEN)✓ Benchmark data cleared$(NC)\n";; \
		""|all) \
			printf "$(BLUE)→ Retrieval eval on:$(NC) $$datasets\n"; \
			$(CARGO) build --release $(CARGO_FEATURES); \
			$(UV) sync --quiet; \
			failed=""; \
			for d in $$datasets; do \
				printf "$(BLUE)→ [$$d] full pipeline...$(NC)\n"; \
				( set -e; \
				  $(BENCH)/download_beir.py $$d; \
				  $(BENCH)/beir_to_tarn.py $$d; \
				  $(BENCH)/run_search.py $$d $(SEARCH_ARGS); \
				  $(BENCH)/evaluate.py $$d; \
				  $(BENCH)/evaluate_ragas.py $$d --tarn-bin $(TARN_BIN); \
				) || { printf "$(RED)✗ $$d failed$(NC)\n"; failed="$$failed $$d"; }; \
			done; \
			$(BENCH)/report.py; \
			if [ -n "$$failed" ]; then \
				printf "$(RED)✗ failed:$(NC)$$failed\n"; \
				printf "$(YELLOW)Reports cover the datasets that completed.$(NC)\n"; \
				exit 1; \
			fi; \
			printf "$(GREEN)✓ Report: target/benchmarks/report.md$(NC)\n";; \
		*) \
			printf "$(RED)✗ Unknown cmd '$(cmd)'$(NC)\n"; \
			printf "Commands: all (default), setup, list, download, adapt, search, score, ragas, report, clean\n"; \
			exit 1;; \
	esac

# ── CI ───────────────────────────────────────────────────────────────
.PHONY: ci
ci:
	@case "$(cmd)" in \
		quick) \
			printf "$(BLUE)→ Running quick CI...$(NC)\n"; \
			$(CARGO) check; \
			$(CARGO) clippy -- -D warnings; \
			$(CARGO) test;; \
		""|full) \
			printf "$(BLUE)→ Running full CI pipeline...$(NC)\n"; \
			$(CARGO) fmt -- --check; \
			$(CARGO) clippy -- -D warnings; \
			$(CARGO) test; \
			$(CARGO) build --release;; \
		*) \
			printf "$(RED)✗ Unknown cmd '$(cmd)'$(NC)\n"; \
			printf "Commands: full (default), quick\n"; \
			exit 1;; \
	esac
	@printf "$(GREEN)✓ CI complete$(NC)\n"
