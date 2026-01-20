use crate::oms::order::Order;
use crate::oms::order_book::OrderBook;
use anyhow::Result;

pub trait Strategy {
    // Check if the strategy should trigger based on market data (OrderBook updates, Trade updates, etc.)
    // Returns Option<Order> if a child order should be placed, or some action taken.
    // Simplifying: assumes single order output for now.
    fn on_order_book_update(&mut self, book: &OrderBook) -> Result<Option<Order>>;
    fn on_trade_update(&mut self, price: f64) -> Result<Option<Order>>;
    fn on_order_status_update(&mut self, order_id: &str, state: crate::oms::order::OrderState) -> Result<Option<Order>> {
        Ok(None)
    }
}
