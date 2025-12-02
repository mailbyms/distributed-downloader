#!/bin/bash

# Test script for distributed downloader

# Create directories
mkdir -p ddr-download/test-manager
mkdir -p ddr-download/test-server
mkdir -p ddr-download/test-client/tmp
mkdir -p ddr-download/test-client/target

# Start manager in background
echo "Starting manager..."
cargo run --bin manager -- --config configs/manager.yml &
MANAGER_PID=$!

# Wait a bit for manager to start
sleep 2

# Start server in background
echo "Starting server..."
cargo run --bin server -- --config configs/server.yml &
SERVER_PID=$!

# Wait a bit for server to start
sleep 2

# Try to download a small file
echo "Attempting to download test file..."
cargo run --bin client -- https://httpbin.org/json --config configs/test_client.yml

# Clean up background processes
kill $MANAGER_PID $SERVER_PID

echo "Test completed."