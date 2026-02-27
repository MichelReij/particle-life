#!/usr/bin/env python3
# Use with virtual environment: source venv/bin/activate && python3 esp32_simulator.py
"""
ESP32 Simulator for Particle Life Simulation
Sends sensor data packets to the Rust simulation via serial port

Protocol: 17 bytes per packet
[0xAA] [zoom_high] [zoom_low] [pan_x_high] [pan_x_low] [pan_y_high] [pan_y_low]
[temp_high] [temp_low] [pressure_high] [pressure_low] [ph_high] [ph_low]
[electrical_high] [electrical_low] [sleep] [0x55]

All sensor values are u16 (0-4096), sleep is bool (0/1)
"""

import serial
import time
import struct
import sys
import argparse
from threading import Thread, Event

class ESP32Simulator:
    def __init__(self, port='/dev/ttyUSB0', baudrate=115200):
        self.port = port
        self.baudrate = baudrate
        self.serial_conn = None
        self.running = False

        # Default sensor values (middle range)
        self.zoom = 2048        # 0-4096 (maps to 1.0-50.0x zoom)
        self.pan_x = 2048       # 0-4096 (maps to world coordinates)
        self.pan_y = 2048       # 0-4096 (maps to world coordinates)
        self.temperature = 820  # 0-4096 (maps to 3-130°C, 820 ≈ 20°C)
        self.pressure = 0       # 0-4096 (maps to 0-350)
        self.ph = 2926          # 0-4096 (maps to 0-14; ~pH 10 default)
        self.electrical = 0     # 0-4096 (maps to 0-3.0)
        self.sleep = False      # bool

    def connect(self):
        """Connect to serial port"""
        try:
            self.serial_conn = serial.Serial(
                port=self.port,
                baudrate=self.baudrate,
                timeout=1,
                write_timeout=1
            )
            print(f"✅ Connected to {self.port} at {self.baudrate} baud")
            return True
        except Exception as e:
            print(f"❌ Failed to connect to {self.port}: {e}")
            return False

    def disconnect(self):
        """Disconnect from serial port"""
        if self.serial_conn and self.serial_conn.is_open:
            self.serial_conn.close()
            print("🔌 Disconnected")

    def create_packet(self):
        """Create ESP32 protocol packet"""
        # Clamp values to valid range
        zoom = max(0, min(4096, self.zoom))
        pan_x = max(0, min(4096, self.pan_x))
        pan_y = max(0, min(4096, self.pan_y))
        temperature = max(0, min(4096, self.temperature))
        pressure = max(0, min(4096, self.pressure))
        ph = max(0, min(4096, self.ph))
        electrical = max(0, min(4096, self.electrical))

        # Build packet: 17 bytes total
        packet = bytearray()
        packet.append(0xAA)  # Start marker

        # Add u16 values as big-endian bytes
        packet.extend(struct.pack('>H', zoom))
        packet.extend(struct.pack('>H', pan_x))
        packet.extend(struct.pack('>H', pan_y))
        packet.extend(struct.pack('>H', temperature))
        packet.extend(struct.pack('>H', pressure))
        packet.extend(struct.pack('>H', ph))
        packet.extend(struct.pack('>H', electrical))

        packet.append(1 if self.sleep else 0)  # Sleep flag
        packet.append(0x55)  # End marker

        return packet

    def send_packet(self):
        """Send one packet to simulation"""
        if not self.serial_conn or not self.serial_conn.is_open:
            return False

        try:
            packet = self.create_packet()
            self.serial_conn.write(packet)
            self.serial_conn.flush()
            return True
        except Exception as e:
            print(f"❌ Send error: {e}")
            return False

    def run_continuous(self, frequency=60):
        """Send packets continuously at specified frequency (Hz)"""
        self.running = True
        interval = 1.0 / frequency

        print(f"📡 Sending packets at {frequency}Hz (every {interval:.3f}s)")
        print("Press Ctrl+C to stop")

        try:
            while self.running:
                if not self.send_packet():
                    print("❌ Failed to send packet, stopping...")
                    break
                time.sleep(interval)
        except KeyboardInterrupt:
            print("\n🛑 Stopped by user")
        finally:
            self.running = False

    def interactive_mode(self):
        """Interactive mode to adjust values in real-time"""
        print("\n🎮 Interactive Mode")
        print("Commands:")
        print("  zoom <value>        Set zoom (0-4096)")
        print("  pan <x> <y>         Set pan coordinates (0-4096 each)")
        print("  temp <value>        Set temperature (0-4096)")
        print("  pressure <value>    Set pressure (0-4096)")
        print("  ph <value>          Set pH (0-4096, maps to 0-14; optimum ~2926=pH10)")
        print("  electrical <value>  Set electrical (0-4096)")
        print("  sleep <on/off>      Set sleep mode")
        print("  status              Show current values")
        print("  quit                Exit")
        print()

        # Start sending thread
        send_thread = Thread(target=self.run_continuous, args=(60,))
        send_thread.daemon = True
        send_thread.start()

        try:
            while True:
                try:
                    cmd = input("> ").strip().lower().split()
                    if not cmd:
                        continue

                    if cmd[0] == 'quit':
                        break
                    elif cmd[0] == 'zoom' and len(cmd) == 2:
                        self.zoom = int(cmd[1])
                        print(f"Zoom set to {self.zoom}")
                    elif cmd[0] == 'pan' and len(cmd) == 3:
                        self.pan_x = int(cmd[1])
                        self.pan_y = int(cmd[2])
                        print(f"Pan set to ({self.pan_x}, {self.pan_y})")
                    elif cmd[0] == 'temp' and len(cmd) == 2:
                        self.temperature = int(cmd[1])
                        print(f"Temperature set to {self.temperature}")
                    elif cmd[0] == 'pressure' and len(cmd) == 2:
                        self.pressure = int(cmd[1])
                        print(f"Pressure set to {self.pressure}")
                    elif cmd[0] == 'ph' and len(cmd) == 2:
                        self.ph = int(cmd[1])
                        print(f"pH set to {self.ph} (~pH {(self.ph/4096.0)*14.0:.1f})")
                    elif cmd[0] == 'electrical' and len(cmd) == 2:
                        self.electrical = int(cmd[1])
                        print(f"Electrical set to {self.electrical}")
                    elif cmd[0] == 'sleep' and len(cmd) == 2:
                        self.sleep = cmd[1] in ['on', 'true', '1']
                        print(f"Sleep set to {self.sleep}")
                    elif cmd[0] == 'status':
                        self.print_status()
                    else:
                        print("❌ Invalid command")
                except ValueError:
                    print("❌ Invalid value")
                except KeyboardInterrupt:
                    break
        finally:
            self.running = False

    def print_status(self):
        """Print current sensor values and their mappings"""
        print(f"\n📊 Current Sensor Values:")
        print(f"  Zoom: {self.zoom} (maps to {1.0 + (self.zoom/4096.0)*49.0:.1f}x)")
        print(f"  Pan: ({self.pan_x}, {self.pan_y})")
        print(f"  Temperature: {self.temperature} (maps to {3.0 + (self.temperature/4096.0)*127.0:.1f}°C)")
        print(f"  Pressure: {self.pressure} (maps to {(self.pressure/4096.0)*350.0:.1f})")
        print(f"  pH: {self.ph} (maps to pH {(self.ph/4096.0)*14.0:.1f}/14, optimum pH 10)")
        print(f"  Electrical: {self.electrical} (maps to {(self.electrical/4096.0)*3.0:.2f})")
        print(f"  Sleep: {self.sleep}")
        print()

