#!/bin/bash

# Bluetooth Auto-Fix with SUDO - Manual use only
# Run this manually when Bluetooth doesn't work after auto-login

echo "🔧 Manual Bluetooth fix with administrator rights..."

# Reload Bluetooth USB module
echo "🔄 Reloading Bluetooth module..."
sudo modprobe -r btusb
sleep 2
sudo modprobe btusb
sleep 3

# Restart Bluetooth service
echo "🔄 Restarting Bluetooth service..."
sudo systemctl restart bluetooth
sleep 3

# Check if controller is available
if bluetoothctl show >/dev/null 2>&1; then
    echo "✅ Bluetooth controller ready"
    
    # Try to reconnect HAMA keyboard
    echo "⌨️ Reconnecting HAMA bluetooth keyboard..."
    bluetoothctl connect A1:88:EA:D8:EC:8B
else
    echo "❌ Bluetooth controller still not available"
fi

echo "🎯 Manual Bluetooth fix completed"
