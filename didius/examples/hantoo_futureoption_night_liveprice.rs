use didius::adapter::hantoo_ngt_futopt::HantooNightAdapter;
use didius::adapter::IncomingMessage;
use anyhow::Result;
use std::sync::mpsc;
use std::time::Duration;
use std::thread;

fn main() -> Result<()> {
    // 1. Initialize Adapter
    println!("Initializing Adapter...");
    let adapter = HantooNightAdapter::new("auth/hantoo.yaml")?;
    adapter.set_debug_mode(true);
    
    // 2. Setup Channel
    let (tx, rx) = mpsc::channel();
    adapter.set_monitor(tx);
    
    // 3. Get List and Pick First Symbol
    println!("Fetching Night Future List...");
    let list = adapter.get_night_future_list()?;
    if list.is_empty() {
        println!("No night futures available.");
        return Ok(());
    }
    
    let first_item = &list[0];
    let symbol = first_item["futs_shrn_iscd"].as_str().unwrap_or("").to_string();
    let name = first_item["hts_kor_isnm"].as_str().unwrap_or("Unknown");
    
    if symbol.is_empty() {
        println!("Invalid symbol in list item: {:?}", first_item);
        return Ok(());
    }
    
    println!("Selected Symbol: {} ({})", symbol, name);
    
    // 4. Subscribe
    println!("Subscribing to {}...", symbol);
    adapter.subscribe(&symbol)?;
    
    // 5. Listen Loop
    println!("Listening for messages (Ctrl+C to stop)...");
    
    let timeout = Duration::from_secs(60); 
    let start = std::time::Instant::now();
    
    loop {
        // Stop after timeout
        if start.elapsed() > timeout {
             break;
        }

        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(msg) => {
                match msg {
                    IncomingMessage::Trade(trade) => {
                        println!("[TRADE] {} | Price: {} | Qty: {}", trade.symbol, trade.price, trade.quantity);
                    },
                    _ => println!("[OTHER] {:?}", msg),
                }
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Heartbeat / waiting
            },
            Err(e) => {
                println!("Channel closed: {}", e);
                break;
            }
        }
    }
    
    Ok(())
}
