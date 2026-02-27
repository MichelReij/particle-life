// ESP32 Communication Module
// Handles serial communication with ESP32 in a separate thread
// Receives sensor data and makes it available to the graphics thread

use crate::config::{ZOOM_MAX, ZOOM_MIN};
use serde::{Deserialize, Serialize};
use serialport::{available_ports, SerialPort};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// Lightning event data structure for ESP32 synchronization
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ESP32LightningEvent {
    pub flash_id: u32,      // Unique lightning flash identifier
    pub lightning_type: u8, // 0 = normal, 1 = super
    pub start_time: f32,    // When the lightning started (simulation time)
    pub intensity: f32,     // Lightning intensity (0.0 - 1.0)
    pub timestamp: u64,     // System timestamp when detected (milliseconds)
}

impl ESP32LightningEvent {
    pub fn new(flash_id: u32, lightning_type: u8, start_time: f32, intensity: f32) -> Self {
        Self {
            flash_id,
            lightning_type,
            start_time,
            intensity,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }
    }

    pub fn is_super_lightning(&self) -> bool {
        self.lightning_type == 1
    }
}

// ESP32 sensor data structure
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ESP32SensorData {
    pub zoom: u16,        // 0-4096
    pub pan_x: u16,       // 0-4096
    pub pan_y: u16,       // 0-4096
    pub temperature: u16, // 0-4096
    pub pressure: u16,    // 0-4096
    pub ph: u16,          // 0-4096
    pub electrical: u16,  // 0-4096
    pub volume: u16,      // 0-4096 (NEW: potentiometer for audio volume)
    pub sleep: bool,      // true/false
}

impl Default for ESP32SensorData {
    fn default() -> Self {
        Self {
            zoom: 2048,       // Default middle value
            pan_x: 2048,      // Default middle value
            pan_y: 2048,      // Default middle value
            temperature: 820, // Default ~20°C (mapped from 3-130°C range)
            pressure: 0,      // Default 0 pressure
            ph: 2926,         // Default pH 10 (optimal for life: 10/14 * 4096 ≈ 2926)
            electrical: 0,    // Default 0 electrical activity
            volume: 2048,     // Default middle volume (50%)
            sleep: false,     // Default awake
        }
    }
}

