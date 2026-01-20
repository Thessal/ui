use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use rust_decimal::Decimal;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: Decimal,
    pub quantity: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookDelta {
    pub symbol: String,
    pub bids: Vec<(Decimal, i64)>,
    pub asks: Vec<(Decimal, i64)>,
    pub update_id: i64,
    pub timestamp: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    pub symbol: String,
    // Using simple BTreeMap might be sorted by price automatically? 
    // User requested consistency. BTreeMap<Decimal, i64> is sorted.
    // However, existing implementation used HashMap<String, i64> where String was likely stringified price?
    // The previous implementation used `HashMap<String, i64>`. 
    // I should probably switch to `BTreeMap<Decimal, i64>` for efficient get_best_bid/ask.
    // But let's stick to user request "change it into rust_decimal".
    // I will replace `HashMap<String, i64>` with `std::collections::BTreeMap<Decimal, i64>`.
    pub bids: std::collections::BTreeMap<Decimal, i64>, 
    pub asks: std::collections::BTreeMap<Decimal, i64>,

    pub last_update_id: i64,
    pub timestamp: f64,
}

impl OrderBook {
    pub fn new(symbol: String) -> Self {
        OrderBook {
            symbol,
            bids: std::collections::BTreeMap::new(),
            asks: std::collections::BTreeMap::new(),
            last_update_id: 0,
            timestamp: 0.0,
        }
    }

    pub fn get_bids(&self) -> std::collections::BTreeMap<Decimal, i64> {
        self.bids.clone()
    }

    pub fn rebuild(&mut self, bids: Vec<(Decimal, i64)>, asks: Vec<(Decimal, i64)>, last_update_id: i64, timestamp: f64) {
        self.bids.clear();
        for (p, q) in bids {
            self.bids.insert(p, q);
        }
        
        self.asks.clear();
        for (p, q) in asks {
            self.asks.insert(p, q);
        }
        
        self.last_update_id = last_update_id;
        self.timestamp = timestamp;
    }

    pub fn apply_delta(&mut self, delta: &OrderBookDelta) {
        if delta.symbol != self.symbol {
             return;
        }
        if delta.timestamp < self.timestamp {
             return;
        }

        // Bids
        for (price, qty) in &delta.bids {
            if *qty <= 0 {
                self.bids.remove(price);
            } else {
                self.bids.insert(*price, *qty);
            }
        }

        // Asks
        for (price, qty) in &delta.asks {
             if *qty <= 0 {
                 self.asks.remove(price);
             } else {
                 self.asks.insert(*price, *qty);
             }
        }

        self.last_update_id = delta.update_id;
        self.timestamp = delta.timestamp;
    }

    pub fn get_best_bid(&self) -> Option<(Decimal, i64)> {
        // BTreeMap is sorted ascending. Best bid is the highest price (last entry).
        self.bids.iter().next_back().map(|(p, q)| (*p, *q))
    }

    pub fn get_best_ask(&self) -> Option<(Decimal, i64)> {
        // Best ask is the lowest price (first entry).
        self.asks.iter().next().map(|(p, q)| (*p, *q))
    }

    pub fn get_mid_price(&self) -> Option<Decimal> {
        let bb = self.get_best_bid();
        let ba = self.get_best_ask();
        match (bb, ba) {
            (Some((b, _)), Some((a, _))) => Some((b + a) / rust_decimal::dec!(2.0)),
            _ => None,
        }
    }
    
    pub fn validate(&self) -> bool {
        // Check crossed book
        let bb = self.get_best_bid();
        let ba = self.get_best_ask();
        if let (Some((b, _)), Some((a, _))) = (bb, ba) {
             if b >= a {
                 return false;
             }
        }
        true
    }
}

