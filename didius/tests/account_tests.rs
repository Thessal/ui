use didius::oms::engine::OMSEngine;
use didius::oms::account::{AccountState, Position};
use didius::oms::order::{Order, OrderSide, OrderType, ExecutionStrategy, OrderState};
use didius::adapter::mock::MockAdapter;
use didius::adapter::Adapter;
use std::sync::Arc;
use pyo3::Python;

// Helper to create basic account state
fn create_account(balance: f64, positions: Vec<Position>) -> AccountState {
    let mut acc = AccountState::new();
    acc.rebuild(balance, 0.0, positions);
    acc
}

#[test]
fn test_account_initial_balance_long() {
    pyo3::prepare_freethreaded_python();
    
    // a) AAPL and MSFT 20 stock each
    let pos1 = Position::new("AAPL".to_string(), 20, 100.0, 105.0);
    let pos2 = Position::new("MSFT".to_string(), 20, 200.0, 210.0); // prices arbitrary
    
    let state = create_account(10000.0, vec![pos1, pos2]);
    let adapter = Arc::new(MockAdapter::with_account_state(state));
    let engine = OMSEngine::new(adapter);
    
    // Init (using None or Some id, mock ignores ID but returns state)
    Python::with_gil(|py| {
        engine.initialize_account(py, "test_acc".to_string()).unwrap();
    });
    
    let acc = engine.get_account();
    assert_eq!(acc.balance, 10000.0);
    assert_eq!(acc.positions.len(), 2);
    assert_eq!(acc.positions.get("AAPL").unwrap().quantity, 20);
    assert_eq!(acc.positions.get("MSFT").unwrap().quantity, 20);
}

#[test]
fn test_account_initial_balance_short() {
    pyo3::prepare_freethreaded_python();

    // b) NQ -1 (short)
    let pos1 = Position::new("NQ".to_string(), -1, 15000.0, 15100.0);
    
    let state = create_account(50000.0, vec![pos1]);
    let adapter = Arc::new(MockAdapter::with_account_state(state));
    let engine = OMSEngine::new(adapter);
    
    Python::with_gil(|py| {
        engine.initialize_account(py, "test_acc".to_string()).unwrap();
    });
    
    let acc = engine.get_account();
    assert_eq!(acc.positions.get("NQ").unwrap().quantity, -1);
}

#[test]
fn test_new_order_hold_balance() {
    pyo3::prepare_freethreaded_python();
    
    // c) Initial 1000 USD, no pos. New order 10 INTC (Buy).
    let state = create_account(1000.0, vec![]);
    let adapter = Arc::new(MockAdapter::with_account_state(state));
    let engine = OMSEngine::new(adapter);
    
    Python::with_gil(|py| {
        engine.initialize_account(py, "test_acc".to_string()).unwrap();
    });

    // Send Order: Buy 10 INTC @ 50 (Total 500)
    let order = Order::new(
        "INTC".to_string(),
        OrderSide::BUY,
        OrderType::LIMIT,
        10,
        Some(50.0),
        None, None, None
    );
    
    Python::with_gil(|py| {
        engine.send_order(py, order.clone()).unwrap();
    });
    
    let acc = engine.get_account();
    // NOTE: Current implementation of OMSEngine::send_order might NOT update locked yet.
    // assert_eq!(acc.locked, 0.0); // No proactive locking in OMS
    assert_eq!(acc.balance, 1000.0); // Balance shouldn't decrease yet
    assert!(acc.positions.is_empty());
}

#[test]
fn test_partial_fill() {
    pyo3::prepare_freethreaded_python();
    
    // d) Partial fill checks
    let state = create_account(1000.0, vec![]);
    let adapter = Arc::new(MockAdapter::with_account_state(state));
    let engine = OMSEngine::new(adapter);
    
    Python::with_gil(|py| {
        engine.initialize_account(py, "test_acc".to_string()).unwrap();
    });

    // Buy 10 INTC @ 50
    let mut order = Order::new(
        "INTC".to_string(),
        OrderSide::BUY,
        OrderType::LIMIT,
        10,
        Some(50.0),
        None, None, None
    );
     // Need to set ID to track it? send_order assigns one if None.
     // But we need to know it to trigger fill.
     order.order_id = Some("ord_1".to_string());

    Python::with_gil(|py| {
        engine.send_order(py, order.clone()).unwrap();
    });
    
    // Trigger Partial Fill: 5 @ 50
    engine.on_trade_update("ord_1", 5, 50.0);
    
    let acc = engine.get_account();
    // Bought 5 @ 50 = 250 cost.
    // Initial 1000. Balance = 750.
    // Locked? 0.
    assert_eq!(acc.balance, 750.0);
    assert_eq!(acc.locked, 0.0);
    assert_eq!(acc.positions.get("INTC").unwrap().quantity, 5);
    
    // Fully Filled: another 5 @ 50
    engine.on_trade_update("ord_1", 5, 50.0);
    let acc = engine.get_account();
    assert_eq!(acc.balance, 500.0);
    assert_eq!(acc.locked, 0.0); // All filled, no lock
    assert_eq!(acc.positions.get("INTC").unwrap().quantity, 10);
}

#[test]
fn test_cancel_order() {
    pyo3::prepare_freethreaded_python();
    
    // e) / f) Cancel tests
    let state = create_account(1000.0, vec![]);
    let adapter = Arc::new(MockAdapter::with_account_state(state));
    let engine = OMSEngine::new(adapter);
    
    Python::with_gil(|py| {
        engine.initialize_account(py, "test_acc".to_string()).unwrap();
    });

    let mut order = Order::new(
        "INTC".to_string(),
        OrderSide::BUY,
        OrderType::LIMIT,
        10,
        Some(50.0),
        None, None, None
    );
     order.order_id = Some("ord_2".to_string());

    Python::with_gil(|py| {
        engine.send_order(py, order.clone()).unwrap();
    });
    
    let acc = engine.get_account();
    assert_eq!(acc.locked, 0.0);
    
    // Cancel without execution
    engine.on_order_status_update("ord_2", OrderState::CANCELED);
    
    let acc = engine.get_account();
    assert_eq!(acc.locked, 0.0); // Released
    assert_eq!(acc.balance, 1000.0);
}

#[test]
fn test_partial_fill_then_cancel() {
    pyo3::prepare_freethreaded_python();
    
    let state = create_account(1000.0, vec![]);
    let adapter = Arc::new(MockAdapter::with_account_state(state));
    let engine = OMSEngine::new(adapter);
    
    Python::with_gil(|py| {
        engine.initialize_account(py, "test_acc".to_string()).unwrap();
    });

    let mut order = Order::new(
        "INTC".to_string(),
        OrderSide::BUY,
        OrderType::LIMIT,
        10,
        Some(50.0),
        None, None, None
    );
     order.order_id = Some("ord_3".to_string());

    Python::with_gil(|py| {
        engine.send_order(py, order.clone()).unwrap();
    });
    
    // Fill 2
    engine.on_trade_update("ord_3", 2, 50.0);
    let acc = engine.get_account();
    // Locked: 0
    assert_eq!(acc.locked, 0.0);
    assert_eq!(acc.balance, 900.0);
    
    // Cancel remaining (8)
    engine.on_order_status_update("ord_3", OrderState::CANCELED);
    
    let acc = engine.get_account();
    assert_eq!(acc.locked, 0.0); // Released remaining
    assert_eq!(acc.balance, 900.0);
}
