.PHONY: all build build-debug build-release build-no-bundle \
        build-macos build-macos-intel build-macos-arm \
        build-linux build-windows \
        dev dev-otel clean test lint fmt check \
        install install-tauri install-targets setup-hooks icons \
        release release-patch release-minor release-major \
        observability-up observability-down \
        help

# Default target
all: build

# ============================================
# Development
# ============================================

# Run Tauri in development mode (hot reload)
dev:
	cd src-tauri && cargo tauri dev

# ============================================
# Observability
# ============================================

# Start the observability stack (Grafana + Tempo + Loki + Pyroscope)
observability-up:
	docker compose -f docker-compose.observability.yml up -d
	@echo "Grafana:   http://localhost:3000"
	@echo "Run app:   make dev-otel"

# Stop the observability stack
observability-down:
	docker compose -f docker-compose.observability.yml down

# Run Tauri in dev mode with OpenTelemetry enabled
dev-otel: observability-up
	cd src-tauri && RUST_LOG=debug cargo tauri dev --features otel

# ============================================
# Build Targets
# ============================================

# Build release (default, current platform)
build: build-release

# Debug build (faster compilation, for testing)
build-debug:
	cd src-tauri && cargo tauri build --debug

# Release build (optimized, current platform)
build-release:
	cd src-tauri && cargo tauri build

# Build without bundling (just the binary)
build-no-bundle:
	cd src-tauri && cargo tauri build --no-bundle

# ============================================
# Platform-Specific Builds
# ============================================

# macOS Universal (both Intel and ARM)
build-macos: build-macos-intel build-macos-arm
	@echo "Built for both macOS architectures"

# macOS Intel (x86_64)
build-macos-intel:
	cd src-tauri && cargo tauri build --target x86_64-apple-darwin

# macOS Apple Silicon (ARM64)
build-macos-arm:
	cd src-tauri && cargo tauri build --target aarch64-apple-darwin

# Linux (x86_64) - requires Linux or cross-compilation setup
build-linux:
	@echo "Note: Linux builds require Linux machine or Docker. Use 'make release' for CI builds."
	cd src-tauri && cargo tauri build --target x86_64-unknown-linux-gnu

# Windows (x86_64) - requires Windows machine (cross-compilation not supported)
build-windows:
ifeq ($(OS),Windows_NT)
	cd src-tauri && cargo tauri build --target x86_64-pc-windows-msvc
else
	@echo "Error: Windows builds require a Windows machine."
	@echo "Cross-compilation to x86_64-pc-windows-msvc is not supported from $$(uname -s)."
	@echo "Use 'make release VERSION=vX.Y.Z' to trigger CI builds for all platforms."
	@exit 1
endif

# ============================================
# Release Management
# ============================================

# Create and push a release tag (triggers CI build for all platforms)
# Usage: make release VERSION=v0.1.0
VERSION ?= $(shell git describe --tags --abbrev=0 2>/dev/null || echo "v0.0.0")
release:
	@if [ -z "$(VERSION)" ] || [ "$(VERSION)" = "v0.0.0" ]; then \
		echo "Usage: make release VERSION=v0.1.0"; \
		exit 1; \
	fi
	@echo "Creating release $(VERSION)..."
	git tag -a $(VERSION) -m "Release $(VERSION)"
	git push origin $(VERSION)
	@echo "Release $(VERSION) created and pushed!"
	@echo "GitHub Actions will now build for all platforms."

# Bump patch version and release (v0.0.X)
release-patch:
	@CURRENT=$$(git describe --tags --abbrev=0 2>/dev/null || echo "v0.0.0"); \
	MAJOR=$$(echo $$CURRENT | sed 's/v//' | cut -d. -f1); \
	MINOR=$$(echo $$CURRENT | sed 's/v//' | cut -d. -f2); \
	PATCH=$$(echo $$CURRENT | sed 's/v//' | cut -d. -f3); \
	NEW_VERSION="v$$MAJOR.$$MINOR.$$((PATCH + 1))"; \
	echo "Bumping from $$CURRENT to $$NEW_VERSION"; \
	$(MAKE) release VERSION=$$NEW_VERSION

# Bump minor version and release (v0.X.0)
release-minor:
	@CURRENT=$$(git describe --tags --abbrev=0 2>/dev/null || echo "v0.0.0"); \
	MAJOR=$$(echo $$CURRENT | sed 's/v//' | cut -d. -f1); \
	MINOR=$$(echo $$CURRENT | sed 's/v//' | cut -d. -f2); \
	NEW_VERSION="v$$MAJOR.$$((MINOR + 1)).0"; \
	echo "Bumping from $$CURRENT to $$NEW_VERSION"; \
	$(MAKE) release VERSION=$$NEW_VERSION

