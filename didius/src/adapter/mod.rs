use crate::oms::order::Order;
use crate::oms::order_book::OrderBook;
use crate::oms::account::AccountState;
use anyhow::Result;

pub trait Adapter: Send + Sync {
    fn connect(&self) -> Result<()>;
    fn disconnect(&self) -> Result<()>;
    fn place_order(&self, order: &Order) -> Result<bool>;
    fn cancel_order(&self, order_id: &str) -> Result<bool>;
    fn get_order_book_snapshot(&self, symbol: &str) -> Result<OrderBook>;
    fn get_account_snapshot(&self, account_id: &str) -> Result<AccountState>;
}

pub mod mock;
pub mod hantoo;
