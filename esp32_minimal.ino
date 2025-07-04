/*
 * Minimal ESP32 code for Particle Life Simulation
 * Sends 17-byte packets at 115200 baud
 *
 * Packet format:
 * [0xAA] [zoom_high] [zoom_low] [pan_x_high] [pan_x_low] [pan_y_high] [pan_y_low]
 * [temp_high] [temp_low] [pressure_high] [pressure_low] [uv_high] [uv_low]
 * [electrical_high] [electrical_low] [sleep] [0x55]
 */

void setup()
{
    Serial.begin(115200);
    delay(1000); // Give serial time to initialize
}

void loop()
{
    // Create packet buffer (17 bytes)
    uint8_t packet[17];

    // Start marker
    packet[0] = 0xAA;

    // Sensor values (0-4096 range)
    uint16_t zoom = 2048;        // Middle zoom
    uint16_t pan_x = 2048;       // Center X
    uint16_t pan_y = 2048;       // Center Y
    uint16_t temperature = 1640; // ~50°C
    uint16_t pressure = 1024;    // Some pressure
    uint16_t uv = 512;           // Low UV
    uint16_t electrical = 0;     // No electrical activity
    bool sleep_mode = false;     // Awake

    // Convert to big-endian bytes
    packet[1] = (zoom >> 8) & 0xFF;        // zoom high byte
    packet[2] = zoom & 0xFF;               // zoom low byte
    packet[3] = (pan_x >> 8) & 0xFF;       // pan_x high byte
    packet[4] = pan_x & 0xFF;              // pan_x low byte
    packet[5] = (pan_y >> 8) & 0xFF;       // pan_y high byte
    packet[6] = pan_y & 0xFF;              // pan_y low byte
    packet[7] = (temperature >> 8) & 0xFF; // temperature high byte
    packet[8] = temperature & 0xFF;        // temperature low byte
    packet[9] = (pressure >> 8) & 0xFF;    // pressure high byte
    packet[10] = pressure & 0xFF;          // pressure low byte
    packet[11] = (uv >> 8) & 0xFF;         // uv high byte
    packet[12] = uv & 0xFF;                // uv low byte
    packet[13] = (electrical >> 8) & 0xFF; // electrical high byte
    packet[14] = electrical & 0xFF;        // electrical low byte
    packet[15] = sleep_mode ? 1 : 0;       // sleep boolean

    // End marker
    packet[16] = 0x55;

    // Send packet
    Serial.write(packet, 17);

    // Wait 16ms for ~60 FPS
    delay(16);
}
