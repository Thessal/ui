use didius_oms::adapter::hantoo::HantooAdapter;
use didius_oms::adapter::Adapter;
use didius_oms::oms::order::{Order, OrderSide, OrderType};
use didius_oms::logger::Logger;
use didius_oms::logger::config::{LoggerConfig, LogDestinationInfo};
use didius_oms::logger::message::Message;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;
use log::info;

fn main() {
    // Setup env logger (console)
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    // Setup Custom Logger (JSONL)
    let log_config = LoggerConfig {
        destination: LogDestinationInfo::LocalFile { path: "logs/log.jsonl".to_string() },
        flush_interval_seconds: 1, // Fast flush for check
        batch_size: 1,
    };
    let mut logger = Logger::new(log_config);
    logger.start();
    
    // Helper to log both ways
    let log_event = |l: &Logger, event: &str, data: serde_json::Value| {
        let msg = Message::new(event.to_string(), data);
        l.log(msg);
    };

    // Ensure config exists or create dummy
    let config_path = "auth/hantoo.yaml";
    if !Path::new(config_path).exists() {
        eprintln!("Warning: {} not found. Trade check requires real credentials.", config_path);
    }

    info!("Initializing HantooAdapter...");
    log_event(&logger, "INIT", serde_json::json!({"msg": "Initializing HantooAdapter"}));
    
    match HantooAdapter::new(config_path) {
        Ok(adapter) => {
            info!("Adapter initialized.");
            
            // Connect
            if let Err(e) = adapter.connect() {
                eprintln!("Failed to connect: {}", e);
                log_event(&logger, "ERROR", serde_json::json!({"msg": "Failed to connect", "error": e.to_string()}));
                logger.stop();
                return;
            }

            // Create Order
            // Buy "001360" at 1601
            let mut order = Order::new(
                "001360".to_string(),
                OrderSide::BUY,
                OrderType::LIMIT,
                1, // Qty 1
                Some(1601.0),
                None, // Strategy
                None, // Params
                None, // Stop Price
            );
            
            // Set Client Order ID
            let order_id = "trade_check_manual_1".to_string();
            order.order_id = Some(order_id.clone());

            info!("Placing Order: {:?}", order);
            log_event(&logger, "ORDER_PLACE_REQ", serde_json::json!({"order": order}));

            match adapter.place_order(&order) {
                Ok(true) => {
                    info!("Order placed successfully. Waiting 10s...");
                    log_event(&logger, "ORDER_PLACED", serde_json::json!({"order_id": order_id}));
                    
                    thread::sleep(Duration::from_secs(10));
                    
                    info!("Cancelling Order: {}", order_id);
                    log_event(&logger, "ORDER_CANCEL_REQ", serde_json::json!({"order_id": order_id}));
                    
                    match adapter.cancel_order(&order_id) {
                        Ok(true) => {
                            info!("Cancellation successful.");
                            log_event(&logger, "ORDER_CANCELLED", serde_json::json!({"order_id": order_id}));
                        },
                        Ok(false) => {
                            eprintln!("Cancellation returned false.");
                            log_event(&logger, "ORDER_CANCEL_FAIL", serde_json::json!({"order_id": order_id, "reason": "returned false"}));
                        },
                        Err(e) => {
                            eprintln!("Cancellation error: {}", e);
                            log_event(&logger, "ORDER_CANCEL_ERROR", serde_json::json!({"order_id": order_id, "error": e.to_string()}));
                        },
                    }
                },
                Ok(false) => {
                    eprintln!("Order placement returned false.");
                    log_event(&logger, "ORDER_PLACE_FAIL", serde_json::json!({"order_id": order_id, "reason": "returned false"}));
                },
                Err(e) => {
                    eprintln!("Order placement error: {}", e);
                    log_event(&logger, "ORDER_PLACE_ERROR", serde_json::json!({"order_id": order_id, "error": e.to_string()}));
                },
            }
        },
        Err(e) => eprintln!("Failed to init adapter: {}", e),
    }
    
    // Explicit stop to ensure flush
    logger.stop();
}
