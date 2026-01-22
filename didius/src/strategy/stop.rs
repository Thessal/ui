use crate::oms::order::{Order, OrderSide, OrderType, OrderState};
use crate::oms::order_book::OrderBook;
use crate::strategy::base::{Strategy, StrategyAction};
use anyhow::Result;
use rust_decimal::prelude::*;

pub struct StopStrategy {
    pub trigger_price: f64,
    pub order_to_send: Order,
    pub triggered: bool,
}

impl StopStrategy {
    pub fn new(trigger_price: f64, order: Order) -> Self {
        StopStrategy {
            trigger_price,
            order_to_send: order,
            triggered: false,
        }
    }
}

impl Strategy for StopStrategy {
    fn on_order_book_update(&mut self, _book: &OrderBook) -> Result<StrategyAction> {
        // Stop triggers usually based on Last Trade Price, but can be Mark Price or Best Bid/Ask without trade.
        // If we use Best Bid/Ask:
        // Buy Stop: Trigger if Best Ask >= Trigger? Or Last Price >= Trigger.
        // Sell Stop: Trigger if Best Bid <= Trigger?
        
        // For simplicity, let's assume we triggered on trade update, not book update.
        // But if user wants us to use MidPrice or something?
        Ok(StrategyAction::None)
    }

    fn on_trade_update(&mut self, price: f64) -> Result<StrategyAction> {
        if self.triggered {
            return Ok(StrategyAction::None);
        }

        let side = &self.order_to_send.side;
        // Buy Stop: Trigger if Price >= Trigger Price (Stop Entry) or Price <= Trigger (Stop Loss)?
        // Usually:
        // Sell Stop-Loss: Current Price drops BELOW trigger.
        // Buy Stop-Entry: Current Price rises ABOVE trigger.
        
        // Convention:
        // If Side=SELL, it's a Stop Loss (or Take Profit if trigger > current?).
        // Let's assume standard Stop Loss for SELL: if price <= trigger.
        // Let's assume Buy Stop: if price >= trigger.
        
        let should_trigger = match side {
            OrderSide::SELL => price <= self.trigger_price,
            OrderSide::BUY => price >= self.trigger_price,
        };

        if should_trigger {
            self.triggered = true;
            let mut o = self.order_to_send.clone();
            // Ensure state is New/Pending
            o.state = OrderState::CREATED; // Reset state so engine processes it as new
            return Ok(StrategyAction::PlaceOrder(o));
        }

        Ok(StrategyAction::None)
    }
}
