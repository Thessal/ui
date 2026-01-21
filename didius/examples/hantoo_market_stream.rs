use didius::adapter::hantoo::HantooAdapter;
use didius::adapter::{Adapter, IncomingMessage};
use didius::oms::engine::OMSEngine;
use didius::logger::Logger;
use didius::logger::config::{LoggerConfig, LogDestinationInfo};
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    // 1. Setup Logger (S3, 30s flush)
    let log_config = LoggerConfig {
        destination: LogDestinationInfo::AmazonS3 {
            bucket: "didius".to_string(),
            key_prefix: "logs".to_string(),
            region: "ap-northeast-2".to_string(),
        },
        flush_interval_seconds: 30,
        batch_size: 100,
    };
    
    let logger = Arc::new(Mutex::new(Logger::new(log_config)));
    
    // Start Logger
    {
        let mut l = logger.lock().unwrap();
        l.start();
    }
    
    println!("Logger started (S3/30s)");

    // 2. Initialize Adapter
    let _config_path = "config/hantoo_config.yaml"; // Assumed existence or use hantoo.yaml in auth implementation path
    // Actually hantoo adapter reads from "config_path".
    // I should check if that file exists or create a placeholder.
    // The previous hantoo_check used "auth/hantoo.yaml"? No, adapter::new takes config path.
    // Let's assume passed path.
    
    // Wait, HantooAdapter::new takes "config_path".
    // The user's repo likely has it. hantoo_check.rs used "auth/hantoo.yaml".
    let adapter = Arc::new(HantooAdapter::new("auth/hantoo.yaml")?);
    
    // 3. Setup Channel
    let (tx, rx) = mpsc::channel::<IncomingMessage>();
    
    // 4. Wire up Adapter
    adapter.set_monitor(tx);
    
    // Configure Subscription for Samsung (005930) and SK Hynix (000660)
    let symbols = vec!["005930".to_string(), "000660".to_string()];
    adapter.subscribe_market(&symbols)?;
    
    // 5. Initialize Engine
    let engine = OMSEngine::new(
        adapter.clone(),
        0.1, // Margin
        logger.clone()
    );
    
    // 6. Connect Adapter (Starts WS thread)
    adapter.set_debug_mode(true);
    adapter.connect()?;
    
    println!("Adapter Connected & WS Thread Started (Subscribed to {:?})", symbols);
    
    // 7. Start Gateway Listener (Logs & Updates Engine)
    engine.start_gateway_listener(rx).expect("Failed to start listener");
    
    println!("Gateway Listener Started");
    
    // 8. Main Loop: Print OrderBook every 5s
    loop {
        thread::sleep(Duration::from_secs(5));
        
        for symbol in &symbols {
            if let Some(book) = engine.get_order_book(symbol) {
                println!("{}", book);
            } else {
                println!("OrderBook for {} not yet available.", symbol);
            }
        }
    }
}