# Bump major version and release (vX.0.0)
release-major:
	@CURRENT=$$(git describe --tags --abbrev=0 2>/dev/null || echo "v0.0.0"); \
	MAJOR=$$(echo $$CURRENT | sed 's/v//' | cut -d. -f1); \
	NEW_VERSION="v$$((MAJOR + 1)).0.0"; \
	echo "Bumping from $$CURRENT to $$NEW_VERSION"; \
	$(MAKE) release VERSION=$$NEW_VERSION

# ============================================
# Code Quality
# ============================================

# Run tests
test:
	cargo test
	cd web && npm test -- --run

# Lint all code
lint:
	cd web && npm run lint
	cargo clippy -- -D warnings

# Format code
fmt:
	cargo fmt
	cd web && npm run lint -- --fix

# Check formatting and lint (used for CI)
check:
	cargo fmt -- --check
	cd web && npm run lint

# ============================================
# Setup & Installation
# ============================================

# Install all dependencies
install: install-deps install-tauri install-targets

# Install frontend dependencies
install-deps:
	cd web && npm install

# Install Tauri CLI
install-tauri:
	cargo install tauri-cli --locked

# Install Rust targets for cross-compilation
install-targets:
	rustup target add x86_64-apple-darwin
	rustup target add aarch64-apple-darwin
	rustup target add x86_64-unknown-linux-gnu
	rustup target add x86_64-pc-windows-msvc

# Setup git hooks
setup-hooks:
	cp scripts/pre-commit .git/hooks/pre-commit
	chmod +x .git/hooks/pre-commit
	@echo "Git hooks installed successfully!"

# ============================================
# Utilities
# ============================================

# Clean build artifacts
clean:
	rm -rf target
	rm -rf web/dist
	rm -rf web/node_modules/.vite

# Generate app icons from a source image
# Usage: make icons ICON_SOURCE=path/to/icon.png
ICON_SOURCE ?= assets/icon.png
icons:
	@if [ ! -f "$(ICON_SOURCE)" ]; then \
		echo "Error: Icon source file not found: $(ICON_SOURCE)"; \
		echo "Usage: make icons ICON_SOURCE=path/to/icon.png"; \
		exit 1; \
	fi
	cd src-tauri && cargo tauri icon "$(abspath $(ICON_SOURCE))"

# ============================================
# Help
# ============================================

help:
	@echo "Paporg - Document Organization Desktop App"
	@echo ""
	@echo "Development:"
	@echo "  dev                - Run in development mode (hot reload)"
	@echo ""
	@echo "Build (Local):"
	@echo "  build              - Build release for current platform"
	@echo "  build-debug        - Build debug version (faster)"
	@echo "  build-release      - Build optimized release"
	@echo "  build-no-bundle    - Build binary only (no installer)"
	@echo ""
	@echo "Build (Platform-Specific):"
	@echo "  build-macos        - Build for macOS (Intel + ARM)"
	@echo "  build-macos-intel  - Build for macOS Intel (x86_64)"
	@echo "  build-macos-arm    - Build for macOS Apple Silicon (ARM64)"
	@echo "  build-linux        - Build for Linux (requires Linux/Docker)"
	@echo "  build-windows      - Build for Windows (requires Windows)"
	@echo ""
	@echo "  Note: For cross-platform builds, use 'make release' to trigger CI."
	@echo ""
	@echo "Release (CI - builds all platforms via GitHub Actions):"
	@echo "  release VERSION=vX.Y.Z  - Create and push release tag"
	@echo "  release-patch           - Bump patch version (v0.0.X)"
	@echo "  release-minor           - Bump minor version (v0.X.0)"
	@echo "  release-major           - Bump major version (vX.0.0)"
	@echo ""
	@echo "Code Quality:"
	@echo "  test               - Run all tests"
	@echo "  lint               - Lint all code"
	@echo "  fmt                - Format code"
	@echo "  check              - Check formatting and lint (CI-style)"
	@echo ""
	@echo "Setup:"
	@echo "  install            - Install all dependencies"
	@echo "  install-deps       - Install frontend dependencies"
	@echo "  install-tauri      - Install Tauri CLI"
	@echo "  install-targets    - Install Rust cross-compilation targets"
	@echo "  setup-hooks        - Install git pre-commit hooks"
	@echo ""
	@echo "Observability:"
	@echo "  observability-up   - Start Grafana + Tempo + Loki + Pyroscope stack"
	@echo "  observability-down - Stop the observability stack"
	@echo "  dev-otel           - Run dev mode with OTel (starts stack automatically)"
	@echo ""
	@echo "Utilities:"
	@echo "  clean              - Remove build artifacts"
	@echo "  icons ICON_SOURCE=path  - Generate app icons"
	@echo "  help               - Show this help"
