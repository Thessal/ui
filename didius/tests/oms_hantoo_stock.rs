#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;
    use didius::adapter::hantoo::HantooAdapter;
    use didius::oms::engine::OMSEngine;
    use didius::logger::Logger;
    use didius::logger::config::LoggerConfig;
    use didius::adapter::Adapter; // Import trait
    use pyo3::prelude::*; // Import PyO3
    use rust_decimal::prelude::ToPrimitive;

    #[test]
    fn test_oms_hantoo_stock_integration() -> Result<()> {
        // 1. Initialize Engine with Logger (Flush interval 10s)
        println!("Initializing Logger and Engine...");
        let logger_config = LoggerConfig {
            flush_interval_seconds: 10,
            ..Default::default() 
        };
        let logger = Arc::new(Mutex::new(Logger::new(logger_config)));
        
        let adapter = Arc::new(HantooAdapter::new("auth/hantoo.yaml")?);
        let engine = OMSEngine::new(adapter.clone(), 0.15, logger);

        // 2. Start Engine
        // Connects Adapter internally, which subscribes to 005930
        println!("Starting Engine...");
        pyo3::prepare_freethreaded_python(); 
        Python::with_gil(|py| {
            engine.start(py, None).unwrap();
        });

        let symbol = "005930";
        println!("Test Symbol: {}", symbol);

        // 3. Comparison Loop (Run for 15s, check every 5s)
        let total_duration = Duration::from_secs(15);
        let check_interval = Duration::from_secs(5);
        let start_time = std::time::Instant::now();
        
        println!("Starting Comparison Loop...");
        while start_time.elapsed() < total_duration {
            thread::sleep(check_interval);
            
            println!("--- Comparison Check ---");
            
            // A. Get REST Snapshot
            let snapshot = adapter.get_order_book_snapshot(symbol)?;
            let rest_best_bid = snapshot.get_best_bid().map(|(p, _)| p.to_f64().unwrap_or(0.0)).unwrap_or(0.0);
            let rest_best_ask = snapshot.get_best_ask().map(|(p, _)| p.to_f64().unwrap_or(0.0)).unwrap_or(0.0);
            
            // B. Get Engine Live Snapshot
            let live_ob = engine.get_order_book(symbol).unwrap_or_else(|| {
                 didius::oms::order_book::OrderBook::new(symbol.to_string())
            });
            let live_best_bid = live_ob.get_best_bid().map(|(p, _)| p.to_f64().unwrap_or(0.0)).unwrap_or(0.0);
            let live_best_ask = live_ob.get_best_ask().map(|(p, _)| p.to_f64().unwrap_or(0.0)).unwrap_or(0.0);
            
            println!("REST Snapshot: {}", snapshot);
            println!("Live OrderBook: {}", live_ob);

            if live_best_bid > 0.0 || live_best_ask > 0.0 {
                if rest_best_bid != live_best_bid {
                    println!("WARNING: Bid Mismatch! REST={} vs Live={}", rest_best_bid, live_best_bid);
                } else {
                    println!("Bid Match!");
                }
                
                if rest_best_ask != live_best_ask {
                     println!("WARNING: Ask Mismatch! REST={} vs Live={}", rest_best_ask, live_best_ask);
                } else {
                    println!("Ask Match!");
                }
            } else {
                println!("Live OrderBook empty/incomplete yet. Waiting for WS data...");
            }
        }
        
        engine.stop(unsafe { Python::assume_gil_acquired() }).ok();
        println!("Test Completed.");
        Ok(())
    }
}
