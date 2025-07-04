/*
 * ESP32 Particle Life Controller
 *
 * This Arduino sketch sends sensor data to control a particle life simulation.
 * Sends 17-byte packets at ~60 FPS over Serial at 115200 baud.
 *
 * Hardware requirements:
 * - ESP32 development board
 * - (Optional) Sensors for real parameter control:
 *   - Temperature sensor (DS18B20, DHT22, etc.)
 *   - Pressure sensor (BMP280, etc.)
 *   - Light sensor (photoresistor, TSL2561, etc.)
 *   - Potentiometers for manual control
 *
 * Protocol: [0xAA] + 7×u16 (14 bytes) + 1×bool + [0x55] = 17 bytes total
 */

// Pin definitions (adjust based on your hardware)
#define TEMP_SENSOR_PIN A0       // Analog pin for temperature sensor
#define PRESSURE_SENSOR_PIN A1   // Analog pin for pressure sensor
#define UV_SENSOR_PIN A2         // Analog pin for UV/light sensor
#define ELECTRICAL_SENSOR_PIN A3 // Analog pin for electrical activity
#define ZOOM_POT_PIN A4          // Potentiometer for zoom control
#define PAN_X_POT_PIN A5         // Potentiometer for pan X
#define PAN_Y_POT_PIN A6         // Potentiometer for pan Y
#define SLEEP_BUTTON_PIN 2       // Digital pin for sleep button

// Packet structure
struct SensorPacket
{
    uint8_t start_marker; // 0xAA
    uint16_t zoom;        // 0-4096: zoom level
    uint16_t pan_x;       // 0-4096: pan X position
    uint16_t pan_y;       // 0-4096: pan Y position
    uint16_t temperature; // 0-4096: temperature (3°C - 130°C)
    uint16_t pressure;    // 0-4096: pressure (0 - 350 units)
    uint16_t uv;          // 0-4096: UV light (0 - 50 units)
    uint16_t electrical;  // 0-4096: electrical activity (0 - 3.0)
    uint8_t sleep;        // 0/1: sleep mode
    uint8_t end_marker;   // 0x55
} __attribute__((packed));

// Timing
unsigned long lastSendTime = 0;
const unsigned long sendInterval = 16; // ~60 FPS (16ms between packets)

// Demo mode variables (if no real sensors)
unsigned long demoTime = 0;
bool demoMode = true; // Set to false if you have real sensors

void setup()
{
    Serial.begin(115200);

    // Initialize pins
    pinMode(SLEEP_BUTTON_PIN, INPUT_PULLUP);

    // Initialize analog pins (if using external sensors)
    // pinMode(TEMP_SENSOR_PIN, INPUT);
    // pinMode(PRESSURE_SENSOR_PIN, INPUT);
    // pinMode(UV_SENSOR_PIN, INPUT);
    // pinMode(ELECTRICAL_SENSOR_PIN, INPUT);
    // pinMode(ZOOM_POT_PIN, INPUT);
    // pinMode(PAN_X_POT_PIN, INPUT);
    // pinMode(PAN_Y_POT_PIN, INPUT);

    Serial.println("ESP32 Particle Life Controller Started");
    Serial.println("Sending sensor data at 60 FPS...");

    // Brief delay to allow serial to stabilize
    delay(100);
}

void loop()
{
    unsigned long currentTime = millis();

    if (currentTime - lastSendTime >= sendInterval)
    {
        sendSensorData();
        lastSendTime = currentTime;
    }

    // Small delay to prevent overwhelming the CPU
    delay(1);
}

void sendSensorData()
{
    SensorPacket packet;

    // Set packet markers
    packet.start_marker = 0xAA;
    packet.end_marker = 0x55;

    if (demoMode)
    {
        // Demo mode: generate smooth, interesting parameter changes
        generateDemoData(&packet);
    }
    else
    {
        // Real sensor mode: read actual sensor values
        readRealSensors(&packet);
    }

    // Send the packet as binary data
    Serial.write((uint8_t *)&packet, sizeof(packet));
    Serial.flush(); // Ensure data is sent immediately
}

