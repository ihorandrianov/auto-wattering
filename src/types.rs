use heapless::String;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
pub struct SensorData {
    pub temperature: f32,
    pub humidity: f32,
    pub pressure: f32,
    pub soil_moisture: f32,
    pub water_level: f32,
}

#[derive(Clone)]
pub enum HttpRequest {
    PostSensorData(SensorData),
    SendAlert { message: String<64> },
    PollTasks,
}
