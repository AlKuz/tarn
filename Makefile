MAKEFLAGS += --warn-undefined-variables
SHELL := bash
.SHELLFLAGS := -eu -o pipefail -c
.DEFAULT_GOAL := help
.DELETE_ON_ERROR:
.SUFFIXES:

# ── Configuration ────────────────────────────────────────────────────
CARGO := cargo

GREEN  := \033[0;32m
YELLOW := \033[0;33m
BLUE   := \033[0;34m
RED    := \033[0;31m
NC     := \033[0m

# ── Help ─────────────────────────────────────────────────────────────
.PHONY: help
help:
	@printf "Usage: make <target> [cmd=<subcommand>]\n\n"
	@printf "$(BLUE)build$(NC)    Build operations (cmd=debug|release)\n"
	@printf "$(BLUE)test$(NC)     Test operations (cmd=all|unit|integration|verbose)\n"
	@printf "$(BLUE)lint$(NC)     Code quality (cmd=check|fix|fmt)\n"
	@printf "$(BLUE)coverage$(NC) Coverage reports (cmd=text|html|lcov|tarpaulin)\n"
	@printf "$(BLUE)doc$(NC)      Documentation (cmd=build|open)\n"
	@printf "$(BLUE)clean$(NC)    Remove build artifacts and coverage reports\n"
	@printf "$(BLUE)ci$(NC)       CI pipeline (cmd=full|quick)\n\n"
	@printf "Examples:\n"
	@printf "  make build                  Build in debug mode (default)\n"
	@printf "  make build cmd=release      Build in release mode\n"
	@printf "  make test cmd=integration   Run integration tests\n"
	@printf "  make coverage cmd=html      Generate HTML coverage report\n"

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
		""|all) \
			printf "$(BLUE)→ Running all tests...$(NC)\n"; \
			$(CARGO) test;; \
		*) \
			printf "$(RED)✗ Unknown cmd '$(cmd)'$(NC)\n"; \
			printf "Commands: all (default), unit, integration, verbose\n"; \
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

# ── Coverage ─────────────────────────────────────────────────────────
.PHONY: coverage
coverage:
	@case "$(cmd)" in \
		html) \
			printf "$(BLUE)→ Generating HTML coverage report...$(NC)\n"; \
			mkdir -p coverage; \
			$(CARGO) llvm-cov --all-features --html --output-dir coverage; \
			printf "$(GREEN)✓ Report: coverage/html/index.html$(NC)\n";; \
		lcov) \
			printf "$(BLUE)→ Generating LCOV coverage report...$(NC)\n"; \
			mkdir -p coverage; \
			$(CARGO) llvm-cov --all-features --lcov --output-path coverage/lcov.info; \
			printf "$(GREEN)✓ Report: coverage/lcov.info$(NC)\n";; \
		tarpaulin) \
			printf "$(BLUE)→ Running tarpaulin coverage...$(NC)\n"; \
			$(CARGO) tarpaulin --config tarpaulin.toml; \
			printf "$(GREEN)✓ Report: coverage/tarpaulin-report.html$(NC)\n";; \
		""|text) \
			printf "$(BLUE)→ Running tests with coverage...$(NC)\n"; \
			$(CARGO) llvm-cov --all-features; \
			printf "$(GREEN)✓ Coverage complete$(NC)\n";; \
		*) \
			printf "$(RED)✗ Unknown cmd '$(cmd)'$(NC)\n"; \
			printf "Commands: text (default), html, lcov, tarpaulin\n"; \
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
