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

# Benchmark settings. `dataset` picks the BEIR corpus; scifact is tier A and the
# one most BEIR baseline papers report first, so a comparable number exists.
dataset  ?= scifact
FEATURES ?= stemming
TARN_BIN := target/release/tarn-mcp
BENCH    := $(UV) run --quiet python scripts

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
	@printf "  make bench dataset=nfcorpus Full retrieval eval on another corpus\n\n"
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
.PHONY: bench
bench:
	@case "$(cmd)" in \
		setup) \
			printf "$(BLUE)→ Syncing benchmark dependencies...$(NC)\n"; \
			$(UV) sync;; \
		list) \
			$(BENCH)/download_beir.py --list;; \
		download) \
			printf "$(BLUE)→ Downloading $(dataset)...$(NC)\n"; \
			$(BENCH)/download_beir.py $(dataset);; \
		adapt) \
			printf "$(BLUE)→ Building the $(dataset) vault...$(NC)\n"; \
			$(BENCH)/beir_to_tarn.py $(dataset);; \
		search) \
			printf "$(BLUE)→ Searching $(dataset)...$(NC)\n"; \
			$(BENCH)/run_search.py $(dataset) --tarn-bin $(TARN_BIN) --features "$(FEATURES)";; \
		score) \
			printf "$(BLUE)→ Scoring $(dataset)...$(NC)\n"; \
			$(BENCH)/evaluate.py $(dataset);; \
		ragas) \
			printf "$(BLUE)→ Chunk-level cross-check on $(dataset)...$(NC)\n"; \
			$(BENCH)/evaluate_ragas.py $(dataset) --tarn-bin $(TARN_BIN);; \
		report) \
			printf "$(BLUE)→ Rebuilding the report...$(NC)\n"; \
			$(BENCH)/report.py;; \
		clean) \
			printf "$(BLUE)→ Removing benchmark datasets and results...$(NC)\n"; \
			rm -rf target/benchmarks data; \
			printf "$(GREEN)✓ Benchmark data cleared$(NC)\n";; \
		""|all) \
			printf "$(BLUE)→ Full retrieval eval on $(dataset)...$(NC)\n"; \
			$(CARGO) build --release; \
			$(UV) sync --quiet; \
			$(BENCH)/download_beir.py $(dataset); \
			$(BENCH)/beir_to_tarn.py $(dataset); \
			$(BENCH)/run_search.py $(dataset) --tarn-bin $(TARN_BIN) --features "$(FEATURES)"; \
			$(BENCH)/evaluate.py $(dataset); \
			$(BENCH)/evaluate_ragas.py $(dataset) --tarn-bin $(TARN_BIN); \
			$(BENCH)/report.py; \
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
