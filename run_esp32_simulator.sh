#!/bin/bash
# Quick ESP32 simulator launcher script
# Automatically activates virtual environment and runs the simulator

cd "$(dirname "$0")"

# Check if virtual environment exists
if [ ! -d "venv" ]; then
    echo "🔧 Creating virtual environment..."
    python3 -m venv venv
fi

# Activate virtual environment
source venv/bin/activate

# Check if pyserial is installed
if ! python3 -c "import serial" 2>/dev/null; then
    echo "📦 Installing pyserial..."
    pip install pyserial
fi

# Run the simulator with all provided arguments
echo "🚀 Starting ESP32 simulator..."
python3 esp32_simulator.py "$@"
