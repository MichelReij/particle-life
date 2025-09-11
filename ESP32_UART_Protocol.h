/*
 * ESP32 UART Protocol Definition for Particle Life Simulation
 * 
 * This header defines the complete UART communication protocol between
 * the ESP32 and the Rust particle life simulation.
 * 
 * PROTOCOL OVERVIEW:
 * - Bidirectional communication over UART at 115200 baud
 * - ESP32 sends sensor data at ~60 FPS (16ms intervals)
 * - Rust app sends lightning events when they occur
 * - All multi-byte values use big-endian format
 * 
 * Include this file in your ESP32 Arduino project:
 * #include "ESP32_UART_Protocol.h"
 */

#ifndef ESP32_UART_PROTOCOL_H
#define ESP32_UART_PROTOCOL_H

#include <Arduino.h>

// =============================================================================
// SENSOR DATA TRANSMISSION (ESP32 → Rust App)
// =============================================================================

#define UART_BAUD_RATE 115200
#define SENSOR_PACKET_SIZE 17
#define SENSOR_FPS 60
#define SENSOR_INTERVAL_MS (1000 / SENSOR_FPS)  // 16ms

// Packet markers
#define PACKET_START_MARKER 0xAA
#define PACKET_END_MARKER   0x55

// Sensor value ranges (all sensors use 0-4096 range for 12-bit precision)
#define SENSOR_MIN_VALUE 0
#define SENSOR_MAX_VALUE 4096

/**
 * Sensor Data Packet Structure (17 bytes total)
 * 
 * This packet is sent from ESP32 to Rust app every 16ms (~60 FPS)
 * All u16 values are transmitted in big-endian format
 */
struct SensorPacket {
    uint8_t  start_marker;   // 0xAA - packet start
    uint16_t zoom;           // 0-4096: zoom level (0=min zoom, 4096=max zoom)
    uint16_t pan_x;          // 0-4096: pan X position (0=left, 2048=center, 4096=right)
    uint16_t pan_y;          // 0-4096: pan Y position (0=top, 2048=center, 4096=bottom)
    uint16_t temperature;    // 0-4096: temperature (maps to 3°C - 130°C)
    uint16_t pressure;       // 0-4096: pressure (maps to 0 - 350 units)
    uint16_t uv;             // 0-4096: UV light intensity (maps to 0 - 50 units)
    uint16_t electrical;     // 0-4096: electrical activity (maps to 0.0 - 3.0)
    uint8_t  sleep;          // 0/1: sleep mode (0=awake, 1=sleep)
    uint8_t  end_marker;     // 0x55 - packet end
} __attribute__((packed));

// Parameter mapping constants for real-world sensors
#define TEMP_MIN_CELSIUS    3.0f     // Minimum temperature in Celsius
#define TEMP_MAX_CELSIUS    130.0f   // Maximum temperature in Celsius
#define PRESSURE_MIN        0.0f     // Minimum pressure units
#define PRESSURE_MAX        350.0f   // Maximum pressure units
#define UV_MIN              0.0f     // Minimum UV intensity
#define UV_MAX              50.0f    // Maximum UV intensity
#define ELECTRICAL_MIN      0.0f     // Minimum electrical activity
#define ELECTRICAL_MAX      3.0f     // Maximum electrical activity

// Helper macros for sensor value conversion
#define MAP_TO_SENSOR_RANGE(value, min_val, max_val) \
    (uint16_t)(((value - min_val) / (max_val - min_val)) * SENSOR_MAX_VALUE)

#define MAP_FROM_SENSOR_RANGE(sensor_val, min_val, max_val) \
    (min_val + ((float)sensor_val / SENSOR_MAX_VALUE) * (max_val - min_val))

// =============================================================================
// LIGHTNING EVENT RECEPTION (Rust App → ESP32)
// =============================================================================

#define LIGHTNING_PACKET_SIZE 9
#define LIGHTNING_START_MARKER 0xBB
#define LIGHTNING_END_MARKER   0xCC

// Lightning types
#define LIGHTNING_TYPE_NORMAL 0
#define LIGHTNING_TYPE_SUPER  1

/**
 * Lightning Event Packet Structure (9 bytes total)
 * 
 * This packet is sent from Rust app to ESP32 when lightning occurs
 * All multi-byte values are transmitted in big-endian format
 */
struct LightningPacket {
    uint8_t  start_marker;    // 0xBB - lightning packet start
    uint32_t flash_id;        // Unique flash identifier (big-endian)
    uint8_t  lightning_type;  // 0=normal lightning, 1=super lightning
    uint16_t intensity;       // 0-4096: lightning intensity (maps to 0.0-1.0)
    uint8_t  end_marker;      // 0xCC - lightning packet end
} __attribute__((packed));

// =============================================================================
// ARDUINO HELPER FUNCTIONS
// =============================================================================

/**
 * Send sensor data packet to Rust app
 * Call this function every 16ms from your main loop
 */
