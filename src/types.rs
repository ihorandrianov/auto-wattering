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

#[derive(Clone, Copy)]
pub struct PumpCommand {
    pub duration_secs: u16,
}

#[derive(Clone, Copy, Default, Deserialize)]
pub struct TasksResponse {
    #[serde(default)]
    pub pump_duration: u16, // 0 = no action, >0 = run pump for N seconds
}
