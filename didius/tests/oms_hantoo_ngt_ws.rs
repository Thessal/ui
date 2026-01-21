#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;
    use didius::adapter::hantoo_ngt_futopt::HantooNightAdapter;
    use didius::oms::engine::OMSEngine;
    use didius::logger::Logger;
    use didius::logger::config::LoggerConfig;
    use std::sync::mpsc;
    use rust_decimal::prelude::ToPrimitive;
    use didius::logger::message::Message;
    use serde_json::json;

    #[test]
    fn test_oms_hantoo_ngt_monitor_live() -> Result<()> {
        // 1. Initialize Adapter
        println!("Initializing HantooNightAdapter...");
        let adapter = Arc::new(HantooNightAdapter::new("auth/hantoo.yaml")?);
        adapter.set_debug_mode(true);
        
        // 2. Initialize Engine
        use didius::logger::config::LogDestinationInfo;
        let file_destination = LogDestinationInfo::LocalFile { path: "logs/monitor.log".to_string() };
        let s3_destination = LogDestinationInfo::AmazonS3 { 
            bucket: "didius".to_string(),
            key_prefix: "logs".to_string(),
            region: "ap-northeast-2".to_string(),
        };
        let logger_config = LoggerConfig {
            destination: s3_destination,
            flush_interval_seconds: 1,
            batch_size: 1000,
        };
        let logger = Arc::new(Mutex::new(Logger::new(logger_config)));
        logger.lock().unwrap().start();
        let engine = OMSEngine::new(adapter.clone(), 0.15, logger.clone());

        // 3. Wire Component (Channel) - CRITICAL for Live Data
        let (tx, rx) = mpsc::channel();
        adapter.set_monitor(tx);
        engine.start_gateway_listener(rx).unwrap();
        
        // 4. Get Symbol and Subscribe
        let list = adapter.get_night_future_list()?;
        if list.is_empty() {
             println!("No night futures found.");
             return Ok(());
        }
        
        let first = &list[0];
        let symbol = first["futs_shrn_iscd"].as_str().unwrap_or("").to_string();
        println!("Subscribing to {}", symbol);
        
        adapter.subscribe(&symbol)?;
        
        // 5. Monitor Loop (10 seconds)
        println!("Starting monitor loop (10s)...");
        for i in 0..10 {
            thread::sleep(Duration::from_secs(1));
            
            if let Some(book) = engine.get_order_book(&symbol) {
                println!("{}", book);
            } else {
                println!("[{}] {} | No Book Yet", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"), symbol);
            }
        }
        
        println!("Stopping logger explicitly to ensure S3 flush...");
        logger.lock().unwrap().stop();
        
        Ok(())
    }
}
