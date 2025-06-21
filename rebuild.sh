#!/bin/bash

# Complete rebuild script for particle-life project
# This ensures WASM module is rebuilt and caches are cleared

echo "Cleaning and rebuilding particle-life project..."

# Stop any running webpack dev server
echo "⏹Stopping webpack dev server..."
pkill -f "webpack serve" 2>/dev/null || true
pkill -f "node.*webpack" 2>/dev/null || true
lsof -ti:3001 | xargs kill -9 2>/dev/null || true
sleep 2

# Clean build artifacts
echo "Cleaning build artifacts..."
rm -rf public/*.wasm public/*.js public/*.css public/*.html 2>/dev/null || true

# Rebuild WASM module
echo "Rebuilding WASM module..."
wasm-pack build --target web --out-dir src/pkg

# Rebuild webpack
echo "Rebuilding webpack..."
npm run build

# Start dev server
echo "🚀 Starting fresh dev server..."
npm start

echo "Complete rebuild finished!"
