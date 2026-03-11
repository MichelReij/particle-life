#!/bin/bash

# Quick launcher for particle-life on round 1080x1080 screen
# Optimized for fast startup without rebuild

echo "🚀 Quick launching particle-life on round screen (1080x1080)..."

# Check if binary exists, if not build it quickly
if [ ! -f "target/debug/native_minimal" ]; then
    echo "🦀 Building particle-life (first time)..."
    cargo build --bin native_minimal --quiet
fi

# Set USB audio as default (no internal brom) - PipeWire method
echo "🔊 Setting USB audio adapter as default (PipeWire)..."
wpctl set-default 46
wpctl set-volume 46 65%

# Launch directly in fullscreen for round screen
echo "🎯 Starting fullscreen on round display..."
cd /home/michel/particle-life
./target/debug/native_minimal

echo "👋 Particle-life session ended"
