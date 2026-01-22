use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub quantity: i64,
    pub average_price: Decimal,
    pub current_price: Decimal,
}

impl Position {
    pub fn new(symbol: String, quantity: i64, average_price: Decimal, current_price: Decimal) -> Self {
        Position {
            symbol,
            quantity,
            average_price,
            current_price,
        }
    }

    pub fn unrealized_pnl(&self) -> Decimal {
        (self.current_price - self.average_price) * Decimal::from_i64(self.quantity).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountState {
    pub balance: Decimal,
    pub locked: Decimal,
    pub positions: HashMap<String, Position>,
}

impl AccountState {
    pub fn new() -> Self {
        AccountState {
            balance: Decimal::ZERO,
            locked: Decimal::ZERO,
            positions: HashMap::new(),
        }
    }

    pub fn rebuild(&mut self, balance: Decimal, locked: Decimal, positions: Vec<Position>) {
        self.balance = balance;
        self.locked = locked;
        self.positions.clear();
        for p in positions {
            self.positions.insert(p.symbol.clone(), p);
        }
    }

    pub fn update_position(&mut self, symbol: String, quantity: i64, price: Decimal) {
        if quantity == 0 {
            self.positions.remove(&symbol);
        } else {
            self.positions.insert(
                symbol.clone(),
                Position {
                    symbol,
                    quantity,
                    average_price: price,
                    current_price: Decimal::ZERO, // Default
                },
            );
        }
    }

    pub fn on_execution(&mut self, symbol: String, side: String, quantity: i64, price: Decimal, fee: Decimal) {
        let signed_qty = if side == "BUY" { quantity } else { -quantity };
        let signed_qty_dec = Decimal::from_i64(signed_qty).unwrap_or_default();
        let cost = signed_qty_dec * price;

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
                 
                 let old_qty_dec = Decimal::from_i64(old_qty).unwrap_or_default();
                 let new_qty_dec = Decimal::from_i64(new_qty).unwrap_or_default();

                 if new_qty == 0 {
                     self.positions.remove(&symbol);
                 } else {
                     // Check for increase or flip
                     if (old_qty > 0 && signed_qty > 0) || (old_qty < 0 && signed_qty < 0) {
                         // Weighted Average
                         let total_val = (old_qty_dec * pos.average_price) + (signed_qty_dec * price);
                         pos.average_price = total_val / new_qty_dec;
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
