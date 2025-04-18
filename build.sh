#!/bin/bash

set -e

echo "🔨 Building Rust binary..."
cargo build --release

echo "📦 Rebuilding and restarting Docker containers..."
docker compose build
docker compose up -d
