use crate::oms::order::Order;
use crate::oms::order_book::OrderBook;
use crate::oms::account::{AccountState};
use crate::adapter::Adapter;
use anyhow::Result;
use std::sync::Mutex;
use rust_decimal::Decimal;
// use std::collections::HashMap;
use crate::adapter::IncomingMessage;

pub struct MockAdapter {
    account_state: Mutex<AccountState>,
}

impl MockAdapter {
    pub fn new() -> Self {
        MockAdapter {
            account_state: Mutex::new(AccountState::new()),
        }
    }
    
    pub fn with_account_state(state: AccountState) -> Self {
        MockAdapter {
            account_state: Mutex::new(state),
        }
    }
    
    pub fn set_account_state(&self, state: AccountState) {
        let mut guard = self.account_state.lock().unwrap();
        *guard = state;
    }
}

impl Adapter for MockAdapter {
    fn connect(&self) -> Result<()> {
        Ok(())
    }

    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn place_order(&self, _order: &Order) -> Result<bool> {
        Ok(true)
    }

    fn cancel_order(&self, _order_id: &str) -> Result<bool> {
        Ok(true)
    }

    fn get_order_book_snapshot(&self, symbol: &str) -> Result<OrderBook> {
        Ok(OrderBook::new(symbol.to_string()))
    }

    fn get_account_snapshot(&self, _account_id: &str) -> Result<AccountState> {
        // Return cloned state
        Ok(self.account_state.lock().unwrap().clone())
    }

    fn modify_order(&self, _order_id: &str, _price: Option<Decimal>, _qty: Option<i64>) -> Result<bool> {
        Ok(true) //TODO: what if the user want to modify whole remaining orders? Does the API support partial modify?
    }

    fn subscribe(&self, _symbols: &[String]) -> Result<()> {
        Ok(())
    }

    fn set_monitor(&self, _sender: std::sync::mpsc::Sender<IncomingMessage>) {
        // Mock adapter currently ignores the monitor, or we could store it to send mock messages later
    }
}
