use didius::oms::engine::OMSEngine;
use didius::oms::order_book::{OrderBook, OrderBookDelta};
use didius::adapter::Adapter;
use std::sync::{Arc, Mutex};
use rust_decimal::dec;
use rust_decimal::Decimal;

// Extended MockAdapter to control snapshot
struct ControllableMockAdapter {
    snapshot: Mutex<Option<OrderBook>>,
}
impl ControllableMockAdapter {
    fn new() -> Self {
        Self { snapshot: Mutex::new(None) }
    }
    fn set_snapshot(&self, book: OrderBook) {
        *self.snapshot.lock().unwrap() = Some(book);
    }
}
impl Adapter for ControllableMockAdapter {
    fn connect(&self) -> anyhow::Result<()> { Ok(()) }
    fn disconnect(&self) -> anyhow::Result<()> { Ok(()) }
    fn place_order(&self, _: &didius::oms::order::Order) -> anyhow::Result<bool> { Ok(true) }
    fn cancel_order(&self, _: &str) -> anyhow::Result<bool> { Ok(true) }
    fn get_order_book_snapshot(&self, symbol: &str) -> anyhow::Result<OrderBook> {
        if let Some(s) = self.snapshot.lock().unwrap().clone() {
            Ok(s)
        } else {
            Ok(OrderBook::new(symbol.to_string()))
        }
    }
    fn get_account_snapshot(&self, _: &str) -> anyhow::Result<didius::oms::account::AccountState> {
        Ok(didius::oms::account::AccountState::new())
    }
}

#[test]
fn test_reconcile_flow() {
    // d) assume that inconsitency is detected. Check reconcile function.
    let adapter = Arc::new(ControllableMockAdapter::new());
    let engine = OMSEngine::new(adapter.clone(), 1.0);
    
    let symbol = "TEST".to_string();
    
    // 1. Initial State: Valid
    engine.on_order_book_update(OrderBookDelta {
        symbol: symbol.clone(),
        bids: vec![(dec!(100), 10)],
        asks: vec![(dec!(101), 10)],
        update_id: 1,
        timestamp: 100.0,
    }).unwrap();
    
    let book = engine.get_order_book(&symbol).unwrap();
    assert!(book.validate());
    
    // 2. Setup Snapshot for Reconcile to pick up
    // Snapshot will have "Correct" state at T=110.
    let mut snapshot = OrderBook::new(symbol.clone());
    snapshot.rebuild(
        vec![(dec!(100), 20)], 
        vec![(dec!(102), 20)],
        2, 
        110.0
    );
    adapter.set_snapshot(snapshot);
    
    // 3. Send Invalid Delta (Crossed)
    // This should trigger reconcile (fetch snapshot).
    engine.on_order_book_update(OrderBookDelta {
        symbol: symbol.clone(),
        bids: vec![(dec!(105), 10)], // Crosses Ask 101
        asks: vec![],
        update_id: 3,
        timestamp: 105.0,
    }).unwrap();
    
    // 4. Verify Engine State matches Snapshot (T=110)
    // The engine should have discarded the invalid delta in favor of the snapshot.
    
    let book = engine.get_order_book(&symbol).unwrap();
    assert_eq!(book.timestamp, 110.0);
    assert_eq!(book.get_bids().get(&dec!(100)), Some(&20));
    assert_eq!(book.get_bids().get(&dec!(105)), None); 
}
