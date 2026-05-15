#!/usr/bin/env bash
set -euo pipefail

echo "🌍 Agent World — Development Setup"
echo "===================================="

# Check prerequisites
command -v rustc >/dev/null 2>&1 || { echo "❌ Rust not found. Install: https://rustup.rs"; exit 1; }
command -v python3 >/dev/null 2>&1 || { echo "❌ Python 3.11+ not found."; exit 1; }
command -v node >/dev/null 2>&1 || { echo "❌ Node.js not found."; exit 1; }
command -v protoc >/dev/null 2>&1 || echo "⚠️  protoc not found. Install: brew install protobuf"

echo "✅ Prerequisites met"
echo ""

# Rust dependencies
echo "🦀 Fetching Rust dependencies..."
cd world-engine && cargo fetch && cd ..

# Python dependencies
echo "🐍 Installing Python dependencies..."
cd agent-runtime && uv pip install -e ".[dev]" 2>/dev/null || pip install -e ".[dev]" && cd ..

# Dashboard dependencies
echo "🖥️  Installing Dashboard dependencies..."
cd dashboard && npm install && cd ..

# Generate protobuf
echo "📦 Generating protobuf code..."
make proto 2>/dev/null || echo "⚠️  protoc not available, skipping"

echo ""
echo "✅ Setup complete! Run 'make dev' to start."
