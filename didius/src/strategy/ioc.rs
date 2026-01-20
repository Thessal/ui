use crate::oms::order::{Order, OrderSide};
use crate::oms::order_book::OrderBook;
use rust_decimal::prelude::*;

pub struct IOCStrategy;

impl IOCStrategy {
    // Returns the quantity that can be filled immediately.
    pub fn calculate_fillable_qty(order: &Order, book: &OrderBook) -> i64 {
        let limit_price = match order.price {
             Some(p) => Decimal::from_f64(p).unwrap_or_default(),
             None => return order.quantity, // Market IOC takes all available?
        };
        
        let mut filled = 0;
        let needed = order.quantity;

        if order.side == OrderSide::BUY {
            for (price, qty) in book.asks.iter() {
                if *price <= limit_price {
                    let take = std::cmp::min(needed - filled, *qty);
                    filled += take;
                    if filled >= needed {
                        break;
                    }
                } else {
                    break;
                }
            }
        } else {
            // Bids: Iterate reverse (high to low)
            for (price, qty) in book.bids.iter().rev() {
                if *price >= limit_price {
                    let take = std::cmp::min(needed - filled, *qty);
                    filled += take;
                    if filled >= needed {
                         break;
                    }
                } else {
                    break;
                }
            }
        }
        
        filled
    }
}
