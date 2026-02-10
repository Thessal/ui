use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;
use crate::oms::order::{Order, OrderState};
use crate::oms::order_book::OrderBookDelta;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Connection status changed
    ConnectionStatus(ConnectionStatus),
    
    /// Order Book Snapshot (Full Replace)
    OrderBookSnapshot(crate::oms::order_book::OrderBookSnapshot),

    /// Order Book Update (Delta)
    OrderBookUpdate {
        symbol: String,
        delta: OrderBookDelta,
    },
    
    /// Trade executed in the market (not our trade)
    MarketTrade {
        symbol: String,
        price: Decimal,
        quantity: i64,
        timestamp: f64,
    },
    
    /// Order Status Update (Our Order)
    OrderStatus {
        order_id: String,
        state: OrderState,
        filled_qty: i64,
        filled_price: Option<Decimal>,
        msg: Option<String>,
        updated_at: f64,
    },
    
    /// Account Balance/Position Update
    AccountUpdate {
        account_id: String,
        balance: Option<Decimal>,
        locked: Option<Decimal>,
        // potentially other fields
    },
    
    /// Execution Report
    Execution {
        order_id: String,
        fill_qty: i64,
        fill_price: Decimal,
    },

    /// Error Message
    Error {
        code: i32,
        message: String,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}
