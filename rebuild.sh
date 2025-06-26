#!/bin/bash

# Complete rebuild script for particle-life project
# This ensures WASM module is rebuilt and caches are cleared

echo "🎯 Cleaning and rebuilding particle-life project for WASM..."

# Stop any running webpack dev server
echo "⏹ Stopping webpack dev server..."
pkill -f "webpack serve" 2>/dev/null || true
pkill -f "node.*webpack" 2>/dev/null || true
lsof -ti:3001 | xargs kill -9 2>/dev/null || true
sleep 2

# Clean build artifacts
echo "🧹 Cleaning build artifacts..."
rm -rf public/*.wasm public/*.js public/*.css public/*.html 2>/dev/null || true
rm -rf src/pkg 2>/dev/null || true
rm -rf target/wasm32-unknown-unknown 2>/dev/null || true

# Clean Rust cache for WASM target
echo "🗑️ Cleaning Rust WASM cache..."
cargo clean --target wasm32-unknown-unknown

# Ensure WASM target is installed
echo "🔧 Ensuring WASM target is installed..."
rustup target add wasm32-unknown-unknown

# Rebuild WASM module with explicit target
echo "🦀 Rebuilding WASM module for wasm32-unknown-unknown target..."
wasm-pack build \
  --target web \
  --out-dir src/pkg \
  --release \
  --scope particle-life

# Verify WASM compilation was successful
if [ ! -f "src/pkg/particle_life_wasm_bg.wasm" ]; then
    echo "❌ WASM compilation failed!"
    exit 1
fi

echo "✅ WASM module built successfully"

# Rebuild webpack
echo "📦 Rebuilding webpack..."
npm run build

# Start dev server
echo "🚀 Starting fresh dev server..."
npm start

echo "✅ Complete rebuild finished!"