inline void sendSensorPacket(const SensorPacket& packet) {
    // Convert all u16 values to big-endian and send
    Serial.write(packet.start_marker);
    
    // Send u16 values in big-endian format
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
    
    Serial.write(packet.sleep);
    Serial.write(packet.end_marker);
    Serial.flush(); // Ensure immediate transmission
}

/**
 * Read lightning event from Rust app (non-blocking)
 * Returns true if a lightning packet was received
 */
inline bool readLightningPacket(LightningPacket& packet) {
    if (Serial.available() >= LIGHTNING_PACKET_SIZE) {
        uint8_t buffer[LIGHTNING_PACKET_SIZE];
        Serial.readBytes(buffer, LIGHTNING_PACKET_SIZE);
        
        // Validate packet format
        if (buffer[0] != LIGHTNING_START_MARKER || 
            buffer[LIGHTNING_PACKET_SIZE-1] != LIGHTNING_END_MARKER) {
            return false;
        }
        
        // Parse packet (convert from big-endian)
        packet.start_marker = buffer[0];
        packet.flash_id = ((uint32_t)buffer[1] << 24) | 
                         ((uint32_t)buffer[2] << 16) | 
                         ((uint32_t)buffer[3] << 8) | 
                         (uint32_t)buffer[4];
        packet.lightning_type = buffer[5];
        packet.intensity = ((uint16_t)buffer[6] << 8) | (uint16_t)buffer[7];
        packet.end_marker = buffer[8];
        
        return true;
    }
    return false;
}

/**
 * Convert real-world temperature to sensor value
 */
inline uint16_t temperatureToSensorValue(float celsius) {
    celsius = constrain(celsius, TEMP_MIN_CELSIUS, TEMP_MAX_CELSIUS);
    return MAP_TO_SENSOR_RANGE(celsius, TEMP_MIN_CELSIUS, TEMP_MAX_CELSIUS);
}

/**
 * Convert real-world pressure to sensor value
 */
inline uint16_t pressureToSensorValue(float pressure) {
    pressure = constrain(pressure, PRESSURE_MIN, PRESSURE_MAX);
    return MAP_TO_SENSOR_RANGE(pressure, PRESSURE_MIN, PRESSURE_MAX);
}

/**
 * Convert real-world UV intensity to sensor value
 */
inline uint16_t uvToSensorValue(float uv_intensity) {
    uv_intensity = constrain(uv_intensity, UV_MIN, UV_MAX);
    return MAP_TO_SENSOR_RANGE(uv_intensity, UV_MIN, UV_MAX);
}

/**
 * Convert real-world electrical activity to sensor value
 */
inline uint16_t electricalToSensorValue(float electrical) {
    electrical = constrain(electrical, ELECTRICAL_MIN, ELECTRICAL_MAX);
    return MAP_TO_SENSOR_RANGE(electrical, ELECTRICAL_MIN, ELECTRICAL_MAX);
}

/**
 * Convert analog reading (0-1023) to sensor value (0-4096)
 */
inline uint16_t analogToSensorValue(int analog_reading) {
    return map(constrain(analog_reading, 0, 1023), 0, 1023, 0, SENSOR_MAX_VALUE);
}

// =============================================================================
// EXAMPLE USAGE
// =============================================================================

/*
// Basic Arduino setup
void setup() {
    Serial.begin(UART_BAUD_RATE);
    pinMode(LED_BUILTIN, OUTPUT);
}

void loop() {
    static unsigned long lastSensorSend = 0;
    static SensorPacket sensorPacket;
    static LightningPacket lightningPacket;
    
    // Send sensor data at 60 FPS
    if (millis() - lastSensorSend >= SENSOR_INTERVAL_MS) {
        // Read your sensors here
        sensorPacket.start_marker = PACKET_START_MARKER;
        sensorPacket.zoom = analogToSensorValue(analogRead(A0));
        sensorPacket.pan_x = analogToSensorValue(analogRead(A1));
        sensorPacket.pan_y = analogToSensorValue(analogRead(A2));
        sensorPacket.temperature = temperatureToSensorValue(readTemperatureSensor());
        sensorPacket.pressure = pressureToSensorValue(readPressureSensor());
        sensorPacket.uv = uvToSensorValue(readUVSensor());
        sensorPacket.electrical = electricalToSensorValue(readElectricalSensor());
        sensorPacket.sleep = digitalRead(SLEEP_BUTTON_PIN) == LOW;
        sensorPacket.end_marker = PACKET_END_MARKER;
        
        sendSensorPacket(sensorPacket);
        lastSensorSend = millis();
    }
    
    // Check for lightning events
    if (readLightningPacket(lightningPacket)) {
        // Handle lightning event
        if (lightningPacket.lightning_type == LIGHTNING_TYPE_SUPER) {
            // Super lightning - bright flash
            digitalWrite(LED_BUILTIN, HIGH);
            delay(200);
            digitalWrite(LED_BUILTIN, LOW);
        } else {
            // Normal lightning - quick flash
            digitalWrite(LED_BUILTIN, HIGH);
            delay(50);
            digitalWrite(LED_BUILTIN, LOW);
        }
    }
}
*/

#endif // ESP32_UART_PROTOCOL_H