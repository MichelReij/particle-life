#!/bin/bash

# Bluetooth Auto-Fix for Auto-Login Issues
# This script ensures Bluetooth works properly after automatic login
# No sudo commands to avoid keyring popups

echo "🔧 Fixing Bluetooth after auto-login..."

# Wait for system to fully boot
sleep 10

# Check if Bluetooth controller is available
if ! bluetoothctl show >/dev/null 2>&1; then
    echo "⚡ Bluetooth controller not ready, trying user-level fixes..."
    
    # Try to reset Bluetooth without sudo
    rfkill block bluetooth 2>/dev/null
    sleep 1
    rfkill unblock bluetooth 2>/dev/null
    sleep 3
    
    echo "🔄 If this doesn't work, run bluetooth-autofix-sudo.sh manually"
fi

# Wait for controller to be ready
for i in {1..10}; do
    if bluetoothctl show >/dev/null 2>&1; then
        echo "✅ Bluetooth controller ready"
        break
    fi
    echo "⏳ Waiting for Bluetooth controller... ($i/10)"
    sleep 2
done

# Try to reconnect HAMA keyboard
echo "⌨️ Reconnecting HAMA bluetooth keyboard..."
bluetoothctl connect A1:88:EA:D8:EC:8B >/dev/null 2>&1 || echo "🔄 Keyboard will reconnect when activated"

echo "🎯 Bluetooth auto-fix completed"
