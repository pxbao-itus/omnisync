.PHONY: dev build clean install-deps help

# Default target
help:
	@echo "OmniSync Makefile"
	@echo "Usage:"
	@echo "  make dev           Start the GUI in development mode"
	@echo "  make build         Build the production desktop application"
	@echo "  make clean         Remove build artifacts"
	@echo "  make install-deps  Install system dependencies (Linux)"

# Run development mode
dev:
	cargo tauri dev

# Build production bundle
build:
	cargo tauri build

# Clean build artifacts
clean:
	cargo clean
	rm -rf target/

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
