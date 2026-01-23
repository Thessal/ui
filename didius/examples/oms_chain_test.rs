use anyhow::Result;
use didius::adapter::hantoo::HantooAdapter;
use didius::adapter::Adapter;
use didius::logger::{
    config::{LogDestinationInfo, LoggerConfig},
    Logger,
};
use didius::oms::engine::OMSEngine;
use didius::oms::order::{ExecutionStrategy, Order, OrderSide, OrderState, OrderType};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::io::{self, Write};
use std::str::FromStr;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use chrono::Local;

fn main() -> Result<()> {
    println!("Initializing HantooAdapter (Stock)...");
    // Ensure auth/hantoo.yaml exists or adjust path
    let adapter = Arc::new(HantooAdapter::new("auth/hantoo.yaml")?);

    let dest_console = LogDestinationInfo::Console;
    let logger_config = LoggerConfig {
        destination: dest_console,
        flush_interval_seconds: 60,
        batch_size: 1024 * 8,
    };
    let logger = Arc::new(Mutex::new(Logger::new(logger_config)));
    logger.lock().unwrap().start();

    // Margin req can be 1.0 for cash
    let engine = OMSEngine::new(adapter.clone(), 1.0, logger.clone());

    let (tx, rx) = mpsc::channel();
    adapter.set_monitor(tx);
    adapter.set_debug_mode(true);
    engine.start_gateway_listener(rx).unwrap();
    
    // User Input: Symbol
    let mut symbol = String::new();
    print!("Enter Stock Symbol (e.g. 005930): ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut symbol)?;
    let symbol = symbol.trim().to_string();
    
    if symbol.is_empty() {
        println!("Symbol cannot be empty.");
        return Ok(());
    }

    // User Input: Tick Size
    let mut tick_str = String::new();
    print!("Enter Tick Size (e.g. 100): ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut tick_str)?;
    let tick_size = Decimal::from_str(tick_str.trim()).unwrap_or(Decimal::new(100, 0));
    println!("Using Symbol: {}, Tick Size: {}", symbol, tick_size);

    adapter.subscribe_market(&[symbol.clone()])?;
    adapter.connect()?;

    println!("Started. Spawning status printer...");

    let engine_print = engine.clone();
    let symbol_print = symbol.clone();
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(1));
            // Dump Status
            let orders = engine_print.get_orders();
            let mut summary = String::new();
            summary.push_str(&format!(
                "\n--- OMS Status ---\nActive Orders: {}\n",
                orders.len()
            ));
            for (id, o) in &orders {
                let strategy = o.strategy.clone();
                let strat_name = match strategy {
                   ExecutionStrategy::CHAIN => "CHAIN",
                   ExecutionStrategy::NONE => "NONE",
                   ExecutionStrategy::FOK => "FOK",
                   ExecutionStrategy::IOC => "IOC",
                   _ => "OTHER"
                };
                
                summary.push_str(&format!(
                    "  [{}] {:?} {} @ {} (State: {:?}, Filled: {}) Strategy: {}\n",
                    id, o.side, o.quantity, o.price.map(|p| p.to_string()).unwrap_or("MKT".into()), o.state, o.filled_quantity, strat_name
                ));
            }
            if let Some(book) = engine_print.get_order_book(&symbol_print) {
                if let Some((bp, bq)) = book.get_best_bid() {
                    summary.push_str(&format!("  Book: Bid {} x {}\n", bp, bq));
                } else {
                     summary.push_str("  Book: Bid None\n");
                }
                if let Some((ap, aq)) = book.get_best_ask() {
                    summary.push_str(&format!("  Book: Ask {} x {}\n", ap, aq));
                } else {
                     summary.push_str("  Book: Ask None\n");
                }
            } else {
                summary.push_str("  Book: None\n");
            }
            println!("{}", summary);
        }
    });

    println!("Interactive Mode:");
    println!("  [b] Buy Trigger (Buy -1tick, Trigger @ Ask, Chain Buy +5tick)");
    println!("  [s] Sell Trigger (Sell +1tick, Trigger @ Bid, Chain Sell -5tick)");
    println!("  [q] Quit");

    loop {
        print!("> ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let cmd = input.trim();

        match cmd {
            "b" | "s" => {
                // Get current price
                let best_ask = engine
                    .get_order_book(&symbol)
                    .and_then(|b| b.get_best_ask().map(|(p, _)| p))
                    .unwrap_or(Decimal::ZERO);
                let best_bid = engine
                    .get_order_book(&symbol)
                    .and_then(|b| b.get_best_bid().map(|(p, _)| p))
                    .unwrap_or(Decimal::ZERO);

                if best_ask == Decimal::ZERO || best_bid == Decimal::ZERO {
                    println!("No market data yet.");
                    continue;
                }

                // Chain Timeout: 5s
                let timeout_timestamp = Local::now().timestamp_millis() as f64 / 1000.0 + 5.0;
                println!("Timeout set to: {}", timeout_timestamp);

                if cmd == "b" {
                    println!("Sending Chain Buy Order...");
                    
                    let mut params = HashMap::new();
                    // Trigger Logic: Time or Price
                    params.insert("trigger_timestamp".to_string(), timeout_timestamp.to_string());
                    // Trigger if Best Ask <= price? (unlikely for buy). 
                    // StopStrategy BUY: Trigger if Bid >= trigger_price. 
                    // Let's use Time Trigger mainly.
                    
                    // Trigger Price for testing (e.g. if Price drops below X? Stop Loss?)
                    // Stop Buy (Buy Stop): Trigger if Price >= Trigger. Then Buy.
                    // Here we are placing a Limit Buy (Passive) and want to switch to Aggressive if not filled.
                    // This is "Chain". refactored to generic Stop.
                    // trigger_price can be unused if timestamp is set.
                    
                    let aggressive_price = best_ask + (tick_size * Decimal::from(5));
                    params.insert("chained_price".to_string(), aggressive_price.to_string());
                    params.insert("trigger_side".to_string(), "BUY".to_string());
                    params.insert("trigger_price".to_string(), best_ask.to_string());

                    let order = Order::new(
                        symbol.clone(),
                        OrderSide::BUY,
                        OrderType::LIMIT,
                        1, 
                        Some((best_bid - tick_size).to_string()), // Passive
                        Some(ExecutionStrategy::STOP),
                        Some(params),
                        None
                    );
                    
                    match engine.send_order_internal(order) {
                        Ok(id) => println!("Order Sent: {}", id),
                        Err(e) => println!("Error sending order: {}", e),
                    }
                } else if cmd == "s" {
                     println!("Sending Chain Sell Order...");
                    
                    let mut params = HashMap::new();
                    params.insert("trigger_timestamp".to_string(), timeout_timestamp.to_string());
                    
                    let aggressive_price = best_bid - (tick_size * Decimal::from(5));
                    params.insert("chained_price".to_string(), aggressive_price.to_string());
                    params.insert("trigger_side".to_string(), "SELL".to_string());
                    params.insert("trigger_price".to_string(), best_bid.to_string());
                    
                    let order = Order::new(
                        symbol.clone(),
                        OrderSide::SELL,
                        OrderType::LIMIT,
                        1, 
                        Some((best_ask + tick_size).to_string()), 
                        Some(ExecutionStrategy::STOP),
                        Some(params),
                        None
                    );
                     match engine.send_order_internal(order) {
                        Ok(id) => println!("Order Sent: {}", id),
                        Err(e) => println!("Error sending order: {}", e),
                    }
                }


            }
            "q" => break,
            _ => {}
        }
    }

    Ok(())
}
