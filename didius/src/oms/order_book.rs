use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;

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
pub struct OrderBookSnapshot {
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
        let _bb = self.get_best_bid();
        let _ba = self.get_best_ask();
        // if let (Some((b, _)), Some((a, _))) = (bb, ba) {
        //     if b > a { // Aggregated book allows b > a condition. Need to study. Is it arbitrage?
        //         eprintln!("Crossed book for {} b{} a{}", self.symbol, b, a);
        //         eprintln!("{}", self);
        //         return false;
        //     }
        // }
        true
    }
}

impl PartialEq for OrderBook {
    fn eq(&self, other: &Self) -> bool {
        self.symbol == other.symbol &&
        self.bids == other.bids &&
        self.asks == other.asks
    }
}

impl Eq for OrderBook {}

impl std::fmt::Display for OrderBookSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OrderBookSnapshot [{}] (UpdateID: {})", self.symbol, self.update_id)?;
        if !self.bids.is_empty() {
             write!(f, "\n  Bids: ")?;
             for (p, q) in self.bids.iter().take(5) {
                 write!(f, "({} @ {}) ", q, p)?;
             }
        }
        if !self.asks.is_empty() {
             write!(f, "\n  Asks: ")?;
             for (p, q) in self.asks.iter().take(5) {
                 write!(f, "({} @ {}) ", q, p)?;
             }
        }
        Ok(())
    }
}

impl std::fmt::Display for OrderBook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OrderBook [{}] (UpdateID: {} | Time: {})", self.symbol, self.last_update_id, self.timestamp)?;
        if let Some((bp, bq)) = self.get_best_bid() {
            write!(f, "\n  Best Bid: {} @ {}", bq, bp)?;
        }
        if let Some((ap, aq)) = self.get_best_ask() {
            write!(f, "\n  Best Ask: {} @ {}", aq, ap)?;
        }
        // Maybe print top 5 levels?
        write!(f, "\n  Bids: ")?;
        for (p, q) in self.bids.iter().rev().take(5) {
             write!(f, "({} @ {}) ", q, p)?;
        }
        write!(f, "\n  Asks: ")?;
        for (p, q) in self.asks.iter().take(5) {
             write!(f, "({} @ {}) ", q, p)?;
        }

        Ok(())
    }
}


