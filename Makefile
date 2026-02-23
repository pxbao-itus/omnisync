.PHONY: dev build build-mac build-win build-linux build-all clean install-deps help setup-targets

# Default target
help:
	@echo "OmniSync Makefile"
	@echo "Local Development:"
	@echo "  make dev           Start the GUI in development mode"
	@echo "  make clean         Remove build artifacts"
	@echo ""
	@echo "Production Builds (Current OS):"
	@echo "  make build         Build the production bundle for your current OS"
	@echo ""
	@echo "Cross-Platform Builds (requires targets & toolchains):"
	@echo "  make build-mac     Build Universal macOS bundle (.app, .dmg)"
	@echo "  make build-win     Build Windows executable (.msi, .exe)"
	@echo "  make build-linux   Build Linux bundle (.AppImage, .deb)"
	@echo "  make build-all     Build for all platforms"
	@echo ""
	@echo "Utility:"
	@echo "  make setup-targets Install Rust targets for cross-compilation"
	@echo "  make install-deps  Install system dependencies (Linux only)"

# Run development mode
dev:
	cargo tauri dev

# Build production bundle for current OS
build:
	cargo tauri build

# macOS Universal (Intel + Apple Silicon)
build-mac:
	@echo "Building for macOS (Universal)..."
	cargo tauri build --target universal-apple-darwin

# Windows (requires x86_64-pc-windows-msvc target)
# Note: Usually requires running on Windows or having a cross-toolchain like 'xwin'
build-win:
	@echo "Building for Windows..."
	cargo tauri build --target x86_64-pc-windows-msvc

# Linux (requires x86_64-unknown-linux-gnu target)
# Note: Ideally run on Linux or via Docker
build-linux:
	@echo "Building for Linux..."
	cargo tauri build --target x86_64-unknown-linux-gnu

# Build for all platforms
build-all: build-mac build-win build-linux

# Setup Rust targets
setup-targets:
	rustup target add aarch64-apple-darwin
	rustup target add x86_64-apple-darwin
	rustup target add x86_64-pc-windows-msvc
	rustup target add x86_64-unknown-linux-gnu

# Clean build artifacts
clean:
	cargo clean
	rm -rf target/
	rm -rf omnisync-gui/dist/

# Install Linux system dependencies
install-deps:
	sudo apt-get update
	sudo apt-get install -y \
		libgtk-3-dev \
		libwebkit2gtk-4.1-dev \
		libayatana-appindicator3-dev \
		librsvg2-dev \
		libsoup-3.0-dev \
		pkg-config \
		build-essential \
		curl \
		wget \
		file \
		libssl-dev \
		libgtk-3-dev \
		libayatanaindicator3-dev

