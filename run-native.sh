#!/bin/bash

# Native build and run script for particle-life project
# This builds and runs the native_minimal binary

echo "🎯 Building and running native particle-life..."

# Clean previous native builds
echo "🧹 Cleaning native build artifacts..."
rm -rf target/debug/native_minimal 2>/dev/null || true
rm -rf target/release/native_minimal 2>/dev/null || true

# Clean Rust cache for native target (optional, for fresh build)
echo "🗑️ Cleaning Rust native cache..."
cargo clean

# Build native binary in debug mode for faster compilation
echo "🦀 Building native binary (debug mode)..."
cargo build --bin native_minimal

# Verify compilation was successful
if [ ! -f "target/debug/native_minimal" ]; then
    echo "❌ Native compilation failed!"
    exit 1
fi

echo "✅ Native binary built successfully"

# Display custom defaults that will be applied
echo ""
echo "🎯 Native will start with these custom defaults:"
echo "  🔍 Zoom: 1.45x"
echo "  🌡️ Temperature: 20.0°C"
echo "  🔧 Pressure: 200.0"
echo "  ☀️ UV Light: 40.0"
echo "  ⚡ Electrical Activity: 2.0"
echo ""

# Run the native binary
echo "🚀 Starting native particle-life application..."
echo "   (Close the window or press Ctrl+C to stop)"
echo ""

./target/debug/native_minimal

echo ""
echo "✅ Native application stopped"
