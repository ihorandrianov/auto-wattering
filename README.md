# Watering Embassy

An automated plant watering system built with Rust and Embassy async runtime for the Raspberry Pi Pico 2 W.

## Features

- **Environmental Monitoring**: BME280 sensor for temperature, humidity, and pressure readings
- **Soil Moisture Sensing**: Capacitive soil moisture sensor via ADC
- **Water Level Detection**: Ultrasonic sonar sensor (HC-SR04) for tank water level monitoring
- **OLED Display**: SSD1306 128x64 display for real-time sensor readings
- **WiFi Connectivity**: CYW43 wireless chip for network communication
- **HTTP Reporting**: Sends sensor data to a remote server

## Hardware

- Raspberry Pi Pico 2 W
- BME280 temperature/humidity/pressure sensor (I2C)
- SSD1306 OLED display (I2C)
- Capacitive soil moisture sensor (ADC on GPIO28)
- HC-SR04 ultrasonic sensor (GPIO16/17)

### Pin Configuration

| Component | Pin |
|-----------|-----|
| I2C SDA | GPIO26 |
| I2C SCL | GPIO27 |
| Soil Sensor | GPIO28 (ADC) |
| Sonar Trigger | GPIO16 |
| Sonar Echo | GPIO17 |

## Building

```bash
cargo build --release
```

## Flashing

Copy the generated `.uf2` file to the Pico in bootloader mode, or use a debug probe.

## Configuration

Create `src/config.rs` with your WiFi credentials:

```rust
pub const WIFI_NETWORK: &str = "your-ssid";
pub const WIFI_PASSWORD: &str = "your-password";
pub const SENSOR_INTERVAL_MS: u64 = 5000;
```

## Dependencies

- Embassy async runtime
- CYW43 WiFi driver
- BME280 sensor driver
- SSD1306 display driver
- embedded-graphics for display rendering
