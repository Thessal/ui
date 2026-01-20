use didius_oms::oms::order_book::{OrderBook, OrderBookDelta};
use rust_decimal::Decimal;
use rust_decimal::dec;
use rand::seq::SliceRandom;
use rand::thread_rng;

#[test]
fn test_orderbook_sequential_add() {
    let mut book = OrderBook::new("TEST".to_string());
    
    // a) initial orderbook is empty. depth is 5. 
    // add 1 orders in every price level, one by one, until every price have 10 volumes.
    // Price levels: 100, 101, 102, 103, 104
    let prices = vec![dec!(100), dec!(101), dec!(102), dec!(103), dec!(104)];
    
    for _ in 0..10 {
        for &p in &prices {
            // Construct delta adding 1 volume
            
            // apply_delta logic in OrderBook: inserts or replaces?
            // "if *qty <= 0 { remove } else { insert }"
            // This means it's a SNAPSHOT of that level, not additive delta?
            // "apply_delta" usually implies update. 
            // implementation: self.bids.insert(key, *qty);
            // So it REPLACES the quantity at that level.
            // User requirement: "add 1 orders... until every price have 10 volumes".
            // If implementation replaces, we need to read current, add 1, and send new total.
            
            // Wait, if it's a "Delta" aimed at updating an OrderBook capable of maintaining state, 
            // usually "Delta" gives the NEW QUANTITY for that price level (Market By Price).
            // OR it gives the CHANGE (+1).
            // Looking at `OrderBook::apply_delta`: `self.bids.insert(*price, *qty)`.
            // It replaces the quantity. So it is "Absolute Quantity Update" model (common in crypto/Hantoo).
            
            // So to "add 1 order", we must know current qty.
            let current_qty = book.get_bids().get(&p).cloned().unwrap_or(0);
            let new_qty = current_qty + 1;
            
            let delta = OrderBookDelta {
                symbol: "TEST".to_string(),
                bids: vec![(p, new_qty)],
                asks: vec![],
                update_id: 1,
                timestamp: 100.0,
            };
            book.apply_delta(&delta);
        }
    }
    
    // Verify
    let bids = book.get_bids();
    for &p in &prices {
        assert_eq!(*bids.get(&p).unwrap(), 10);
    }
}

#[test]
fn test_orderbook_random_shuffle() {
    // b) randomly add and remove orders using list of OrderBookDelta. shuffle the list
    let mut book1 = OrderBook::new("TEST".to_string());
    let mut book2 = OrderBook::new("TEST".to_string());
    
    let mut deltas = Vec::new();
    
    // Generate valid sequence of updates? 
    // If updates contain timestamps, order matters. 
    // If we receive them out of order (shuffled), buffer/reorder might be needed?
    // BUT `apply_delta` implementation: `if delta.timestamp < self.timestamp { return; }`
    // This rejects older updates.
    // If we shuffle, we might drop updates if a newer one processes first.
    // The user asks: "shuffle the list, and make sure the result is the same."
    // This implies that either:
    // 1. We are testing buffering/reordering logic (Reconcile logic).
    // 2. Or the operations are commutative (they are not if they overwrite).
    // 3. Or we have distinct timestamps and the system should handle out-of-order.
    
    // If `OrderBook::apply_delta` rejects old timestamps, then processing `T=2` then `T=1` results in `T=1` being ignored.
    // Result is state at `T=2`.
    // Processing `T=1` then `T=2` results in state at `T=2`.
    // So final state SHOULD be same (the state of the latest timestamp).
    
    // Let's generate deltas with increasing timestamps.
    for i in 1..=20 {
        deltas.push(OrderBookDelta {
            symbol: "TEST".to_string(),
            bids: vec![(dec!(100), i)],
            asks: vec![],
            update_id: i,
            timestamp: i as f64,
        });
    }
    
    // Apply in order to book1
    for d in &deltas {
        book1.apply_delta(d);
    }
    
    // Shuffle and apply to book2
    let mut rng = thread_rng();
    let mut shuffled = deltas.clone();
    shuffled.shuffle(&mut rng);
    
    for d in &shuffled {
        book2.apply_delta(d);
    }
    
    // Verify equality
    // State should be matching the MAX timestamp update (qty = 20).
    assert_eq!(book1.get_bids().get(&dec!(100)), Some(&20));
    assert_eq!(book2.get_bids().get(&dec!(100)), Some(&20));
}

#[test]
fn test_orderbook_validate_fail() {
    let mut book = OrderBook::new("TEST".to_string());
    
    // Valid state: Bid 100, Ask 101
    book.apply_delta(&OrderBookDelta {
        symbol: "TEST".to_string(),
        bids: vec![(dec!(100), 10)],
        asks: vec![(dec!(101), 10)],
        update_id: 1,
        timestamp: 1.0,
    });
    assert!(book.validate());
    
    // Crossed state: Bid 102, Ask 101
    book.apply_delta(&OrderBookDelta {
        symbol: "TEST".to_string(),
        bids: vec![(dec!(102), 10)],
        asks: vec![],
        update_id: 2,
        timestamp: 2.0,
    });
    
    assert!(!book.validate());
}
