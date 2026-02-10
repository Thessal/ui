use crate::oms::order::Order;
use crate::oms::order_book::OrderBook;
use crate::oms::account::AccountState;
use anyhow::Result;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub symbol: String,
    pub price: Decimal,
    pub quantity: i64,
    pub timestamp: f64,
}

pub use crate::message::Message as IncomingMessage;
use crate::message::Message;


pub trait Adapter: Send + Sync {
    fn connect(&self) -> Result<()>;
    fn disconnect(&self) -> Result<()>;
    fn place_order(&self, order: &Order) -> Result<bool>;
    fn cancel_order(&self, order_id: &str) -> Result<bool>;
    fn get_order_book_snapshot(&self, symbol: &str) -> Result<OrderBook>;
    fn get_account_snapshot(&self, account_id: &str) -> Result<AccountState>;
    fn modify_order(&self, order_id: &str, price: Option<Decimal>, qty: Option<i64>) -> Result<bool>;
    fn subscribe(&self, symbols: &[String]) -> Result<()>;
    fn set_monitor(&self, sender: std::sync::mpsc::Sender<IncomingMessage>);
}

pub mod mock;
pub mod hantoo;
pub mod hantoo_ngt_futopt;
pub mod interface;
