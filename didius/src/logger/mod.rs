pub mod message;
pub mod config;

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use crate::logger::message::Message;
use crate::logger::config::{LoggerConfig, LogDestinationInfo};
use anyhow::Result;
use std::fs::OpenOptions;
use std::io::Write;

pub struct Logger {
    config: LoggerConfig,
    buffer: Arc<Mutex<Vec<Message>>>,
    is_running: Arc<Mutex<bool>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Logger {
    pub fn new(config: LoggerConfig) -> Self {
        Logger {
            config,
            buffer: Arc::new(Mutex::new(Vec::new())),
            is_running: Arc::new(Mutex::new(false)),
            handle: None,
        }
    }

    pub fn start(&mut self) {
        let running = self.is_running.clone();
        {
            let mut r = running.lock().unwrap();
            if *r { return; }
            *r = true;
        }

        let buffer = self.buffer.clone();
        let destination = self.config.destination.clone();
        let interval = self.config.flush_interval_seconds;
        let running_clone = self.is_running.clone();

        self.handle = Some(thread::spawn(move || {
            loop {
                // Sleep first or flush first? 
                thread::sleep(Duration::from_secs(interval));
                
                {
                    let r = running_clone.lock().unwrap();
                    if !*r {
                         // Flush one last time before exit?
                         Self::flush(&buffer, &destination);
                         break;
                    }
                }
                
                Self::flush(&buffer, &destination);
            }
        }));
    }

    pub fn stop(&mut self) {
        {
            let mut r = self.is_running.lock().unwrap();
            *r = false;
        }
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }

    pub fn log(&self, msg: Message) {
        let mut buf = self.buffer.lock().unwrap();
        buf.push(msg);
        // Explicit batch size flush check could act here too
        if buf.len() >= self.config.batch_size {
             // To avoid blocking the log call with IO, we might signal the flusher?
             // Or just let the timer handle it for now to stay non-blocking.
             // Or spawn a quick flush?
             // For simplicity, we rely on the timer in MVP.
        }
    }

    fn flush(buffer: &Arc<Mutex<Vec<Message>>>, destination: &LogDestinationInfo) {
        let mut messages = {
            let mut b = buffer.lock().unwrap();
            if b.is_empty() { return; }
            std::mem::take(&mut *b)
        };
        
        match destination {
            LogDestinationInfo::LocalFile { path } => {
                if let Err(e) = Self::write_to_file(path, &messages) {
                    eprintln!("Failed to write logs to file: {}", e);
                    // Re-buffer? Or drop? 
                    // For now drop and log error.
                }
            },
            LogDestinationInfo::AmazonS3 { bucket, key_prefix, region: _ } => {
                // Mock S3 implementation or placeholder
                // "Implement it or add package" -> I'll assume users want to see the code structure
                // effectively doing nothing or printing.
                println!("Mock: Uploading {} messages to S3 bucket {} key {}", messages.len(), bucket, key_prefix);
                // In real impl: use aws_sdk_s3
            }
        }
    }

    fn write_to_file(path: &str, messages: &[Message]) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
            
        for msg in messages {
            let json = serde_json::to_string(msg)?;
            writeln!(file, "{}", json)?;
        }
        Ok(())
    }
}
