#!/bin/bash

# Build script for Rust WASM module
# This script compiles the Rust code to WebAssembly and generates TypeScript bindings

set -e

echo "🦀 Building Rust WASM module for wasm32-unknown-unknown target..."

# Install wasm-pack if not already installed
if ! command -v wasm-pack &> /dev/null; then
    echo "📦 Installing wasm-pack..."
    curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi

# Ensure WASM target is installed
rustup target add wasm32-unknown-unknown --quiet

# Build the WASM module with explicit target (incrementeel — geen cargo clean)
echo "🎯 Compiling Rust to WASM..."
wasm-pack build \
  --target web \
  --out-dir src/pkg \
  --out-name particle_life_wasm \
  --release

# Verify the build was successful
if [ ! -f "src/pkg/particle_life_wasm_bg.wasm" ]; then
    echo "❌ WASM build failed!"
    exit 1
fi

echo "✅ WASM compilation successful!"

# Optimize the generated WASM file
if command -v wasm-opt &> /dev/null; then
    echo "⚡ Optimizing WASM file..."
    wasm-opt -Oz src/pkg/particle_life_wasm_bg.wasm -o src/pkg/particle_life_wasm_bg.wasm
    echo "✅ WASM optimization complete!"
else
    echo "💡 wasm-opt not found. Consider installing binaryen for better optimization:"
    echo "   brew install binaryen   (on macOS)"
    echo "   apt install binaryen    (on Ubuntu)"
fi

echo "📋 Syncing shaders to public/shaders/..."
mkdir -p public/shaders
cp src/shaders/*.wgsl public/shaders/
echo "✅ Shaders synced!"

echo "🎉 WASM build complete!"
echo "Generated files:"
echo "   - src/pkg/particle_life_wasm.js"
echo "   - src/pkg/particle_life_wasm_bg.wasm"
echo "   - src/pkg/particle_life_wasm.d.ts"

echo ""
echo "To use the hybrid engine:"
echo "   1. Import: import { initializeParticleLeniaEngineHybrid } from './particle-lenia-hybrid';"
echo "   2. Initialize: const engine = await initializeParticleLeniaEngineHybrid(canvas);"
echo "   3. Start: startAnimation(engine);"
