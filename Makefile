# ─── alchemy-cleaner Makefile ────────────────────────────────────────────────
#
# Common targets:
#   make            → release build (default)
#   make dev        → debug build
#   make run        → release build + run full diagnostic
#   make check      → cargo check (fast syntax/type check, no binary)
#   make lint       → clippy warnings
#   make fmt        → auto-format with rustfmt
#   make fmt-check  → check formatting without changing files (CI)
#   make test       → run unit tests
#   make clean      → remove build artefacts
#   make install    → install binary to ~/.cargo/bin
#   make uninstall  → remove installed binary
#   make list       → print available checkers
#   make help       → print this message

BINARY   := alchemy-cleaner
CARGO    := cargo
RELEASE  := target/release/$(BINARY)
DEBUG    := target/debug/$(BINARY)

.PHONY: all dev run run-no-report run-no-interactive check lint fmt fmt-check test clean install uninstall list help

# ── Default: release build ────────────────────────────────────────────────────
all: $(RELEASE)

$(RELEASE): src/**/*.rs Cargo.toml
	$(CARGO) build --release

# ── Debug build ───────────────────────────────────────────────────────────────
dev: $(DEBUG)

$(DEBUG): src/**/*.rs Cargo.toml
	$(CARGO) build

# ── Run full diagnostic (release) ─────────────────────────────────────────────
run: $(RELEASE)
	./$(RELEASE)

# ── Run with no report saved ──────────────────────────────────────────────────
run-no-report: $(RELEASE)
	./$(RELEASE) --no-report

# ── Run without the interactive fix-runner menu ───────────────────────────────
run-no-interactive: $(RELEASE)
	./$(RELEASE) --no-interactive

# ── Cargo check (fast, no codegen) ───────────────────────────────────────────
check:
	$(CARGO) check

# ── Clippy lints ──────────────────────────────────────────────────────────────
lint:
	$(CARGO) clippy -- -D warnings

# ── Format source in-place ───────────────────────────────────────────────────
fmt:
	$(CARGO) fmt

# ── Check formatting (CI-safe, no writes) ─────────────────────────────────────
fmt-check:
	$(CARGO) fmt -- --check

# ── Unit tests ────────────────────────────────────────────────────────────────
test:
	$(CARGO) test

# ── Clean build artefacts ─────────────────────────────────────────────────────
clean:
	$(CARGO) clean

# ── Install binary to ~/.cargo/bin ────────────────────────────────────────────
install:
	$(CARGO) install --path .

# ── Remove installed binary ───────────────────────────────────────────────────
uninstall:
	$(CARGO) uninstall $(BINARY)

# ── List available checkers ───────────────────────────────────────────────────
list: $(RELEASE)
	./$(RELEASE) --list

# ── Help ──────────────────────────────────────────────────────────────────────
help:
	@echo ""
	@echo "  alchemy-cleaner — available make targets"
	@echo ""
	@echo "  make              Release build (optimised)"
	@echo "  make dev          Debug build"
	@echo "  make run              Release build + run full diagnostic"
	@echo "  make run-no-report    Run without saving a Desktop report"
	@echo "  make run-no-interactive  Run without the interactive fix-runner menu"
	@echo "  make check        Syntax/type check only (no binary)"
	@echo "  make lint         Clippy lints (warnings = errors)"
	@echo "  make fmt          Auto-format with rustfmt"
	@echo "  make fmt-check    Check formatting without changes (CI)"
	@echo "  make test         Run unit tests"
	@echo "  make clean        Remove build artefacts"
	@echo "  make install      Install binary to ~/.cargo/bin"
	@echo "  make uninstall    Remove installed binary"
	@echo "  make list         Print available checkers"
	@echo ""
