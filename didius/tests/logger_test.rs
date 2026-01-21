use didius::logger::Logger;
use didius::logger::config::{LoggerConfig, LogDestinationInfo};
use didius::logger::message::Message;
use std::fs;
use std::thread;
use std::time::Duration;
use serde_json::json;

#[test]
fn test_logger_file_output() {
    let path = "logs/test_log.jsonl";
    if fs::metadata(path).is_ok() {
        fs::remove_file(path).unwrap();
    }

    let config = LoggerConfig {
        destination: LogDestinationInfo::LocalFile { path: path.to_string() },
        flush_interval_seconds: 1,
        batch_size: 1,
    };

    let mut logger = Logger::new(config);
    logger.start();

    let msg = Message::new("TEST_EVENT".to_string(), json!({"data": 123}));
    logger.log(msg);

    // Wait for flush
    thread::sleep(Duration::from_millis(1500));
    
    logger.stop();

    let content = fs::read_to_string(path).unwrap();
    assert!(content.contains("TEST_EVENT"));
    assert!(content.contains("123"));
    
    fs::remove_file(path).unwrap();
}

#[test]
fn test_logger_s3_init() {
    // This test primarily checks that we can initialize the logger with S3 config 
    // and it runs without panicking, even if upload fails due to missing creds.
    // To properly test S3, we would need to mock S3Client or have real credentials.
    
    // Create dummy auth/aws.yaml if not exists, just to pass the file read check?
    // The current impl reads from "auth/aws.yaml" hardcoded in mod.rs (which is a limitation I should fix later, but for now...)
    // Wait, I hardcoded "auth/aws.yaml" in mod.rs. 
    // I should probably ensure the directory exists at least.
    
    let auth_dir = "auth";
    if fs::metadata(auth_dir).is_err() {
        fs::create_dir(auth_dir).ok();
    }
    let aws_path = "auth/aws.yaml";
    let created_dummy = if fs::metadata(aws_path).is_err() {
        fs::write(aws_path, "region: ap-northeast-2\naccess_key_id: test\nsecret_access_key: test").unwrap();
        true
    } else {
        false
    };

    let config = LoggerConfig {
        destination: LogDestinationInfo::AmazonS3 { 
            bucket: "didius".to_string(), 
            key_prefix: "logs".to_string(), 
            region: "ap-northeast-2".to_string() 
        },
        flush_interval_seconds: 1,
        batch_size: 1,
    };

    let mut logger = Logger::new(config);
    logger.start();

    let msg = Message::new("S3_TEST_EVENT".to_string(), json!({"data": "s3"}));
    logger.log(msg);

    // Wait for flush attempt
    thread::sleep(Duration::from_millis(1500));
    
    logger.stop();

    // Clean up
    if created_dummy {
        fs::remove_file(aws_path).unwrap();
    }
}
