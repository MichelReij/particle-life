# ESP32 Communication API

## Overview

The particle life simulation now supports real-time parameter control via ESP32 microcontroller. The ESP32 sends sensor data over serial communication to control all simulation parameters.

## API Specification

### Data Format

The ESP32 sends a 17-byte packet every ~16ms (60 FPS):

```
[0xAA] [zoom_high] [zoom_low] [pan_x_high] [pan_x_low] [pan_y_high] [pan_y_low]
[temp_high] [temp_low] [pressure_high] [pressure_low] [uv_high] [uv_low]
[electrical_high] [electrical_low] [sleep] [0x55]
```

### Parameters

| Parameter | Type | Range | Simulation Effect |
|-----------|------|--------|------------------|
| `zoom` | u16 | 0-4096 | Zoom level: 1.0x to 50.0x |
| `pan_x` | u16 | 0-4096 | Pan X: 0 to virtual_world_width |
| `pan_y` | u16 | 0-4096 | Pan Y: 0 to virtual_world_height |
| `temperature` | u16 | 0-4096 | Temperature: 3°C to 130°C |
| `pressure` | u16 | 0-4096 | Pressure: 0 to 350 units |
| `uv` | u16 | 0-4096 | UV Light: 0 to 50 units |
| `electrical` | u16 | 0-4096 | Electrical Activity: 0 to 3.0 |
| `sleep` | bool | 0/1 | Sleep mode: false/true |

### Parameter Effects

#### Temperature (3°C - 130°C)
- **Drift Speed**: Higher temperature = faster particle drift
- **Friction**: Higher temperature = lower friction (particles move more freely)
- **Background Color**: Cold (blue) → Hot (red/orange)

#### Pressure (0 - 350 units)
- **Force Scale**: Higher pressure = stronger particle interactions
- **R Smooth**: Higher pressure = sharper force transitions

#### UV Light (0 - 50 units)
- **Inter-Type Radius Scale**: Higher UV = larger interaction radii
- **Lenia Kernel Radius**: Higher UV = larger growth patterns

#### Electrical Activity (0 - 3.0)
- **Inter-Type Attraction Scale**: Higher electrical = stronger attractions (cubic scaling)
- **Lightning Parameters**: Controls frequency, intensity, and duration

#### Zoom & Pan
- **Zoom**: 1.0x (full world view) to 50.0x (highly zoomed)
- **Pan**: Moves the viewport center within the virtual world

#### Sleep Mode
- **Effect**: Activates power-saving mode (implementation TBD)

## Connection Management

### Auto-Detection
- Scans all serial ports every second when disconnected
- Looks for common ESP32 USB-to-serial chips:
  - CH340, CP210x, FTDI, Silicon Labs chips
  - Ports containing "USB", "ESP32" in description

### Communication Thread
- Runs in separate thread to avoid blocking graphics
- Non-blocking parameter updates
- Automatic reconnection on disconnect
- 115200 baud rate, 100ms timeout

### Error Handling
- **PortNotFound**: ESP32 not detected, using default values
- **ConnectionLost**: ESP32 disconnected, attempt reconnection
- **InvalidData**: Bad packet format, ignore and continue
- **ReadTimeout**: Normal during polling, no action needed

## Usage

### In Native Application

```rust
use particle_life_wasm::*;

// Initialize ESP32 manager
let esp32_manager = ESP32Manager::new();

// In render loop (non-blocking)
match esp32_manager.get_sensor_data() {
    Ok(sensor_data) => {
        // Apply to simulation parameters
        simulation_params.apply_esp32_sensor_data(&sensor_data);

        // Handle sleep mode
        if sensor_data.sleep {
            // Implement sleep mode behavior
        }
    }
    Err(ESP32Error::PortNotFound) => {
        // ESP32 not connected, continue with defaults
    }
    Err(err) => {
        // Handle other errors
        println!("ESP32 error: {:?}", err);
    }
}
```

### Testing Without Hardware

```rust
// Test conversion functions
test_esp32_sensor_data_conversion();

// Create test data
let test_data = ESP32SensorData::test_data();
test_data.log_converted_values();
```

## ESP32 Firmware Requirements

The ESP32 should:

1. **Read sensors** at ~60 FPS
2. **Send 17-byte packets** continuously:
   - Start byte: `0xAA`
   - 7 × u16 sensor values (big-endian)
   - 1 × u8 sleep flag
   - End byte: `0x55`
3. **Validate ranges**: All u16 values must be 0-4096
4. **Use 115200 baud rate** for serial communication

## Architecture Benefits

- **Non-blocking**: ESP32 communication runs in separate thread
- **Fault-tolerant**: Automatic detection and reconnection
- **Hot-pluggable**: Can connect/disconnect ESP32 during runtime
- **Consistent scaling**: Same parameter conversion as UI sliders
- **Real-time**: ~60 FPS sensor updates for smooth control

## Status Monitoring

The system provides real-time status information:

```rust
let status = esp32_manager.get_status();
let time_since_update = esp32_manager.time_since_last_update();

match status {
    ESP32Status::Disconnected => println!("Searching for ESP32..."),
    ESP32Status::Connecting => println!("Connecting to ESP32..."),
    ESP32Status::Connected => println!("ESP32 connected successfully"),
    ESP32Status::Error(err) => println!("ESP32 error: {:?}", err),
}
```
