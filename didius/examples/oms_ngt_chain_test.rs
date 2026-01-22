use anyhow::Result;
use didius::adapter::hantoo_ngt_futopt::HantooNightAdapter;
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

fn main() -> Result<()> {
    println!("Initializing HantooNightAdapter...");
    let adapter = Arc::new(HantooNightAdapter::new("auth/hantoo.yaml")?);

    let dest_console = LogDestinationInfo::Console;
    let logger_config = LoggerConfig {
        destination: dest_console,
        flush_interval_seconds: 60,
        batch_size: 1024 * 8,
    };
    let logger = Arc::new(Mutex::new(Logger::new(logger_config)));
    logger.lock().unwrap().start();

    let engine = OMSEngine::new(adapter.clone(), 0.1, logger.clone());

    let (tx, rx) = mpsc::channel();
    adapter.set_monitor(tx);
    adapter.set_debug_mode(true);
    engine.start_gateway_listener(rx).unwrap();

    adapter.connect()?;

    println!("Fetching Night Future List...");
    let list = adapter.get_night_future_list()?;
    if list.is_empty() {
        println!("No Night Futures found.");
        return Ok(());
    }

    let first = &list[0];
    let symbol = first["futs_shrn_iscd"].as_str().unwrap_or("A05602").to_string(); // 'pdno' is symbol
    println!("Selected Symbol: {}", symbol);

    // Initial check of account (Optional, but user asked to print in this file)
    let acct = engine.get_account();
    println!("Initial Account Balance from Engine: {}", acct.balance);

    adapter.subscribe(&symbol)?;

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
                summary.push_str(&format!(
                    "  [{}] {:?} {} @ {:?} (State: {:?}, filled: {}) Strategy: {:?}\n",
                    id, o.side, o.quantity, o.price, o.state, o.filled_quantity, o.strategy
                ));
            }
            if let Some(book) = engine_print.get_order_book(&symbol_print) {
                if let Some((bp, bq)) = book.get_best_bid() {
                    summary.push_str(&format!("  Book: Bid {} x {}\n", bp, bq));
                }
                if let Some((ap, aq)) = book.get_best_ask() {
                    summary.push_str(&format!("  Book: Ask {} x {}\n", ap, aq));
                }
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

                let tick_size = Decimal::new(2, 2); // Assuming tick size 0.02
                println!("tick size : {}", tick_size);

                if cmd == "b" {
                    // "buy at current bp-1tick" -> Original Order
                    let buy_price = best_bid - tick_size;
                    // "chained with current ap+5tick ... current ap as trigger price"
                    let trigger_price = best_ask;
                    let chain_price = best_ask + (tick_size * Decimal::from(5));

                    println!(
                        "Placing Chain Order: Buy @ {}, Trigger @ {}, Chain Buy @ {}",
                        buy_price, trigger_price, chain_price
                    );

                    let mut order = Order::new(
                        symbol.clone(),
                        OrderSide::BUY,
                        OrderType::LIMIT,
                        1,
                        Some(buy_price.to_string()),
                        Some(ExecutionStrategy::CHAIN),
                        None,
                        None,
                    );

                    let mut params = HashMap::new();
                    params.insert("trigger_price".to_string(), trigger_price.to_string());
                    params.insert("trigger_side".to_string(), "BUY".to_string());
                    // Trigger Logic in ChainStrategy:
                    // BUY Trigger means: Bid >= Trigger? No.
                    // Implementation:
                    // OrderSide::BUY => book.get_best_bid()... >= self.trigger_price
                    // If we want to trigger when Price goes UP to Ask?
                    // User said: "current ap as trigger price".
                    // If we want to catch "Breakout", and we set trigger=Ask.
                    // If Price moves UP to Ask, Bid will eventually reach Ask.
                    // So TriggerSide BUY (monitor Bid) >= Trigger (Ask)?
                    // Logic seems compatible.

                    params.insert("trigger_timestamp".to_string(), "0".to_string()); // FIXME:Time trigger disabled?

                    params.insert("chained_symbol".to_string(), symbol.clone());
                    params.insert("chained_side".to_string(), "BUY".to_string());
                    params.insert("chained_quantity".to_string(), "1".to_string());
                    params.insert("chained_price".to_string(), chain_price.to_string());

                    order.strategy_params = params;

                    if let Ok(oid) = engine.send_order_internal(order) {
                        println!("Order Sent: {}", oid);
                    } else {
                        println!("Failed to send order");
                    }
                } else {
                    // "s"
                    // "sell at current ap+1tick" -> Original Order
                    let sell_price = best_ask + tick_size;
                    // "chained with current bp-5tick ... current bp as trigger price"
                    let trigger_price = best_bid;
                    let chain_price = best_bid - (tick_size * Decimal::from(5));

                    println!(
                        "Placing Chain Order: Sell @ {}, Trigger @ {}, Chain Sell @ {}",
                        sell_price, trigger_price, chain_price
                    );

                    let mut order = Order::new(
                        symbol.clone(),
                        OrderSide::SELL,
                        OrderType::LIMIT,
                        1,
                        Some(sell_price.to_string()),
                        Some(ExecutionStrategy::CHAIN),
                        None,
                        None,
                    );

                    let mut params = HashMap::new();
                    params.insert("trigger_price".to_string(), trigger_price.to_string());
                    params.insert("trigger_side".to_string(), "SELL".to_string());
                    // SELL means monitor Ask <= Trigger.
                    // If Price drops to Bid, Ask eventually drops to Bid.

                    params.insert("chained_symbol".to_string(), symbol.clone());
                    params.insert("chained_side".to_string(), "SELL".to_string());
                    params.insert("chained_quantity".to_string(), "1".to_string());
                    params.insert("chained_price".to_string(), chain_price.to_string());

                    order.strategy_params = params;

                    if let Ok(oid) = engine.send_order_internal(order) {
                        println!("Order Sent: {}", oid);
                    } else {
                        println!("Failed to send order");
                    }
                }
            }
            "q" => break,
            _ => {}
        }
    }

    Ok(())
}
