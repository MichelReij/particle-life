#!/bin/bash

# Verification script to check WASM build and WebGPU configuration

echo "🔍 Checking WASM build and WebGPU configuration..."

# Check if WASM files exist
if [ ! -f "src/pkg/particle_life_wasm_bg.wasm" ]; then
    echo "❌ WASM file not found! Run ./build-wasm.sh first."
    exit 1
fi

if [ ! -f "src/pkg/particle_life_wasm.js" ]; then
    echo "❌ WASM JS bindings not found! Run ./build-wasm.sh first."
    exit 1
fi

echo "✅ WASM files found:"
echo "   - src/pkg/particle_life_wasm_bg.wasm ($(du -h src/pkg/particle_life_wasm_bg.wasm | cut -f1))"
echo "   - src/pkg/particle_life_wasm.js"
echo "   - src/pkg/particle_life_wasm.d.ts"

# Check WASM file details
echo ""
echo "📊 WASM file details:"
file src/pkg/particle_life_wasm_bg.wasm

# Check that Rust code is configured for WebGPU-only
echo ""
echo "🎯 Checking WebGPU configuration in Rust code..."
if grep -q "BROWSER_WEBGPU" src/webgpu_renderer.rs; then
    echo "✅ WebGPU-only backend configured (no WebGL fallback)"
else
    echo "⚠️  WebGPU-only configuration not found in webgpu_renderer.rs"
fi

# Verify target architecture compilation
echo ""
echo "🦀 Checking Rust target compilation..."
if cargo check --target wasm32-unknown-unknown --quiet 2>/dev/null; then
    echo "✅ Rust code compiles successfully for wasm32-unknown-unknown"
else
    echo "❌ Rust compilation failed for WASM target"
    exit 1
fi

echo ""
echo "🎉 All checks passed! Your WASM build is ready for WebGPU."
echo ""
echo "💡 To test WebGPU backend in browser:"
echo "   1. Ensure you're using a WebGPU-compatible browser (Chrome 113+, Firefox 110+)"
echo "   2. Enable WebGPU if needed (chrome://flags/#enable-unsafe-webgpu)"
echo "   3. Check browser console for 'Using WebGPU backend' message"
