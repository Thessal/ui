use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub quantity: i64,
    pub average_price: f64,
    pub current_price: f64,
}

impl Position {
    pub fn new(symbol: String, quantity: i64, average_price: f64, current_price: f64) -> Self {
        Position {
            symbol,
            quantity,
            average_price,
            current_price,
        }
    }

    pub fn unrealized_pnl(&self) -> f64 {
        (self.current_price - self.average_price) * self.quantity as f64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountState {
    pub balance: f64,
    pub locked: f64,
    pub positions: HashMap<String, Position>,
}

impl AccountState {
    pub fn new() -> Self {
        AccountState {
            balance: 0.0,
            locked: 0.0,
            positions: HashMap::new(),
        }
    }

    pub fn rebuild(&mut self, balance: f64, locked: f64, positions: Vec<Position>) {
        self.balance = balance;
        self.locked = locked;
        self.positions.clear();
        for p in positions {
            self.positions.insert(p.symbol.clone(), p);
        }
    }

    pub fn update_position(&mut self, symbol: String, quantity: i64, price: f64) {
        if quantity == 0 {
            self.positions.remove(&symbol);
        } else {
            self.positions.insert(
                symbol.clone(),
                Position {
                    symbol,
                    quantity,
                    average_price: price,
                    current_price: 0.0, // Default or previous? Python sets simple constructor.
                },
            );
        }
    }

    pub fn on_execution(&mut self, symbol: String, side: String, quantity: i64, price: f64, fee: f64) {
        let signed_qty = if side == "BUY" { quantity } else { -quantity };
        let cost = signed_qty as f64 * price;

        self.balance -= cost;
        self.balance -= fee;

        if !self.positions.contains_key(&symbol) {
             self.positions.insert(
                 symbol.clone(),
                 Position {
                     symbol,
                     quantity: signed_qty,
                     average_price: price,
                     current_price: price,
                 },
             );
        } else {
             if let Some(pos) = self.positions.get_mut(&symbol) {
                 let old_qty = pos.quantity;
                 let new_qty = old_qty + signed_qty;

                 if new_qty == 0 {
                     self.positions.remove(&symbol);
                 } else {
                     // Check for increase or flip
                     if (old_qty > 0 && signed_qty > 0) || (old_qty < 0 && signed_qty < 0) {
                         let total_val = (old_qty as f64 * pos.average_price) + (signed_qty as f64 * price);
                         pos.average_price = total_val / new_qty as f64;
                     } else if (old_qty > 0 && new_qty < 0) || (old_qty < 0 && new_qty > 0) {
                         // Flip
                         pos.average_price = price;
                     }
                     // Else reducing, avg price stays same.

                     pos.quantity = new_qty;
                     pos.current_price = price;
                 }
             }
        }
    }
}
