use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LogDestinationInfo {
    LocalFile { path: String },
    AmazonS3 { bucket: String, key_prefix: String, region: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggerConfig {
    pub destination: LogDestinationInfo,
    pub flush_interval_seconds: u64,
    pub batch_size: usize,
}

impl Default for LoggerConfig {
    fn default() -> Self {
        LoggerConfig {
            destination: LogDestinationInfo::LocalFile { path: "logs/trade.log".to_string() },
            flush_interval_seconds: 60,
            batch_size: 100,
        }
    }
}
