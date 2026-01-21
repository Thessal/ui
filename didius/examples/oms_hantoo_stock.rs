use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use didius_oms::adapter::hantoo::HantooAdapter;
use didius_oms::oms::engine::OMSEngine;
use didius_oms::logger::Logger;
use didius_oms::logger::config::{LoggerConfig, LogDestinationInfo};
use std::sync::mpsc;
use didius_oms::adapter::Adapter;
// OrderBook now implements PartialEq/Eq

fn main() -> Result<()> {
    // let symbol1 = "005930".to_string(); // Samsung
    // let symbol2 = "000660".to_string(); // Sk Hynix
    
    println!("Initializing HantooAdapter...");
    let adapter = Arc::new(HantooAdapter::new("auth/hantoo.yaml")?);
    
    // Use S3 logger for production-like test
    let dest_s3 = 
        LogDestinationInfo::AmazonS3 { 
            bucket: "didius".to_string(),
            key_prefix: "logs".to_string(),
            region: "ap-northeast-2".to_string(),
        };
    let dest_console = LogDestinationInfo::Console;
    let logger_config = LoggerConfig {
        destination: dest_s3, 
        flush_interval_seconds: 60,
        batch_size: 1024,
    };
    let logger = Arc::new(Mutex::new(Logger::new(logger_config)));
    logger.lock().unwrap().start();
    
    let engine = OMSEngine::new(adapter.clone(), 0.1, logger.clone());
    
    // Channel
    let (tx, rx) = mpsc::channel();
    adapter.set_monitor(tx);
    engine.start_gateway_listener(rx).unwrap();
    
    // 1. Subscribe to KOSPI50 constituents
    println!("Example: Long-running Market Data Stream to S3");
    println!("Attempting to download KOSPI50 constituents...");
    use didius_oms::utils::universe::download_kospi_50;
    
    let symbols = match download_kospi_50() {
        Ok(list) => {
            if list.is_empty() {
                println!("Downloaded list is empty. Using fallback.");
                vec!["005930".to_string(), "000660".to_string()]
            } else {
                println!("Successfully downloaded {} KOSPI50 constituents.", list.len());
                // Optional: print first few?
                println!("First 5: {:?}", list.iter().take(5).collect::<Vec<_>>());
                list
            }
        },
        Err(e) => {
             println!("Failed to download KOSPI50: {}. Using fallback.", e);
             vec!["005930".to_string(), "000660".to_string()]
        }
    };
    //let symbols = vec!["005930".to_string(), "000660".to_string()];

    println!("Subscribing to {} symbols...", symbols.len());
    adapter.subscribe_market(&symbols)?;
    
    // Connect
    adapter.set_debug_mode(false);
    adapter.connect()?;
    
    println!("Waiting 5s for initial data...");
    thread::sleep(Duration::from_secs(5));

    // Gateway Listener Explanation:
    // The gateway listener is responsible for consuming messages from the Adapter via the channel (rx).
    // It processes:
    //  - OrderBookDelta/Snapshot: Updates the OMSEngine's internal OrderBook state.
    //  - Executions: Updates Order state and Account positions.
    //  - Trades: (Future implementation)
    //
    // IF THE GATEWAY LISTENER IS NOT STARTED:
    // 1. The OMSEngine's OrderBooks will NOT be updated. `engine.get_order_book(symbol)` will return None or stale data.
    // 2. Strategies consuming market data will NOT be triggered.
    // 3. Execution reports from the Adapter will NOT be processed, so Order status will remain pending/stale.
    // 4. The channel buffer might fill up if the Adapter continues to push messages.
    //
    // Therefore, starting the listener is critical for the OMS to function reactively.
    
    // 2. Long-Running Loop
    println!("Starting long-running loop. Logs will be flushed to S3 every 60s.");
    println!("Press Ctrl-C to stop.");
    
    loop {
        // Just keep the main thread alive. The background threads (Adapter WebSocket, Engine Gateway, Logger) do the work.
        // We can periodically print a status or just sleep.
        thread::sleep(Duration::from_secs(10));
        
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        println!("[{}] OMSEngine Running... (Subscribed: {})", now, symbols.len());
        
        // Optional: Print a random book to show it's alive?
        if !symbols.is_empty() {
             //let sample = &symbols[0];
             let sample = &("005930".to_string());
             if let Some(book) = engine.get_order_book(sample) {
                 println!("  Sample Book [{}]: Best Bid {} / Best Ask {} (UpdateID: {})", 
                    sample, 
                    book.get_best_bid().map(|(p,_)| p.to_string()).unwrap_or_default(),
                    book.get_best_ask().map(|(p,_)| p.to_string()).unwrap_or_default(),
                    book.last_update_id
                 );
             }
        }
    }
}