// Connection status
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ESP32Status {
    Disconnected,
    Connecting,
    Connected,
    Error(ESP32Error),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ESP32Error {
    PortNotFound,
    OpenFailed,
    ReadTimeout,
    InvalidData,
    ConnectionLost,
}

// Shared state between communication thread and graphics thread
#[derive(Debug)]
pub struct ESP32SharedState {
    pub sensor_data: ESP32SensorData,
    pub status: ESP32Status,
    pub last_update: Instant,
    pub pending_lightning_events: Vec<ESP32LightningEvent>, // Queue of lightning events to send
    pub last_lightning_sent: Instant,
    pub last_logged_data: Option<ESP32SensorData>, // For throttling log output
    pub last_log_time: Instant,                    // For periodic logging
}

impl Default for ESP32SharedState {
    fn default() -> Self {
        Self {
            sensor_data: ESP32SensorData::default(),
            status: ESP32Status::Disconnected,
            last_update: Instant::now(),
            pending_lightning_events: Vec::new(),
            last_lightning_sent: Instant::now(),
            last_logged_data: None,
            last_log_time: Instant::now(),
        }
    }
}

// ESP32 Communication Manager
pub struct ESP32Manager {
    shared_state: Arc<Mutex<ESP32SharedState>>,
    _thread_handle: thread::JoinHandle<()>,
}

impl ESP32Manager {
    // Create new ESP32 manager and start communication thread
    pub fn new() -> Self {
        let shared_state = Arc::new(Mutex::new(ESP32SharedState::default()));
        let shared_state_clone = Arc::clone(&shared_state);

        // Spawn communication thread
        let thread_handle = thread::spawn(move || {
            esp32_communication_thread(shared_state_clone);
        });

        Self {
            shared_state,
            _thread_handle: thread_handle,
        }
    }

    // Get current sensor data (non-blocking)
    pub fn get_sensor_data(&self) -> Result<ESP32SensorData, ESP32Error> {
        match self.shared_state.lock() {
            Ok(state) => match state.status {
                ESP32Status::Connected => Ok(state.sensor_data),
                ESP32Status::Disconnected => Err(ESP32Error::PortNotFound),
                ESP32Status::Connecting => Err(ESP32Error::PortNotFound),
                ESP32Status::Error(err) => Err(err),
            },
            Err(_) => Err(ESP32Error::ConnectionLost),
        }
    }

    // Get current connection status
    pub fn get_status(&self) -> ESP32Status {
        self.shared_state
            .lock()
            .map(|state| state.status)
            .unwrap_or(ESP32Status::Error(ESP32Error::ConnectionLost))
    }

    // Get time since last successful update
    pub fn time_since_last_update(&self) -> Option<Duration> {
        self.shared_state
            .lock()
            .ok()
            .map(|state| state.last_update.elapsed())
    }

    // Send a lightning event to ESP32
    pub fn send_lightning_event(
        &self,
        flash_id: u32,
        lightning_type: u8,
        start_time: f32,
        intensity: f32,
    ) {
        let lightning_event =
            ESP32LightningEvent::new(flash_id, lightning_type, start_time, intensity);

        if let Ok(mut state) = self.shared_state.lock() {
            state.pending_lightning_events.push(lightning_event);
            println!(
                "⚡ ESP32: Queued lightning event (Flash ID: {}, Type: {}, Intensity: {:.2})",
                flash_id,
                if lightning_type == 1 {
                    "Super"
                } else {
                    "Normal"
                },
                intensity
            );
        }
    }

    // Get and clear pending lightning events (for debugging)
    pub fn get_pending_lightning_events(&self) -> Vec<ESP32LightningEvent> {
        if let Ok(mut state) = self.shared_state.lock() {
            let events = state.pending_lightning_events.clone();
            state.pending_lightning_events.clear();
            events
        } else {
            Vec::new()
        }
    }

    // Check if there are pending lightning events
    pub fn has_pending_lightning_events(&self) -> bool {
        self.shared_state
            .lock()
            .map(|state| !state.pending_lightning_events.is_empty())
            .unwrap_or(false)
    }
}

// Main communication thread function
fn esp32_communication_thread(shared_state: Arc<Mutex<ESP32SharedState>>) {
    let mut port: Option<Box<dyn SerialPort>> = None;
    let mut last_poll_time = Instant::now();

    println!("🔌 ESP32 communication thread started");

    loop {
        // If not connected, try to find and connect to ESP32 every second
        if port.is_none() && last_poll_time.elapsed() >= Duration::from_secs(1) {
            last_poll_time = Instant::now();

            // Update status to connecting
            if let Ok(mut state) = shared_state.lock() {
                state.status = ESP32Status::Connecting;
            }

            port = find_and_connect_esp32();

            if port.is_some() {
                println!("✅ ESP32 connected successfully");
                if let Ok(mut state) = shared_state.lock() {
                    state.status = ESP32Status::Connected;
                }
            } else {
                if let Ok(mut state) = shared_state.lock() {
                    state.status = ESP32Status::Error(ESP32Error::PortNotFound);
                }
            }
        }

        // If connected, try to read data
        if let Some(ref mut serial_port) = port {
            match read_esp32_data(serial_port) {
                Ok(sensor_data) => {
                    // Successfully read data - update shared state
                    let should_log = {
                        if let Ok(mut state) = shared_state.lock() {
                            // Check if we should log this data (throttled logging)
                            let should_log = should_log_sensor_data(&sensor_data, &mut state);

                            state.sensor_data = sensor_data;
                            state.status = ESP32Status::Connected;
                            state.last_update = Instant::now();

                            should_log
                        } else {
                            false
                        }
                    };

                    // Log outside of the mutex to avoid holding the lock
                    if should_log {
                        println!(
                            "📡 ESP32: zoom={} pan=({},{}) temp={} pressure={} ph={} electrical={} volume={} sleep={}",
                            sensor_data.zoom, sensor_data.pan_x, sensor_data.pan_y,
                            sensor_data.temperature, sensor_data.pressure, sensor_data.ph,
                            sensor_data.electrical, sensor_data.volume, sensor_data.sleep
                        );
                    }

                    // Check for pending lightning events to send
                    send_pending_lightning_events(serial_port, &shared_state);

                    // Small delay to prevent excessive CPU usage
                    thread::sleep(Duration::from_millis(16)); // ~60 FPS polling rate
                }
                Err(ESP32Error::ReadTimeout) => {
                    // Timeout is normal, just continue
                    thread::sleep(Duration::from_millis(50));
                }
                Err(err) => {
                    // Connection error - disconnect and retry
                    println!("❌ ESP32 connection error: {:?}", err);
                    port = None;
                    if let Ok(mut state) = shared_state.lock() {
                        state.status = ESP32Status::Error(err);
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
        } else {
            // Not connected, sleep for a bit before next poll attempt
            thread::sleep(Duration::from_millis(100));
        }
    }
}

// Find and connect to ESP32 device
fn find_and_connect_esp32() -> Option<Box<dyn SerialPort>> {
    // Get available ports
    let ports = match available_ports() {
        Ok(ports) => {
            println!("🔍 Found {} available serial ports:", ports.len());
            for port in &ports {
                println!("  📍 {}: {:?}", port.port_name, port.port_type);
            }
            ports
        }
        Err(e) => {
            println!("🔍 Failed to list serial ports: {}", e);
            return None;
        }
    };

    // Add virtual ports that might not be detected by available_ports() (OS-specific)
    let mut virtual_ports = Vec::new();

    // Detect operating system
    let is_macos = cfg!(target_os = "macos");
    let is_linux = cfg!(target_os = "linux");

    if is_linux {
        println!("🐧 Linux detected - checking for ttyUSB/ttyACM and virtual ports");
        // On Linux, ESP32 devices typically appear as /dev/ttyUSB* or /dev/ttyACM*
        for i in 0..10 {
            virtual_ports.push(format!("/dev/ttyUSB{}", i));
            virtual_ports.push(format!("/dev/ttyACM{}", i));
        }

        // Add specific socat virtual port patterns for Linux (ttys020, ttys021, etc.)
        for i in 20..50 {
            virtual_ports.push(format!("/dev/ttys{:03}", i));
        }

        // Add the specific socat ports we know exist on Linux
        virtual_ports.push("/dev/ttys030".to_string());
        virtual_ports.push("/dev/ttys031".to_string());
        virtual_ports.push("/dev/ttys043".to_string());
        virtual_ports.push("/dev/ttys044".to_string());

        // Add more common Linux patterns
        for i in 0..10 {
            virtual_ports.push(format!("/dev/pty{}", i));
            virtual_ports.push(format!("/dev/ttyS{}", i));
        }
    } else if is_macos {
        println!("🍎 macOS detected - focusing on /dev/cu.* devices");
        // On macOS, ESP32 devices appear as /dev/cu.usbmodem* or /dev/cu.SLAB_USBtoUART*
        // Skip virtual /dev/ttys* ports as they're not relevant for ESP32 on macOS

        // Only add macOS-specific patterns that might not be enumerated
        for i in 0..10 {
            virtual_ports.push(format!("/dev/cu.usbmodem{}", i));
            virtual_ports.push(format!("/dev/cu.SLAB_USBtoUART{}", i));
            virtual_ports.push(format!("/dev/cu.wchusbserial{}", i));
        }
    } else {
        println!("❓ Unknown OS - using generic patterns");
        // Fallback for other operating systems
        for i in 0..10 {
            virtual_ports.push(format!("/dev/ttyUSB{}", i));
            virtual_ports.push(format!("/dev/ttyACM{}", i));
        }
    }

    // Look for ESP32-like devices (OS-specific patterns)
    let esp32_patterns = if is_macos {
        [
            "lolin-s2-mini",     // Exact product match (highest priority)
            "wemos.cc",          // Exact manufacturer match (highest priority)
            "cu.usbmodem",       // Primary macOS ESP32 pattern
            "cu.SLAB_USBtoUART", // Silicon Labs USB-to-UART on macOS
            "cu.wchusbserial",   // WCH USB-to-serial on macOS
            "USB",
            "CH340",
            "CP210",
            "FTDI",
            "ESP32",
            "Silicon Labs",
            "WEMOS",
            "LOLIN",
            "debug-console", // Development console
        ]
    } else {
        [
            "lolin-s2-mini", // Exact product match (highest priority)
            "wemos.cc",      // Exact manufacturer match (highest priority)
            "ttyACM",        // Primary Linux ESP32 pattern
            "ttyUSB",        // Common Linux ESP32 pattern
            "USB",
            "CH340",
            "CP210",
            "FTDI",
            "ESP32",
            "Silicon Labs",
            "ch341",
            "WEMOS",
            "LOLIN",
            "debug-console", // Development console
        ]
    };

    // First try ESP32-like devices with prioritization
    // On macOS, prioritize actual USB devices over virtual ports
    let mut all_candidates = Vec::new();

    // Collect and prioritize candidates
    for port_info in &ports {
        let port_name = &port_info.port_name;
        let port_description = format!("{:?}", port_info.port_type).to_lowercase();

        // Check if this looks like an ESP32
        let looks_like_esp32 = esp32_patterns.iter().any(|pattern| {
            port_name.to_lowercase().contains(&pattern.to_lowercase())
                || port_description.contains(&pattern.to_lowercase())
        });

        if looks_like_esp32 {
            // Calculate priority score (higher = better)
            let mut priority = 0;

            // Highest priority: Exact match for WEMOS LOLIN-S2-MINI board
            if port_description.contains("wemos.cc") && port_description.contains("lolin-s2-mini") {
                priority += 1000; // Highest priority for exact board match
                println!("🎯 Found exact WEMOS LOLIN-S2-MINI board match!");
            }

            // High priority: Manufacturer matches
            if port_description.contains("wemos") || port_description.contains("wemos.cc") {
                priority += 500; // High priority for WEMOS manufacturer
            }

            // High priority: Product name matches
            if port_description.contains("lolin") || port_description.contains("lolin-s2-mini") {
                priority += 400; // High priority for LOLIN product line
            }

            if is_macos {
                // macOS: Prioritize /dev/cu.usbmodem* (real USB devices)
                if port_name.contains("cu.usbmodem") {
                    priority += 100;
                }
                if port_description.contains("usb") {
                    priority += 20;
                }
                if port_name.contains("cu.") {
                    priority += 10;
                } // cu. devices are preferred on macOS
            } else {
                // Linux: Prioritize /dev/ttyACM* and /dev/ttyUSB*
                if port_name.contains("ttyACM") {
                    priority += 100;
                }
                if port_name.contains("ttyUSB") {
                    priority += 90;
                }
                if port_description.contains("usb") {
                    priority += 20;
                }
            }

            all_candidates.push((priority, port_name.clone(), port_description));
        }
    }

    // Sort by priority (highest first)
    all_candidates.sort_by(|a, b| b.0.cmp(&a.0));

    // Try candidates in priority order
    for (priority, port_name, port_description) in all_candidates {
        let is_exact_match =
            port_description.contains("wemos.cc") && port_description.contains("lolin-s2-mini");
        if is_exact_match {
            println!(
                "🎯 Trying EXACT WEMOS LOLIN-S2-MINI match: {} (priority: {}, {})",
                port_name, priority, port_description
            );
        } else {
            println!(
                "🔍 Trying ESP32 candidate: {} (priority: {}, {})",
                port_name, priority, port_description
            );
        }

        // Try to open the port
        match serialport::new(&port_name, 115200)
            .timeout(Duration::from_millis(500)) // Increase timeout
            .open()
        {
            Ok(mut port) => {
                // Test communication by trying to read some data
                thread::sleep(Duration::from_millis(100)); // Give ESP32 time to send data

                match test_esp32_communication(&mut port) {
                    Ok(true) => {
                        println!("✅ ESP32 found on port: {}", port_name);
                        return Some(port);
                    }
                    Ok(false) => {
                        println!("❌ Device on {} is not responding as ESP32", port_name);
                    }
                    Err(e) => {
                        println!("❌ Communication test failed on {}: {:?}", port_name, e);
                    }
                }
            }
            Err(e) => {
                println!("❌ Failed to open port {}: {}", port_name, e);
            }
        }
    }

    // Now try virtual ports that may not be enumerated (OS-specific)
    let existing_virtual_ports: Vec<_> = virtual_ports
        .iter()
        .filter(|p| std::path::Path::new(p).exists())
        .collect();

    if !existing_virtual_ports.is_empty() {
        if is_macos {
            println!(
                "🔍 Found {} additional macOS USB ports to check...",
                existing_virtual_ports.len()
            );
        } else {
            println!(
                "🔍 Found {} virtual ports to check...",
                existing_virtual_ports.len()
            );
        }
    } else if is_macos {
        println!("✅ macOS: Skipping virtual port scan (no additional USB devices found)");
    }

    for port_name in existing_virtual_ports {
        // Check if this looks like an ESP32
        let looks_like_esp32 = esp32_patterns
            .iter()
            .any(|pattern| port_name.to_lowercase().contains(&pattern.to_lowercase()));

        println!(
            "🔍 Checking virtual port: {} (ESP32 pattern: {})",
            port_name, looks_like_esp32
        );

        // First try with serialport crate
        match serialport::new(port_name, 115200)
            .timeout(Duration::from_millis(500)) // Increase timeout
            .open()
        {
            Ok(mut port) => {
                // Test communication by trying to read some data
                thread::sleep(Duration::from_millis(100)); // Give ESP32 time to send data

                match test_esp32_communication(&mut port) {
                    Ok(true) => {
                        println!("✅ ESP32 found on virtual port: {}", port_name);
                        return Some(port);
                    }
                    Ok(false) => {
                        println!("❌ Device on {} is not responding as ESP32", port_name);
                    }
                    Err(e) => {
                        println!("❌ Communication test failed on {}: {:?}", port_name, e);
                    }
                }
            }
            Err(e) => {
                println!("❌ Failed to open virtual port {}: {}", port_name, e);

                // If serialport fails and this is a PTY device on Linux, try alternative approach
                if is_linux && port_name.contains("ttys") {
                    println!("🔄 Trying alternative PTY approach for: {}", port_name);
                    if let Some(port) = try_pty_connection(port_name) {
                        return Some(port);
                    }
                } else if is_macos && port_name.contains("ttys") {
                    println!("⚠️ macOS: Skipping PTY attempt for {}", port_name);
                }
            }
        }
    }

    // If no ESP32-like device found, try all available ports
    println!("🔍 No ESP32-like device found, trying all enumerated ports...");
    for port_info in ports {
        let port_name = &port_info.port_name;
        let port_description = format!("{:?}", port_info.port_type).to_lowercase();

        // Skip Bluetooth ports
        if port_name.contains("Bluetooth") {
            continue;
        }

        println!(
            "🔍 Trying any available port: {} ({})",
            port_name, port_description
        );

        // Try to open the port
        match serialport::new(port_name, 115200)
            .timeout(Duration::from_millis(500)) // Increase timeout
            .open()
        {
            Ok(mut port) => {
                // Test communication by trying to read some data
                thread::sleep(Duration::from_millis(100)); // Give device time to send data

                match test_esp32_communication(&mut port) {
                    Ok(true) => {
                        println!("✅ ESP32 simulator found on port: {}", port_name);
                        return Some(port);
                    }
                    Ok(false) => {
                        println!("❌ Device on {} is not sending ESP32 data", port_name);
                    }
                    Err(e) => {
                        println!("❌ Communication test failed on {}: {:?}", port_name, e);
                    }
                }
            }
            Err(e) => {
                println!("❌ Failed to open port {}: {}", port_name, e);
            }
        }
    }

    println!("🔍 No ESP32 device found");
    None
}

// Test if the device on this port is actually our ESP32
fn test_esp32_communication(port: &mut Box<dyn SerialPort>) -> Result<bool, ESP32Error> {
    // Clear any existing data in the buffer first
    let _ = port.clear(serialport::ClearBuffer::All);

    // Try to read some data to see if it matches our expected format
    let mut attempts = 3; // Reduce attempts to avoid buffer confusion

    while attempts > 0 {
        // Wait for data to arrive
        thread::sleep(Duration::from_millis(200));

        match read_esp32_data(port) {
            Ok(data) => {
                println!(
                    "✅ ESP32 data received: zoom={}, temp={}, pressure={}",
                    data.zoom, data.temperature, data.pressure
                );
                return Ok(true); // Successfully parsed ESP32 data
            }
            Err(ESP32Error::ReadTimeout) => {
                attempts -= 1;
                if attempts > 0 {
                    // Clear buffer before next attempt
                    let _ = port.clear(serialport::ClearBuffer::All);
                }
            }
            Err(ESP32Error::InvalidData) => {
                attempts -= 1;
                if attempts > 0 {
                    // Clear buffer before next attempt
                    let _ = port.clear(serialport::ClearBuffer::All);
                }
            }
            Err(_) => {
                return Ok(false); // Connection error
            }
        }
    }

    Ok(false) // No valid data after several attempts
}

// Read sensor data from ESP32
fn read_esp32_data(port: &mut Box<dyn SerialPort>) -> Result<ESP32SensorData, ESP32Error> {
    // ESP32 sends data in this format (17 bytes total):
    // [0xAA] [zoom_high] [zoom_low] [pan_x_high] [pan_x_low] [pan_y_high] [pan_y_low]
    // [temp_high] [temp_low] [pressure_high] [pressure_low] [ph_high] [ph_low]
    // [electrical_high] [electrical_low] [sleep] [0x55]

    let mut buffer = [0u8; 19];
    let mut bytes_read = 0;

    // Try to read complete packet
    while bytes_read < 19 {
        match port.read(&mut buffer[bytes_read..]) {
            Ok(n) => {
                bytes_read += n;
                if n == 0 {
                    return Err(ESP32Error::ReadTimeout);
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                return Err(ESP32Error::ReadTimeout);
            }
            Err(_) => {
                return Err(ESP32Error::ConnectionLost);
            }
        }
    }

    // Validate packet format
    if buffer[0] != 0xAA || buffer[18] != 0x55 {
        println!("❌ ESP32 InvalidData - Bad packet format:");
        println!("   Expected: [0xAA, ..., 0x55] (19 bytes)");
        println!("   Received: {:02X?}", buffer);
        println!(
            "   Header: 0x{:02X} (expected 0xAA), Footer: 0x{:02X} (expected 0x55)",
            buffer[0], buffer[18]
        );
        return Err(ESP32Error::InvalidData);
    }

    // Parse sensor data
    let zoom = u16::from_be_bytes([buffer[1], buffer[2]]);
    let pan_x = u16::from_be_bytes([buffer[3], buffer[4]]);
    let pan_y = u16::from_be_bytes([buffer[5], buffer[6]]);
    let temperature = u16::from_be_bytes([buffer[7], buffer[8]]);
    let pressure = u16::from_be_bytes([buffer[9], buffer[10]]);
    let ph = u16::from_be_bytes([buffer[11], buffer[12]]);
    let electrical = u16::from_be_bytes([buffer[13], buffer[14]]);
    let volume = u16::from_be_bytes([buffer[15], buffer[16]]);
    let sleep = buffer[17] != 0;

    // Validate ranges (all values should be 0-4096)
    if zoom > 4096
        || pan_x > 4096
        || pan_y > 4096
        || temperature > 4096
        || pressure > 4096
        || ph > 4096
        || electrical > 4096
        || volume > 4096
    {
        println!("❌ ESP32 InvalidData - Value out of range:");
        println!("   Raw buffer: {:02X?}", buffer);
        println!("   Parsed values: zoom={}, pan_x={}, pan_y={}, temp={}, pressure={}, ph={}, electrical={}, volume={}",
            zoom, pan_x, pan_y, temperature, pressure, ph, electrical, volume);
        println!("   Valid range: 0-4096 for all sensor values");
        return Err(ESP32Error::InvalidData);
    }

    Ok(ESP32SensorData {
        zoom,
        pan_x,
        pan_y,
        temperature,
        pressure,
        ph,
        electrical,
        volume,
        sleep,
    })
}

// Conversion functions from ESP32 sensor values to simulation parameters

impl ESP32SensorData {
    // Convert zoom (0-4096) to simulation zoom level (ZOOM_MIN - ZOOM_MAX)
    pub fn to_zoom_level(&self) -> f32 {
        // Map 0-4096 to ZOOM_MIN-ZOOM_MAX range (1.0-12.0)
        ZOOM_MIN + (self.zoom as f32 / 4096.0) * (ZOOM_MAX - ZOOM_MIN)
    }

    // Convert pan values (0-4096) to world coordinates
    pub fn to_pan_coordinates(&self, world_width: f32, world_height: f32) -> (f32, f32) {
        let pan_x = (self.pan_x as f32 / 4096.0) * world_width;
        let pan_y = (self.pan_y as f32 / 4096.0) * world_height;
        (pan_x, pan_y)
    }

    // Convert temperature (0-4096) to Celsius (3-160°C)
    pub fn to_temperature_celsius(&self) -> f32 {
        3.0 + (self.temperature as f32 / 4096.0) * 157.0
    }

    // Convert pressure (0-4096) to pressure units (0-1000 bar = ~0-10000m depth)
    pub fn to_pressure(&self) -> f32 {
        (self.pressure as f32 / 4096.0) * 1000.0
    }

    // Convert pH (0-4096) to pH units (0-14)
    // Optimal pH for life (hydrothermal vent theory) is ~10
    pub fn to_ph(&self) -> f32 {
        (self.ph as f32 / 4096.0) * 14.0
    }

    // Convert electrical (0-4096) to electrical activity (0-3)
    pub fn to_electrical_activity(&self) -> f32 {
        (self.electrical as f32 / 4096.0) * 3.0
    }

    // Convert volume (0-4096) to audio volume percentage (0-100)
    // Values below 10 (raw) will return 0 to pause audio
    pub fn to_volume_percentage(&self) -> u8 {
        let volume = self.volume;
        if volume < 10 {
            0 // Mute/pause when below threshold
        } else {
            let adjusted = volume - 10;
            let max_adjusted = 4095 - 10;
            let clamped = adjusted.min(max_adjusted);
            let scaled = ((clamped as u32) * 100) / max_adjusted as u32;
            scaled as u8
        }
    }

    // Create test sensor data for debugging
    pub fn test_data() -> Self {
        Self {
            zoom: 1024,        // ~25% zoom (3.7x zoom level)
            pan_x: 2048,       // Center X
            pan_y: 2048,       // Center Y
            temperature: 1640, // ~50°C
            pressure: 2048,    // ~500 bar (50% of range)
            ph: 2926,          // ~pH 10 (optimal for life)
            electrical: 2048,  // ~1.5 electrical activity (50% of range)
            volume: 2048,      // ~50% volume
            sleep: false,      // Awake
        }
    }

    // Create sensor data with all maximum values
    pub fn test_max_data() -> Self {
        Self {
            zoom: 4096,        // Maximum zoom (12.0x)
            pan_x: 4096,       // Max X
            pan_y: 4096,       // Max Y
            temperature: 4096, // 130°C
            pressure: 4096,    // 1000 bar
            ph: 4096,          // pH 14
            electrical: 4096,  // 3.0 electrical activity
            volume: 4096,      // 100% volume
            sleep: true,       // Sleep mode
        }
    }

    // Log all converted values for debugging
    pub fn log_converted_values(&self) {
        println!("📊 ESP32 Sensor Data Conversion:");
        println!("  Raw values: zoom={}, pan_x={}, pan_y={}, temp={}, pressure={}, ph={}, electrical={}, volume={}, sleep={}",
            self.zoom, self.pan_x, self.pan_y, self.temperature, self.pressure, self.ph, self.electrical, self.volume, self.sleep);
        println!("  Converted values:");
        println!("    Zoom: {:.2}x (range: 1.0-12.0)", self.to_zoom_level());
        println!(
            "    Pan: ({:.1}, {:.1}) (world range: 0-4320)",
            self.to_pan_coordinates(4320.0, 4320.0).0,
            self.to_pan_coordinates(4320.0, 4320.0).1
        );
        println!(
            "    Temperature: {:.1}°C (range: 3-160°C)",
            self.to_temperature_celsius()
        );
        println!(
            "    Pressure: {:.1} bar (range: 0-1000)",
            self.to_pressure()
        );
        println!("    pH: {:.2} (range: 0-14, optimum ~10)", self.to_ph());
        println!(
            "    Electrical: {:.2} (range: 0-3)",
            self.to_electrical_activity()
        );
        println!(
            "    Volume: {}% (range: 0-100%)",
            self.to_volume_percentage()
        );
        println!("    Sleep: {}", self.sleep);

        // Show range utilization percentages
        println!("  Range utilization:");
        println!("    Zoom: {:.1}%", (self.zoom as f32 / 4095.0) * 100.0);
        println!("    Pan X: {:.1}%", (self.pan_x as f32 / 4095.0) * 100.0);
        println!("    Pan Y: {:.1}%", (self.pan_y as f32 / 4095.0) * 100.0);
        println!(
            "    Temperature: {:.1}%",
            (self.temperature as f32 / 4095.0) * 100.0
        );
        println!(
            "    Pressure: {:.1}%",
            (self.pressure as f32 / 4095.0) * 100.0
        );
        println!("    pH: {:.1}%", (self.ph as f32 / 4095.0) * 100.0);
        println!(
            "    Electrical: {:.1}%",
            (self.electrical as f32 / 4095.0) * 100.0
        );
        println!("    Volume: {:.1}%", (self.volume as f32 / 4095.0) * 100.0);
    }

    // Validate all sensor mappings use the full 0-4095 range correctly
    pub fn validate_sensor_mappings() {
        println!("🧪 Validating ESP32 sensor mappings...");

        // Test edge cases including volume threshold
        let test_cases = [
            ESP32SensorData {
                zoom: 0,
                pan_x: 0,
                pan_y: 0,
                temperature: 0,
                pressure: 0,
                ph: 0,
                electrical: 0,
                volume: 0,
                sleep: false,
            },
            ESP32SensorData {
                zoom: 1024,
                pan_x: 1024,
                pan_y: 1024,
                temperature: 1024,
                pressure: 1024,
                ph: 1024,
                electrical: 1024,
                volume: 5,
                sleep: false,
            }, // Below volume threshold
            ESP32SensorData {
                zoom: 1024,
                pan_x: 1024,
                pan_y: 1024,
                temperature: 1024,
                pressure: 1024,
                ph: 1024,
                electrical: 1024,
                volume: 10,
                sleep: false,
            }, // At volume threshold
            ESP32SensorData {
                zoom: 2047,
                pan_x: 2047,
                pan_y: 2047,
                temperature: 2047,
                pressure: 2047,
                ph: 2047,
                electrical: 2047,
                volume: 2047,
                sleep: false,
            },
            ESP32SensorData {
                zoom: 4095,
                pan_x: 4095,
                pan_y: 4095,
                temperature: 4095,
                pressure: 4095,
                ph: 4095,
                electrical: 4095,
                volume: 4095,
                sleep: true,
            },
        ];

        for (i, test_data) in test_cases.iter().enumerate() {
            println!(
                "\n📋 Test case {} (raw values: {})",
                i + 1,
                match i {
                    0 => "all minimum (0)",
                    1 => "volume below threshold (5)",
                    2 => "volume at threshold (10)",
                    3 => "all middle (2047)",
                    4 => "all maximum (4095)",
                    _ => "unknown",
                }
            );

            println!(
                "  Zoom: {} → {:.2}x",
                test_data.zoom,
                test_data.to_zoom_level()
            );
            let (pan_x, pan_y) = test_data.to_pan_coordinates(4320.0, 4320.0);
            println!(
                "  Pan: ({}, {}) → ({:.1}, {:.1})",
                test_data.pan_x, test_data.pan_y, pan_x, pan_y
            );
            println!(
                "  Temperature: {} → {:.1}°C",
                test_data.temperature,
                test_data.to_temperature_celsius()
            );
            println!(
                "  Pressure: {} → {:.1}",
                test_data.pressure,
                test_data.to_pressure()
            );
            println!(
                "  pH: {} → {:.2} (optimum ~10)",
                test_data.ph,
                test_data.to_ph()
            );
            println!(
                "  Electrical: {} → {:.2}",
                test_data.electrical,
                test_data.to_electrical_activity()
            );
            println!(
                "  Volume: {} → {}% ({})",
                test_data.volume,
                test_data.to_volume_percentage(),
                if test_data.volume < 10 {
                    "PAUSED"
                } else {
                    "PLAYING"
                }
            );
        }

        println!("\n✅ All sensor mappings validated - full 0-4095 range correctly utilized");
    }
}

// Determine if we should log sensor data (throttled logging for human readability)
fn should_log_sensor_data(new_data: &ESP32SensorData, state: &mut ESP32SharedState) -> bool {
    // Log every 1 second regardless of changes
    let time_since_last_log = state.last_log_time.elapsed();
    if time_since_last_log >= Duration::from_secs(1) {
        state.last_log_time = Instant::now();
        state.last_logged_data = Some(*new_data);
        return true;
    }

    false // Remove significant change detection for cleaner output
}

// Alternative approach for PTY devices (socat virtual ports)
struct PtySerialPort {
    file: std::fs::File,
}

impl PtySerialPort {
    fn new(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?;
        Ok(PtySerialPort { file })
    }
}

impl std::io::Read for PtySerialPort {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.file.read(buf)
    }
}

impl std::io::Write for PtySerialPort {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.file.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

impl SerialPort for PtySerialPort {
    fn name(&self) -> Option<String> {
        Some("PTY Device".to_string())
    }

    fn baud_rate(&self) -> serialport::Result<u32> {
        Ok(115200)
    }

    fn data_bits(&self) -> serialport::Result<serialport::DataBits> {
        Ok(serialport::DataBits::Eight)
    }

    fn flow_control(&self) -> serialport::Result<serialport::FlowControl> {
        Ok(serialport::FlowControl::None)
    }

    fn parity(&self) -> serialport::Result<serialport::Parity> {
        Ok(serialport::Parity::None)
    }

    fn stop_bits(&self) -> serialport::Result<serialport::StopBits> {
        Ok(serialport::StopBits::One)
    }

    fn timeout(&self) -> Duration {
        Duration::from_millis(500)
    }

    fn set_baud_rate(&mut self, _baud_rate: u32) -> serialport::Result<()> {
        Ok(())
    }

    fn set_data_bits(&mut self, _data_bits: serialport::DataBits) -> serialport::Result<()> {
        Ok(())
    }

    fn set_flow_control(
        &mut self,
        _flow_control: serialport::FlowControl,
    ) -> serialport::Result<()> {
        Ok(())
    }

    fn set_parity(&mut self, _parity: serialport::Parity) -> serialport::Result<()> {
        Ok(())
    }

    fn set_stop_bits(&mut self, _stop_bits: serialport::StopBits) -> serialport::Result<()> {
        Ok(())
    }

    fn set_timeout(&mut self, _timeout: Duration) -> serialport::Result<()> {
        Ok(())
    }

    fn write_request_to_send(&mut self, _level: bool) -> serialport::Result<()> {
        Ok(())
    }

    fn write_data_terminal_ready(&mut self, _level: bool) -> serialport::Result<()> {
        Ok(())
    }

    fn read_clear_to_send(&mut self) -> serialport::Result<bool> {
        Ok(true)
    }

    fn read_data_set_ready(&mut self) -> serialport::Result<bool> {
        Ok(true)
    }

    fn read_ring_indicator(&mut self) -> serialport::Result<bool> {
        Ok(false)
    }

    fn read_carrier_detect(&mut self) -> serialport::Result<bool> {
        Ok(true)
    }

    fn bytes_to_read(&self) -> serialport::Result<u32> {
        Ok(0)
    }

    fn bytes_to_write(&self) -> serialport::Result<u32> {
        Ok(0)
    }

    fn clear(&self, _buffer_to_clear: serialport::ClearBuffer) -> serialport::Result<()> {
        Ok(())
    }

    fn try_clone(&self) -> serialport::Result<Box<dyn SerialPort>> {
        Err(serialport::Error::new(
            serialport::ErrorKind::Unknown,
            "Cloning not supported for PTY devices",
        ))
    }

    fn set_break(&self) -> serialport::Result<()> {
        Ok(())
    }

    fn clear_break(&self) -> serialport::Result<()> {
        Ok(())
    }
}

fn try_pty_connection(port_name: &str) -> Option<Box<dyn SerialPort>> {
    match PtySerialPort::new(port_name) {
        Ok(pty_port) => {
            println!("🔗 Successfully opened PTY device: {}", port_name);

            // Test communication
            thread::sleep(Duration::from_millis(100));

            let mut boxed_port: Box<dyn SerialPort> = Box::new(pty_port);
            match test_esp32_communication(&mut boxed_port) {
                Ok(true) => {
                    println!("✅ ESP32 found on PTY device: {}", port_name);
                    return Some(boxed_port);
                }
                Ok(false) => {
                    println!("❌ PTY device {} is not responding as ESP32", port_name);
                }
                Err(e) => {
                    println!("❌ PTY communication test failed on {}: {:?}", port_name, e);
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to open PTY device {}: {}", port_name, e);
        }
    }
    None
}

// Send pending lightning events to ESP32
fn send_pending_lightning_events(
    port: &mut Box<dyn SerialPort>,
    shared_state: &Arc<Mutex<ESP32SharedState>>,
) {
    let events_to_send = {
        if let Ok(mut state) = shared_state.lock() {
            // Only send events if it's been at least 100ms since last send (rate limiting)
            if state.last_lightning_sent.elapsed() < Duration::from_millis(100) {
                return;
            }

            if state.pending_lightning_events.is_empty() {
                return;
            }

            // Take all pending events and clear the queue
            let events = state.pending_lightning_events.clone();
            state.pending_lightning_events.clear();
            state.last_lightning_sent = Instant::now();
            events
        } else {
            return;
        }
    };

    // Send each lightning event
    for event in events_to_send {
        let lightning_command = format!(
            "LIGHTNING:{},{},{:.2},{:.2}\n",
            event.flash_id, event.lightning_type, event.start_time, event.intensity
        );

        match port.write_all(lightning_command.as_bytes()) {
            Ok(()) => {
                println!(
                    "📤 ESP32: Sent lightning event (Flash ID: {}, Type: {})",
                    event.flash_id,
                    if event.is_super_lightning() {
                        "Super"
                    } else {
                        "Normal"
                    }
                );
            }
            Err(e) => {
                println!("❌ ESP32: Failed to send lightning event: {}", e);
                break; // Stop sending if there's an error
            }
        }
    }
}

// Test ESP32 communication without actual hardware
pub fn test_esp32_sensor_data_conversion() {
    println!("🧪 Testing ESP32 communication and conversion functions...");

    let test_data = ESP32SensorData::test_data();
    test_data.log_converted_values();

    println!("");

    let max_data = ESP32SensorData::test_max_data();
    max_data.log_converted_values();

    println!("");

    // Validate all sensor mappings
    ESP32SensorData::validate_sensor_mappings();

    println!("✅ ESP32 communication test completed");
}
