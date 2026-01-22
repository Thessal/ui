use crate::oms::order::Order;
use crate::oms::order_book::OrderBook;
use anyhow::Result;

#[derive(Debug, Clone)]
pub enum StrategyAction {
    PlaceOrder(Order),
    CancelOrder(String), // order_id
    None,
}

pub trait Strategy {
    // Check if the strategy should trigger based on market data (OrderBook updates, Trade updates, etc.)
    fn on_order_book_update(&mut self, book: &OrderBook) -> Result<StrategyAction>;
    fn on_trade_update(&mut self, price: f64) -> Result<StrategyAction>;
    fn on_order_status_update(&mut self, order_id: &str, state: crate::oms::order::OrderState) -> Result<StrategyAction> {
        Ok(StrategyAction::None)
    }
}
