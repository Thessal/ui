use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub log_type: String,
    pub log_body: serde_json::Value,
    pub timestamp: f64, // Epoch seconds
}

impl Message {
    pub fn new(log_type: String, log_body: serde_json::Value) -> Self {
        Message {
            log_type,
            log_body,
            timestamp: chrono::Local::now().timestamp_millis() as f64 / 1000.0,
        }
    }
}