void generateDemoData(SensorPacket *packet)
{
    demoTime = millis();
    float t = demoTime / 1000.0; // Time in seconds

    // Generate smooth, oscillating values that create interesting effects

    // Zoom: Slow zoom in/out cycle (10-second period)
    float zoom_factor = 0.5 + 0.4 * sin(t * 0.628); // 0.1 to 0.9
    packet->zoom = (uint16_t)(zoom_factor * 4096);

    // Pan: Slow circular motion (20-second period)
    float pan_x_factor = 0.5 + 0.3 * cos(t * 0.314);
    float pan_y_factor = 0.5 + 0.3 * sin(t * 0.314);
    packet->pan_x = (uint16_t)(pan_x_factor * 4096);
    packet->pan_y = (uint16_t)(pan_y_factor * 4096);

    // Temperature: Gradual heating/cooling (30-second period)
    float temp_factor = 0.3 + 0.6 * sin(t * 0.209); // 0.3 to 0.9
    packet->temperature = (uint16_t)(temp_factor * 4096);

    // Pressure: Medium frequency oscillation (8-second period)
    float pressure_factor = 0.4 + 0.5 * sin(t * 0.785);
    packet->pressure = (uint16_t)(pressure_factor * 4096);

    // UV: Fast oscillation (5-second period)
    float uv_factor = 0.2 + 0.6 * sin(t * 1.257);
    packet->uv = (uint16_t)(uv_factor * 4096);

    // Electrical: Irregular pulses
    float electrical_factor = 0.1 + 0.8 * abs(sin(t * 2.0) * cos(t * 0.7));
    packet->electrical = (uint16_t)(electrical_factor * 4096);

    // Sleep: Toggle every 60 seconds
    packet->sleep = ((int)(t / 60) % 2) == 1 ? 1 : 0;
}

void readRealSensors(SensorPacket *packet)
{
    // Read analog sensors (0-1023 from analogRead, map to 0-4096)

    // Zoom control (potentiometer)
    int zoom_raw = analogRead(ZOOM_POT_PIN);
    packet->zoom = map(zoom_raw, 0, 1023, 0, 4096);

    // Pan controls (potentiometers)
    int pan_x_raw = analogRead(PAN_X_POT_PIN);
    int pan_y_raw = analogRead(PAN_Y_POT_PIN);
    packet->pan_x = map(pan_x_raw, 0, 1023, 0, 4096);
    packet->pan_y = map(pan_y_raw, 0, 1023, 0, 4096);

    // Temperature sensor (adjust based on your sensor)
    int temp_raw = analogRead(TEMP_SENSOR_PIN);
    // Example for simple temperature sensor with voltage divider:
    // float voltage = temp_raw * (3.3 / 1023.0);
    // float temp_celsius = (voltage - 0.5) * 100; // LM35 formula
    // packet->temperature = map(constrain(temp_celsius, 3, 130), 3, 130, 0, 4096);
    packet->temperature = map(temp_raw, 0, 1023, 0, 4096); // Simple mapping for now

    // Pressure sensor
    int pressure_raw = analogRead(PRESSURE_SENSOR_PIN);
    packet->pressure = map(pressure_raw, 0, 1023, 0, 4096);

    // UV/Light sensor
    int uv_raw = analogRead(UV_SENSOR_PIN);
    packet->uv = map(uv_raw, 0, 1023, 0, 4096);

    // Electrical activity sensor
    int electrical_raw = analogRead(ELECTRICAL_SENSOR_PIN);
    packet->electrical = map(electrical_raw, 0, 1023, 0, 4096);

    // Sleep button (active low with pullup)
    packet->sleep = !digitalRead(SLEEP_BUTTON_PIN);
}

// Optional: Function to print packet data for debugging
void debugPrintPacket(SensorPacket *packet)
{
    Serial.print("DEBUG: ");
    Serial.print("zoom=");
    Serial.print(packet->zoom);
    Serial.print(" pan_x=");
    Serial.print(packet->pan_x);
    Serial.print(" pan_y=");
    Serial.print(packet->pan_y);
    Serial.print(" temp=");
    Serial.print(packet->temperature);
    Serial.print(" pressure=");
    Serial.print(packet->pressure);
    Serial.print(" uv=");
    Serial.print(packet->uv);
    Serial.print(" electrical=");
    Serial.print(packet->electrical);
    Serial.print(" sleep=");
    Serial.println(packet->sleep);
}
