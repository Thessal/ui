use crate::oms::order::{Order, OrderSide, OrderType, OrderState, ExecutionStrategy};
use crate::oms::order_book::OrderBook;
use crate::strategy::base::Strategy;
use anyhow::{Result, anyhow};
use rust_decimal::prelude::*;
use std::collections::HashMap;

pub struct VWAPStrategy {
    side: OrderSide,
    limit_price: Option<f64>,
    total_volume: i64,
    interval_sec: f64,
    timeout_sec: f64,
    
    start_time: f64,
    next_trigger_sec: f64,
    remaining_volume: i64,
    
    current_slice_order_id: Option<String>,
    waiting_for_cancel: bool,
    
    last_known_price: f64,
}

impl VWAPStrategy {
    pub fn new(
        side: OrderSide,
        price: Option<f64>,
        volume: i64,
        interval: f64,
        timeout: f64,
    ) -> Self {
        let now = chrono::Local::now().timestamp_millis() as f64 / 1000.0;
        VWAPStrategy {
            side,
            limit_price: price,
            total_volume: volume,
            interval_sec: interval,
            timeout_sec: timeout,
            start_time: now,
            next_trigger_sec: now + interval,
            remaining_volume: volume,
            current_slice_order_id: None,
            waiting_for_cancel: false,
            last_known_price: 0.0,
        }
    }

    fn create_slice_order(&self, price: f64) -> Order {
        // Calculate Slice Size
        // Time Remaining
        let now = chrono::Local::now().timestamp_millis() as f64 / 1000.0;
        let elapsed = now - self.start_time;
        let intervals_passed = (elapsed / self.interval_sec).ceil() as f64;
        let total_intervals = (self.timeout_sec / self.interval_sec).ceil() as f64;
        let remaining_intervals = (total_intervals - intervals_passed).max(1.0);
        
        let target_slice = (self.remaining_volume as f64 / remaining_intervals).ceil() as i64;
        let quantity = target_slice.min(self.remaining_volume);
        
        // Aggressive Price: +/- 3 ticks (assuming tick=0.1 for now or 0.01%?)
        // User said "3 tick price". Without tick size info, reasonable assumption is needed or use best bid/ask +/- offset.
        // Let's use 0.1% for now.
        let spread = price * 0.001; 
        let exec_price = match self.side {
             OrderSide::BUY => price + spread,
             OrderSide::SELL => price - spread,
        };
        
        // Cap with Limit Price
        let final_price = if let Some(lp) = self.limit_price {
            match self.side {
                OrderSide::BUY => exec_price.min(lp),
                OrderSide::SELL => exec_price.max(lp),
            }
        } else {
            exec_price
        };

        Order::new(
            "VWAP".to_string(), // Symbol? We need to store symbol in strategy!
            self.side.clone(),
            OrderType::LIMIT,
            quantity,
            Some(final_price),
            Some(ExecutionStrategy::NONE), // Child orders don't have strategy? Or marked?
            None,
            None
        )
    }
}

impl Strategy for VWAPStrategy {
    fn on_order_book_update(&mut self, book: &OrderBook) -> Result<Option<Order>> {
        let now = book.timestamp; // Use book time
        // or Local::now()? Book time is safer for simulation constraints.
        
        // Update price
        if let Some(mid) = book.get_mid_price() {
             self.last_known_price = mid.to_f64().unwrap_or(0.0);
        }
        
        // Check Trigger
        if now >= self.next_trigger_sec && self.remaining_volume > 0 {
             // Need to slice.
             // If we have active order, cancel it first.
             if let Some(oid) = &self.current_slice_order_id {
                 if !self.waiting_for_cancel {
                     self.waiting_for_cancel = true;
                     // Create Cancel Order? 
                     // The Strategy output is Option<Order>. A Cancel is an Action.
                     // The Order struct has state.
                     // But we usually send a "Cancel Request" object, or modify state.
                     // Interface `cancel_order` by ID.
                     // A Strategy returning `Order` usually implies placement.
                     // If we want to cancel, we might need a meta-enum `StrategyAction`?
                     // Or, the Engine interprets a specific Order state/type?
                     // Or we return `None` here but call engine directly? 
                     // Strategy doesn't hold engine ref.
                     
                     // Let's assume we return an Order with state PENDING_CANCEL and ID populated? 
                     // Engine: if order.state == PENDING_CANCEL, call cancel?
                     
                     let mut cancel_req = Order::new("".into(), OrderSide::BUY, OrderType::MARKET, 0, None, None, None, None);
                     cancel_req.order_id = Some(oid.clone());
                     cancel_req.state = OrderState::PENDING_CANCEL; 
                     
                     // Next trigger only after cancel resolved? 
                     // Or we optimistically advance trigger?
                     self.next_trigger_sec += self.interval_sec; 
                     return Ok(Some(cancel_req));
                 }
                 // If already waiting, do nothing until confirmed.
             } else {
                 // No active order, place new one.
                 let mut order = self.create_slice_order(self.last_known_price);
                 order.symbol = book.symbol.clone(); // Set symbol from book
                 self.current_slice_order_id = order.order_id.clone(); // It's None initially... engine assigns ID.
                 // Strategy needs to know ID to cancel later. 
                 // If we create order here, ID is None. Engine assigns it.
                 // We need to capture the ID assignment!
                 // Strategy interface limitation.
                 
                 // Solution: Identify order by ClientID or CorrelationID?
                 // Or, generating UUID here is better.
                 let uuid = uuid::Uuid::new_v4().to_string();
                 order.order_id = Some(uuid.clone());
                 self.current_slice_order_id = Some(uuid);
                 
                 self.next_trigger_sec += self.interval_sec;
                 return Ok(Some(order));
             }
        }
        
        Ok(None)
    }

    fn on_trade_update(&mut self, price: f64) -> Result<Option<Order>> {
        self.last_known_price = price;
        Ok(None)
    }
    
    fn on_order_status_update(&mut self, order_id: &str, state: OrderState) -> Result<Option<Order>> {
        if let Some(curr) = &self.current_slice_order_id {
            if curr == order_id {
                match state {
                    OrderState::FILLED => {
                        // Full fil
                        // How much? We need fill qty. `on_order_status_update` sig doesn't have qty.
                        // We need `on_trade_update` for fills?
                        // Assuming filled means slice done.
                        self.current_slice_order_id = None;
                        self.waiting_for_cancel = false;
                        
                        // Remaining volume tracking? 
                        // We need partial fill info.
                    },
                    OrderState::CANCELED | OrderState::REJECTED => {
                        self.current_slice_order_id = None;
                        self.waiting_for_cancel = false;
                        // Now we can place next slice immediately or wait for trigger?
                        // The logic says "cancel it and accumulates to the next".
                        // So we wait for next trigger (already set).
                    },
                    _ => {}
                }
            }
        }
        Ok(None)
    }
}
