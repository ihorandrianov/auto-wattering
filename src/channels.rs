use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

use crate::types::{HttpRequest, SensorData};

// Sensor data to display (capacity 1 - only latest reading matters)
pub static SENSOR_CHANNEL: Channel<CriticalSectionRawMutex, SensorData, 1> = Channel::new();

// HTTP request queue (capacity 4 - buffer a few requests)
pub static HTTP_CHANNEL: Channel<CriticalSectionRawMutex, HttpRequest, 4> = Channel::new();
