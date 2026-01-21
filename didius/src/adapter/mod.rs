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

#[derive(Debug, Clone)]
pub enum IncomingMessage {
    OrderBookDelta(crate::oms::order_book::OrderBookDelta),
    Trade(Trade),
    Execution {
        order_id: String,
        fill_qty: i64,
        fill_price: f64,
    },
    OrderBookSnapshot(crate::oms::order_book::OrderBookSnapshot),
}

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
pub mod hantoo_ngt_futopt;
pub mod interface;
