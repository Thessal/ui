use crate::oms::order::{Order, OrderSide};
use crate::oms::order_book::OrderBook;
use rust_decimal::prelude::*;

pub struct FOKStrategy;

impl FOKStrategy {
    pub fn new() -> Self {
        FOKStrategy
    }

    // Returns true if the order can be fully filled immediately.
    pub fn check(order: &Order, book: &OrderBook) -> bool {
        let quantity = order.quantity;
        let limit_price = match order.price {
            Some(p) => p,
            None => return true, // Market order FOK? Assume fillable if enough liquidity?
        };

        if order.side == OrderSide::BUY {
            // Check Asks
            let mut available = 0;
            for (price, qty) in book.asks.iter() {
                if *price <= limit_price {
                    available += qty;
                    if available >= quantity {
                        return true;
                    }
                } else {
                    // Assuming sorted asks (ascending), if price > limit, we stop.
                    break;
                }
            }
        } else {
            // Check Bids (descending keys in BTreeMap?)
            // BTreeMap iterates keys in ascending order.
            // For bids, we want highest price first.
            // So we need to iterate in reverse.
            let mut available = 0;
            for (price, qty) in book.bids.iter().rev() {
                if *price >= limit_price {
                    available += qty;
                    if available >= quantity {
                        return true;
                    }
                } else {
                    break;
                }
            }
        }

        false
    }
}
