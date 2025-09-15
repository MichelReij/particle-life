/*
  ESP32 Volume Controller
  Enhanced sensor controller with potentiometer volume control for particle-life simulation
  
  Pinout:
  - Volume potentiometer: A0 (GPIO 36) - analog input
  - Temperature sensor: A1 (GPIO 39) - analog input  
  - Pressure sensor: A2 (GPIO 34) - analog input
  - UV sensor: A3 (GPIO 35) - analog input
  - Electrical sensor: A4 (GPIO 32) - analog input
  
  Protocol: 19-byte UART packet
  Header(0xAA) + Zoom(2) + Pan_X(2) + Pan_Y(2) + Temp(2) + Pressure(2) + UV(2) + Electrical(2) + Volume(2) + Sleep(1) + Footer(0x55)
  
  Volume Control:
  - Values 0-99: Audio paused
  - Values 100-4096: Audio playing (1-100% volume)
*/

// Pin definitions
const int VOLUME_PIN = 36;      // A0 - Volume potentiometer
const int TEMP_PIN = 39;        // A1 - Temperature sensor
const int PRESSURE_PIN = 34;    // A2 - Pressure sensor  
const int UV_PIN = 35;          // A3 - UV sensor
const int ELECTRICAL_PIN = 32;  // A4 - Electrical sensor

// Packet structure
struct SensorPacket {
  uint8_t header;        // 0xAA
  uint16_t zoom;         // Zoom level (0-4096)
  uint16_t pan_x;        // Pan X position (0-4096)
  uint16_t pan_y;        // Pan Y position (0-4096)
  uint16_t temperature;  // Temperature reading (0-4096)
  uint16_t pressure;     // Pressure reading (0-4096)
  uint16_t uv;           // UV reading (0-4096)
  uint16_t electrical;   // Electrical reading (0-4096)
  uint16_t volume;       // Volume control (0-4096)
  uint8_t sleep;         // Sleep flag (0/1)
  uint8_t footer;        // 0x55
};

// Variables for smooth value interpolation
float smoothed_zoom = 2048.0;
float smoothed_pan_x = 2048.0;
float smoothed_pan_y = 2048.0;
float smoothed_volume = 2048.0;

// Smoothing factor (0.1 = slow, 0.9 = fast)
const float SMOOTHING = 0.15;

void setup() {
  Serial.begin(115200);
  
  // Configure analog resolution
  analogReadResolution(12); // 12-bit ADC (0-4095)
  
  // Set analog attenuation for better range
  analogSetAttenuation(ADC_11db); // 0-3.3V range
  
  Serial.println("ESP32 Volume Controller v2.0");
  Serial.println("Enhanced particle-life sensor system with volume control");
  delay(1000);
}

void loop() {
  SensorPacket packet;
  
  // Read analog sensors
  uint16_t temp_raw = analogRead(TEMP_PIN);
  uint16_t pressure_raw = analogRead(PRESSURE_PIN);
  uint16_t uv_raw = analogRead(UV_PIN);
  uint16_t electrical_raw = analogRead(ELECTRICAL_PIN);
  uint16_t volume_raw = analogRead(VOLUME_PIN);
  
  // Simulate interactive controls (these would be joysticks/encoders in real setup)
  float time_factor = millis() / 10000.0;
  uint16_t zoom_target = 2048 + sin(time_factor * 0.5) * 1000;
  uint16_t pan_x_target = 2048 + cos(time_factor * 0.3) * 800;
  uint16_t pan_y_target = 2048 + sin(time_factor * 0.4) * 600;
  
  // Apply smoothing to control values
  smoothed_zoom = smoothed_zoom * (1.0 - SMOOTHING) + zoom_target * SMOOTHING;
  smoothed_pan_x = smoothed_pan_x * (1.0 - SMOOTHING) + pan_x_target * SMOOTHING;
  smoothed_pan_y = smoothed_pan_y * (1.0 - SMOOTHING) + pan_y_target * SMOOTHING;
  smoothed_volume = smoothed_volume * (1.0 - SMOOTHING) + volume_raw * SMOOTHING;
  
  // Build packet
  packet.header = 0xAA;
  packet.zoom = constrain((uint16_t)smoothed_zoom, 0, 4096);
  packet.pan_x = constrain((uint16_t)smoothed_pan_x, 0, 4096);
  packet.pan_y = constrain((uint16_t)smoothed_pan_y, 0, 4096);
  packet.temperature = constrain(temp_raw, 0, 4096);
  packet.pressure = constrain(pressure_raw, 0, 4096);
  packet.uv = constrain(uv_raw, 0, 4096);
  packet.electrical = constrain(electrical_raw, 0, 4096);
  packet.volume = constrain((uint16_t)smoothed_volume, 0, 4096);
  packet.sleep = 0; // Active mode
  packet.footer = 0x55;
  
  // Send packet as big-endian bytes
  Serial.write(packet.header);
  Serial.write((packet.zoom >> 8) & 0xFF);
  Serial.write(packet.zoom & 0xFF);
  Serial.write((packet.pan_x >> 8) & 0xFF);
  Serial.write(packet.pan_x & 0xFF);
  Serial.write((packet.pan_y >> 8) & 0xFF);
  Serial.write(packet.pan_y & 0xFF);
  Serial.write((packet.temperature >> 8) & 0xFF);
  Serial.write(packet.temperature & 0xFF);
  Serial.write((packet.pressure >> 8) & 0xFF);
  Serial.write(packet.pressure & 0xFF);
  Serial.write((packet.uv >> 8) & 0xFF);
  Serial.write(packet.uv & 0xFF);
  Serial.write((packet.electrical >> 8) & 0xFF);
  Serial.write(packet.electrical & 0xFF);
  Serial.write((packet.volume >> 8) & 0xFF);
  Serial.write(packet.volume & 0xFF);
  Serial.write(packet.sleep);
  Serial.write(packet.footer);
  
  // Debug output every 2 seconds
  static unsigned long last_debug = 0;
  if (millis() - last_debug > 2000) {
    last_debug = millis();
    
    Serial.print("Volume: ");
    Serial.print(packet.volume);
    Serial.print(" (");
    if (packet.volume < 100) {
      Serial.print("PAUSED");
    } else {
      float volume_percent = ((packet.volume - 100) * 99.0 / (4096 - 100)) + 1.0;
      Serial.print(volume_percent, 1);
      Serial.print("%");
    }
    Serial.print("), Temp: ");
    Serial.print(packet.temperature);
    Serial.print(", Pressure: ");
    Serial.print(packet.pressure);
    Serial.print(", UV: ");
    Serial.print(packet.uv);
    Serial.print(", Electrical: ");
    Serial.println(packet.electrical);
  }
  
  // Send at ~30Hz
  delay(33);
}