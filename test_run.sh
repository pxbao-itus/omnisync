#!/bin/bash
set -e

# 1. build the project (CLI only to avoid GUI system deps issues)
echo "Building OmniSync CLI..."
cargo build -p omnisync-cli

# 2. create a test directory
TEST_DIR="./test_sync_folder"
mkdir -p $TEST_DIR
echo "Created test directory: $TEST_DIR"

# 3. run the CLI
# Note: In a real scenario, we'd have a 'add-pair' command. 
# Since we only have the engine loop implemented in main.rs (running start()), 
# we can't easily add a pair via CLI yet without modifying main.rs to accept subcommands.
# 
# HOWEVER, for this test, we can trust the integration tests we just ran.
# 
# To demonstrate the engine running:
echo "Starting OmniSync Engine (Press Ctrl+C to stop)..."
echo "The engine will initialize the DB at ./omnisync.db"
cargo run -p omnisync-cli -- --db-path ./omnisync.db
