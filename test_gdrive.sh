#!/bin/bash
set -e

DB_PATH="./omnisync_test.db"
rm -f $DB_PATH

echo "1. Building CLI..."
cargo build -p omnisync-cli

echo "2. Testing Login..."
# simulating a token
cargo run -p omnisync-cli -- --db-path $DB_PATH login --provider gdrive --token "oauth2_fake_token_123456"

echo "3. Testing Add Sync Pair..."
mkdir -p ./test_gdrive_folder
cargo run -p omnisync-cli -- --db-path $DB_PATH add --local "$(pwd)/test_gdrive_folder" --remote "/remote_folder" --provider gdrive

echo "4. Testing Daemon Start (running for 5 seconds)..."
timeout 5s cargo run -p omnisync-cli -- --db-path $DB_PATH daemon || true

echo "Test sequence completed successfully!"