def find_serial_ports():
    """Find available serial ports"""
    import serial.tools.list_ports
    ports = serial.tools.list_ports.comports()
    if not ports:
        print("❌ No serial ports found")
        return []

    print("📡 Available serial ports:")
    for i, port in enumerate(ports):
        print(f"  {i}: {port.device} - {port.description}")
    return [port.device for port in ports]

def main():
    parser = argparse.ArgumentParser(description='ESP32 Simulator for Particle Life')
    parser.add_argument('--port', '-p', help='Serial port (e.g. /dev/ttyUSB0, COM3)')
    parser.add_argument('--baud', '-b', type=int, default=115200, help='Baud rate (default: 115200)')
    parser.add_argument('--frequency', '-f', type=int, default=60, help='Packet frequency in Hz (default: 60)')
    parser.add_argument('--interactive', '-i', action='store_true', help='Interactive mode')
    parser.add_argument('--list-ports', '-l', action='store_true', help='List available serial ports')

    args = parser.parse_args()

    if args.list_ports:
        find_serial_ports()
        return

    # Auto-detect port if not specified
    if not args.port:
        ports = find_serial_ports()
        if not ports:
            return
        if len(ports) == 1:
            args.port = ports[0]
            print(f"🔍 Auto-selected port: {args.port}")
        else:
            print("Multiple ports found. Please specify with --port")
            return

    # Create simulator
    simulator = ESP32Simulator(args.port, args.baud)

    if not simulator.connect():
        return

    try:
        if args.interactive:
            simulator.interactive_mode()
        else:
            simulator.run_continuous(args.frequency)
    finally:
        simulator.disconnect()

if __name__ == '__main__':
    main()
