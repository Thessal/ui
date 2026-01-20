use didius_oms::logger::config::{LoggerConfig, LogDestinationInfo};
use didius_oms::logger::message::Message;
use didius_oms::logger::Logger;
use serde_json::json;
use std::fs;
use std::thread;
use std::time::Duration;

#[test]
fn test_logger_file_flush() {
    let path = "tests/test_logs.log";
    // Clean up previous
    let _ = fs::remove_file(path);
    
    let config = LoggerConfig {
        destination: LogDestinationInfo::LocalFile { path: path.to_string() },
        flush_interval_seconds: 1,
        batch_size: 2,
    };
    
    let mut logger = Logger::new(config);
    logger.start();
    
    // Log messages
    logger.log(Message::new("INFO".to_string(), json!({"msg": "Hello"})));
    logger.log(Message::new("INFO".to_string(), json!({"msg": "World"})));
    
    // Config flush interval is 1s. Wait 1.5s.
    thread::sleep(Duration::from_millis(1500));
    
    // Check file
    let content = fs::read_to_string(path).expect("Failed to read log file");
    assert!(content.contains("Hello"));
    assert!(content.contains("World"));
    
    logger.stop();
    let _ = fs::remove_file(path);
}

#[test]
fn test_logger_manual_stop_flush() {
    // Verify that stop() flushes remaining buffer
    let path = "tests/test_stop_flush.log";
    let _ = fs::remove_file(path);

    let config = LoggerConfig {
        destination: LogDestinationInfo::LocalFile { path: path.to_string() },
        flush_interval_seconds: 10, // Long interval
        batch_size: 100,
    };
    
    let mut logger = Logger::new(config);
    logger.start();
    
    logger.log(Message::new("INFO".to_string(), json!({"msg": "StopMe"})));
    
    // Stop should trigger flush
    logger.stop();
    
    let content = fs::read_to_string(path).expect("Failed to read log file");
    assert!(content.contains("StopMe"));
    
    let _ = fs::remove_file(path);
}
