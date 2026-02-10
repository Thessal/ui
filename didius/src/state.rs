use std::collections::HashMap;
use crate::message::{Message, ConnectionStatus};
use crate::oms::order::{Order, OrderState};
use crate::oms::order_book::OrderBook;
use crate::oms::account::AccountState;

#[derive(Debug, Clone)]
pub struct State {
    pub connection_status: ConnectionStatus,
    pub order_books: HashMap<String, OrderBook>,
    pub accounts: HashMap<String, AccountState>,
    pub orders: HashMap<String, Order>,
}

impl State {
    pub fn new() -> Self {
        Self {
            connection_status: ConnectionStatus::Disconnected,
            order_books: HashMap::new(),
            accounts: HashMap::new(),
            orders: HashMap::new(),
        }
    }

    pub fn apply(&mut self, msg: &Message) {
        match msg {
            Message::ConnectionStatus(status) => {
                self.connection_status = status.clone();
            }
            Message::OrderBookUpdate { symbol, delta } => {
                let book = self.order_books.entry(symbol.clone()).or_insert_with(|| OrderBook::new(symbol.clone()));
                book.apply_delta(delta); 
            }
            Message::OrderBookSnapshot(snapshot) => {
                 let book = self.order_books.entry(snapshot.symbol.clone()).or_insert_with(|| OrderBook::new(snapshot.symbol.clone()));
                 book.rebuild(snapshot.bids.clone(), snapshot.asks.clone(), snapshot.update_id, snapshot.timestamp);
            }
            Message::MarketTrade { .. } => {
                // Market trades might update Last Price, Volume, etc.
            }
            Message::OrderStatus { order_id, state, filled_qty, filled_price, .. } => {
                 if let Some(order) = self.orders.get_mut(order_id) {
                     order.state = state.clone();
                     order.filled_quantity = *filled_qty;
                     if let Some(price) = filled_price {
                         order.average_fill_price = *price; // Simplified
                     }
                 }
            }
            Message::AccountUpdate { account_id, balance, locked } => {
                let account = self.accounts.entry(account_id.clone()).or_insert_with(AccountState::new);
                if let Some(b) = balance {
                    account.balance = *b;
                }
                if let Some(l) = locked {
                    account.locked = *l;
                }
            }
            Message::Execution { order_id, fill_qty, fill_price: _ } => {
                 if let Some(order) = self.orders.get_mut(order_id) {
                     // Update order based on execution?
                     // Usually Execution implies OrderStatus update too, or we drive it here.
                     // For now just logging or simple update logic if State manages orders.
                     order.filled_quantity += fill_qty; 
                     // Update avg price... logic omitted for brevity but should be here.
                 }
            }
            Message::Error { .. } => {
            }
        }
    }
}
