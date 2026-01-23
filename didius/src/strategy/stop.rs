use crate::oms::order::{Order, OrderSide, OrderType, OrderState, ExecutionStrategy};
use crate::oms::order_book::OrderBook;
use crate::strategy::base::{Strategy, StrategyAction};
use anyhow::Result;
use rust_decimal::prelude::*;
use chrono::Local;

pub struct StopStrategy {
    pub original_order_id: String,
    pub original_symbol: String,
    pub original_side: OrderSide,
    pub original_qty: i64,      // Total Qty
    
    pub trigger_side: OrderSide, // Logic: BUY means monitor Bid >= Trigger. SELL means monitor Ask <= Trigger.
    pub trigger_price: Decimal,
    pub trigger_timestamp: f64,
    
    pub stop_limit_price: Option<Decimal>, // New Order Price (or None for Market)
    
    pub filled_qty: i64,        // Track fills for original order
    pub triggered: bool,
    pub finished: bool,
}

impl StopStrategy {
    pub fn new(
        original_order_id: String,
        original_symbol: String,
        original_side: OrderSide,
        original_qty: i64,
        trigger_side: OrderSide, // TODO: trigger_side = order_side
        trigger_price: Decimal, 
        trigger_timestamp: f64, 
        stop_limit_price: Option<Decimal>
    ) -> Self {
        StopStrategy {
            original_order_id,
            original_symbol,
            original_side,
            original_qty,
            trigger_side,
            trigger_price,
            trigger_timestamp,
            stop_limit_price,
            filled_qty: 0,
            triggered: false,
            finished: false,
        }
    }

    fn check_trigger(&mut self, book: Option<&OrderBook>) -> bool {
         if self.trigger_timestamp > 0.0 {
             let now = Local::now().timestamp_millis() as f64 / 1000.0;
             if now >= self.trigger_timestamp {
                 return true;
             }
         }
         
         if let Some(b) = book {
              match self.trigger_side {
                 OrderSide::SELL => {
                     // Trigger if Ask <= Price (Price dropped to trigger)
                     if let Some((best_ask, _)) = b.get_best_ask() {
                         if best_ask <= self.trigger_price { return true; }
                     }
                 },
                 OrderSide::BUY => {
                     // Trigger if Bid >= Price (Price rose to trigger)
                     if let Some((best_bid, _)) = b.get_best_bid() {
                         if best_bid >= self.trigger_price { return true; }
                     }
                 }
             }
         }
         
         false
    }
}

impl Strategy for StopStrategy {
    fn on_order_book_update(&mut self, book: &OrderBook) -> Result<StrategyAction> {
        if self.triggered || self.finished {
            return Ok(StrategyAction::None);
        }
        
        if self.check_trigger(Some(book)) {
            self.triggered = true;
            return Ok(StrategyAction::CancelOrder(self.original_order_id.clone()));
        }
        
        Ok(StrategyAction::None)
    }
    
    fn on_trade_update(&mut self, _price: f64) -> Result<StrategyAction> {
        // TODO: last trade price trigger
        Ok(StrategyAction::None)
    }
    
    fn on_timer(&mut self) -> Result<StrategyAction> {
        if self.triggered || self.finished {
            return Ok(StrategyAction::None);
        }
        
        if self.check_trigger(None) {
            self.triggered = true;
            return Ok(StrategyAction::CancelOrder(self.original_order_id.clone()));
        }
        
        Ok(StrategyAction::None)
    }
    
    fn on_order_status_update(&mut self, order: &Order) -> Result<StrategyAction> {
        let order_id = order.order_id.as_deref().unwrap_or("");
        
        if order_id != self.original_order_id {
            return Ok(StrategyAction::None);
        }
        
        // Check for FILLED (Finished naturally)
        if order.state == OrderState::FILLED {
            self.finished = true;
            return Ok(StrategyAction::None);
        }
        
        if order.state == OrderState::CANCELED && self.triggered {
            let filled = order.filled_quantity;
            let remaining = self.original_qty - filled;
            
            if remaining <= 0 {
                return Ok(StrategyAction::None);
            }
            
            let o = Order::new(
                self.original_symbol.clone(),
                self.original_side.clone(),
                OrderType::LIMIT,
                remaining,
                self.stop_limit_price.as_ref().map(|d| d.to_string()),
                Some(ExecutionStrategy::NONE),
                None, None
            );
            
            return Ok(StrategyAction::PlaceOrder(o));
        }

        Ok(StrategyAction::None)
    }
}
