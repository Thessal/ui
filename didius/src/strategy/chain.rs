use crate::oms::order::{Order, OrderSide, OrderType, OrderState};
use crate::oms::order_book::OrderBook;
use crate::strategy::base::{Strategy, StrategyAction};
use anyhow::Result;
use rust_decimal::prelude::*;
use tokio::time;

pub struct ChainStrategy {
    pub original_order_id: String,
    pub trigger_price_side: OrderSide,
    pub trigger_price: Decimal,
    pub trigger_timestamp: f64,
    pub chained_order: Order,
    pub triggered: bool,
}

impl ChainStrategy {
    // Original order is canceled and chained order is submitted when the trigger is activited.
    // Trigger is activated if : 
    // * current timestamp >= trigger timestamp 
    // * trigger_price_side is BUY and current bid price > trigger_price 
    // * trigger_price_side is SELL and current ask price < trigger_price
    pub fn new(original_order_id: String, trigger_price_side: OrderSide, trigger_price: Decimal, trigger_timestamp: f64, chained_order: Order) -> Self {
        ChainStrategy {
            original_order_id,
            trigger_price_side,
            trigger_price,
            trigger_timestamp,
            chained_order: chained_order,
            triggered: false,
        }
    }
}

impl Strategy for ChainStrategy {
    fn on_order_book_update(&mut self, book: &OrderBook) -> Result<StrategyAction> {
        // Assumes that the original order is submitted 
        if self.triggered {
            return Ok(StrategyAction::None);
        }
        
        let time_trig: bool = chrono::Local::now().timestamp_millis() as f64 / 1000.0 >= self.trigger_timestamp; 
        let price_trig = match self.trigger_price_side {
            OrderSide::SELL => book.get_best_ask().map_or(false, |(p,_q)| p <= self.trigger_price),
            OrderSide::BUY => book.get_best_bid().map_or(false, |(p,_q)| p >= self.trigger_price),
        };

        if time_trig || price_trig {
            self.triggered = true;
            return Ok(StrategyAction::CancelOrder(self.original_order_id.clone()));
        }
        Ok(StrategyAction::None)
    }
    
    fn on_trade_update(&mut self, _price: f64) -> Result<StrategyAction> {
        // TODO: enable triggering by last trade price
        Ok(StrategyAction::None)
    }
    
    fn on_order_status_update(&mut self, order_id: &str, state: OrderState) -> Result<StrategyAction> {
        // TODO : check if the order is canceled by the trigger (not canceled by the user)
        if order_id == self.original_order_id && state == OrderState::CANCELED {
            // Original order successfully canceled, place chained order
             let mut o = self.chained_order.clone();
             o.state = OrderState::CREATED; 
             return Ok(StrategyAction::PlaceOrder(o));
        }
        Ok(StrategyAction::None)
    }
}
