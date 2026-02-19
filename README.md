# OmniSync-Engine

A high-performance, cross-platform file synchronization service built with Rust.

## Components

- **omnisync-core**: The library containing the synchronization logic and database models.
- **omnisync-cli**: A command-line interface for the engine.
- **omnisync-gui**: A Tauri-based graphical user interface.

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- System dependencies for Tauri (see [Tauri Setup Guide](https://tauri.app/v1/guides/getting-started/prerequisites))

### Linux Dependencies
```bash
sudo apt-get update
sudo apt-get install libwebkit2gtk-4.0-dev \
    build-essential \
    curl \
    wget \
    file \
    libssl-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev
```

## Getting Started

### 1. Build the Project

```bash
cargo build
```

### 2. Run Integration Tests

```bash
cargo test -p omnisync-core
```

### 3. Run the CLI

To start the sync engine (requires a path to a SQLite database file, which will be created if it doesn't exist):

```bash
cargo run -p omnisync-cli -- --db-path ./omnisync.db
```

### 4. Run the GUI

```bash
cargo tauri dev
# OR if you want to run specifically the gui package context
cd omnisync-gui
cargo tauri dev
```

## Architecture

See [architecture.md](architecture.md) for a high-level overview.
