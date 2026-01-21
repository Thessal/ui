use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use crate::oms::order::{Order, OrderState, ExecutionStrategy, OrderSide};
use crate::oms::order_book::OrderBook;
use crate::oms::account::AccountState;
use crate::adapter::Adapter;
use crate::logger::Logger;
use crate::logger::message::Message;
use uuid::Uuid;
use chrono::Local;
use std::sync::mpsc::Receiver;
use crate::adapter::{IncomingMessage, Trade};

#[derive(Clone)]
pub struct OMSEngine {
    adapter: Arc<dyn Adapter>,
    order_books: Arc<Mutex<HashMap<String, OrderBook>>>,
    account: Arc<Mutex<AccountState>>,
    orders: Arc<Mutex<HashMap<String, Order>>>,
    is_running: Arc<Mutex<bool>>,
    margin_requirement: f64,

    active_strategies: Arc<Mutex<Vec<Box<dyn crate::strategy::base::Strategy + Send + Sync>>>>,
    logger: Arc<Mutex<Logger>>,
}

impl OMSEngine {
    pub fn new(adapter: Arc<dyn Adapter>, margin_requirement: f64, logger: Arc<Mutex<Logger>>) -> Self {
        OMSEngine {
            adapter,
            order_books: Arc::new(Mutex::new(HashMap::new())),
            account: Arc::new(Mutex::new(AccountState::new())),
            orders: Arc::new(Mutex::new(HashMap::new())),
            is_running: Arc::new(Mutex::new(false)),
            margin_requirement,
            active_strategies: Arc::new(Mutex::new(Vec::new())),
            logger,
        }
    }

    pub fn start(&self, _py: Python, account_id: Option<String>) -> PyResult<()> {
        let _running = {
            let mut r = self.is_running.lock().unwrap();
            if *r {
                return Ok(());
            }
            *r = true;
            true
        };
        
        self.adapter.connect().map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        
        if let Some(acc) = account_id {
            self.initialize_account(_py, acc)?;
        }

        
        // Start logger
        {
            let mut l = self.logger.lock().unwrap();
            l.start();
        }
        
        let is_running_clone = self.is_running.clone();
        
        thread::spawn(move || {
            loop {
                {
                    let r = is_running_clone.lock().unwrap();
                    if !*r {
                        break;
                    }
                }
                thread::sleep(Duration::from_secs(1));
            }
        });

        Ok(())
    }

    pub fn stop(&self, _py: Python) -> PyResult<()> {
        {
            let mut r = self.is_running.lock().unwrap();
            *r = false;
        }
        // Stop logger
        {
            let mut l = self.logger.lock().unwrap();
            l.stop();
        }

        self.adapter.disconnect().map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(())
    }

    pub fn initialize_symbol(&self, _py: Python, symbol: String) -> PyResult<()> {
        let snapshot = self.adapter.get_order_book_snapshot(&symbol)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        
        let mut books = self.order_books.lock().unwrap();
        books.insert(symbol.clone(), snapshot);
        Ok(())
    }
    
    pub fn initialize_account(&self, _py: Python, account_id: String) -> PyResult<()> {
        let snapshot = self.adapter.get_account_snapshot(&account_id)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        
        let mut acct = self.account.lock().unwrap();
        *acct = snapshot;
        Ok(())
    }

    pub fn send_order(&self, _py: Python, mut order: Order) -> PyResult<()> {
        if order.order_id.is_none() {
            order.order_id = Some(Uuid::new_v4().to_string());
        }
        
        // Strategy Handling
        match order.strategy {
            ExecutionStrategy::FOK => {
                let passed = {
                     let books = self.order_books.lock().unwrap();
                     if let Some(book) = books.get(&order.symbol) {
                         crate::strategy::fok::FOKStrategy::check(&order, book)
                     } else {
                         false // No book = assume fail?
                     }
                };
                if !passed {
                    println!("FOK Check Failed for Order {}", order.order_id.as_ref().unwrap_or(&"".to_string()));
                    // Rejection
                    {
                         let mut orders = self.orders.lock().unwrap();
                         if let Some(oid) = &order.order_id {
                             order.state = OrderState::REJECTED;
                             order.error_message = Some("FOK verification failed".into());
                             orders.insert(oid.clone(), order.clone());
                         }

                    }
                    
                    let msg = Message::new(
                        "ORDER_REJECTED".to_string(),
                        serde_json::json!({
                            "reason": "IOC No Liquidity",
                            "order_id": order.order_id
                        })
                    );
                    self.logger.lock().unwrap().log(msg);

                    return Ok(());
                }
            },
            ExecutionStrategy::IOC => {
                let fillable = {
                     let books = self.order_books.lock().unwrap();
                     if let Some(book) = books.get(&order.symbol) {
                         crate::strategy::ioc::IOCStrategy::calculate_fillable_qty(&order, book)
                     } else {
                         0
                     }
                };
                
                if fillable == 0 {
                    // Reject
                    {
                         let mut orders = self.orders.lock().unwrap();
                         if let Some(oid) = &order.order_id {
                             order.state = OrderState::REJECTED;
                             order.error_message = Some("IOC: No liquidity".into());
                             orders.insert(oid.clone(), order.clone());
                         }
                    }
                    
                    let msg = Message::new(
                        "ORDER_REJECTED".to_string(),
                        serde_json::json!({
                            "reason": "IOC No Liquidity",
                            "order_id": order.order_id
                        })
                    );
                    self.logger.lock().unwrap().log(msg);

                    return Ok(());
                }
                
                // Modify Quantity
                order.quantity = fillable;
            },
            ExecutionStrategy::STOP_LOSS | ExecutionStrategy::TAKE_PROFIT => {
                // Register Strategy
                let trigger_price = order.stop_price.unwrap_or(0.0);
                let strategy = crate::strategy::stop::StopStrategy::new(trigger_price, order.clone());
                
                {
                    let mut strats = self.active_strategies.lock().unwrap();
                    strats.push(Box::new(strategy));
                }
                
                // Store Order as PENDING/CREATED but DO NOT send to adapter yet.
                 {
                     let mut orders = self.orders.lock().unwrap();
                     if let Some(oid) = &order.order_id {
                         order.state = OrderState::CREATED; // Waiting for trigger
                         orders.insert(oid.clone(), order.clone());
                     }
                }
                return Ok(());
            },
            _ => {}
        }

        {
             let mut orders = self.orders.lock().unwrap();
             if let Some(oid) = &order.order_id {
                 order.state = OrderState::PENDING_NEW;
                 orders.insert(oid.clone(), order.clone());
             }
        }
        
        let success = self.adapter.place_order(&order)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        
        if !success {
             let mut orders = self.orders.lock().unwrap();
             if let Some(oid) = &order.order_id {
                 if let Some(o) = orders.get_mut(oid) {
                     o.update_state(OrderState::REJECTED, Some("Adapter Send Failed".into()));
                 }
             }
        }

        Ok(())
    }

    pub fn cancel_order(&self, _py: Python, order_id: String) -> PyResult<()> {
        let mut orders = self.orders.lock().unwrap();
        if let Some(order) = orders.get_mut(&order_id) {
            order.update_state(OrderState::PENDING_CANCEL, None);
        } else {
             return Err(pyo3::exceptions::PyValueError::new_err("Order not found"));
        }
        // Release lock before calling adapter to avoid potential deadlock? 
        // Adapter call might be slow.
        drop(orders);
        
        let success = self.adapter.cancel_order(&order_id)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
            
        let msg = Message::new(
            "ORDER_CANCEL_REQ".to_string(),
            serde_json::json!({
                "order_id": order_id,
                "success": success
            })
        );
        self.logger.lock().unwrap().log(msg);
            
        if !success {
             // Revert state logic if needed, but for now just returning failure is handled by caller catch?
             // Or update order verification?
        }
        
        Ok(())
    }
    
    // Pure Rust method for internal or test usage
    pub fn on_trade_update(&self, order_id: &str, fill_qty: i64, fill_price: f64) {
        let mut orders = self.orders.lock().unwrap();
        
        if let Some(order) = orders.get_mut(order_id) {
             let old_filled = order.filled_quantity;
             let new_filled = old_filled + fill_qty;
             let total_qty = order.quantity;
             
             let _is_fully_filled = new_filled >= total_qty;
             
             // Update Order
             order.filled_quantity = new_filled;
             // Update avg price (simplified)
             let old_val = old_filled as f64 * order.average_fill_price;
             let fill_val = fill_qty as f64 * fill_price;
             order.average_fill_price = (old_val + fill_val) / new_filled as f64;
             
             order.state = if new_filled >= total_qty { OrderState::FILLED } else { OrderState::PARTIALLY_FILLED };
             order.updated_at = Local::now().timestamp_millis() as f64 / 1000.0;
             
             // Update Account (Balance/Positions)
             {
                 let mut acct = self.account.lock().unwrap();
                 let symbol = order.symbol.clone();
                 let side = match order.side { OrderSide::BUY => "BUY", OrderSide::SELL => "SELL" };
                 
                 acct.on_execution(symbol, side.to_string(), fill_qty, fill_price, 0.0); // Assuming 0 fee for now
             }
        }
    }

    // Handle cancellation
    pub fn on_order_status_update(&self, order_id: &str, state: OrderState) {
        let mut orders = self.orders.lock().unwrap();
        if let Some(order) = orders.get_mut(order_id) {
             // if state == OrderState::CANCELED || state == OrderState::REJECTED { ... }
             order.update_state(state.clone(), None);
        }
        
        // Notify Strategies
        {
            let mut strats = self.active_strategies.lock().unwrap();
            let mut triggered = Vec::new();
             for strat in strats.iter_mut() {
                 if let Ok(Some(action)) = strat.on_order_status_update(order_id, state.clone()) {
                     triggered.push(action);
                 }
             }
             drop(strats);
             
             if !triggered.is_empty() {
                 Python::with_gil(|py| {
                     for o in triggered {
                         if o.state == OrderState::PENDING_CANCEL {
                             if let Some(oid) = o.order_id {
                                 let _ = self.cancel_order(py, oid);
                             }
                         } else {
                             let _ = self.send_order(py, o);
                         }
                     }
                 });
             }
        }
    }

    pub fn on_market_data(&self, _py: Python, _data: PyObject) -> PyResult<()> {
        // data processing (Dict -> Struct)
        Ok(())
    }
    
    pub fn on_account_update(&self, _py: Python, _data: PyObject) -> PyResult<()> {
        Ok(())
    }
    
    pub fn get_account(&self) -> AccountState {
        self.account.lock().unwrap().clone()
    }

    pub fn get_order_book(&self, symbol: &str) -> Option<OrderBook> {
        self.order_books.lock().unwrap().get(symbol).cloned()
    }

    pub fn on_order_book_information(&self, msg: IncomingMessage) -> PyResult<()> {
        let (symbol, delta_opt, snapshot_opt) = match msg {
            IncomingMessage::OrderBookDelta(d) => (d.symbol.clone(), Some(d), None),
            IncomingMessage::OrderBookSnapshot(s) => (s.symbol.clone(), None, Some(s)),
            _ => return Ok(()),
        };
        
        let mut books = self.order_books.lock().unwrap();
        // If book doesn't exist, create empty.
        let book = books.entry(symbol.clone()).or_insert_with(|| OrderBook::new(symbol.clone()));
        
        if let Some(delta) = delta_opt {
            book.apply_delta(&delta);
        } else if let Some(snapshot) = snapshot_opt {
            book.rebuild(snapshot.bids, snapshot.asks, snapshot.update_id, snapshot.timestamp);
        }
        
        if !book.validate() {
            // Reconcile if invalid
            // Drop lock to avoid deadlock during reconcile (which calls adapter)
            drop(books); 
            self.reconcile_orderbook(&symbol)?;
            return Ok(()); // Reconcile updates book
        }
        
        let mut triggered_orders = Vec::new();

        // Strategy Processing
        // We hold 'books' lock here.
        // We acquire 'active_strategies' lock.
        {
            let mut strats = self.active_strategies.lock().unwrap();
            
            let mut i = 0;
            while i < strats.len() {
                if let Ok(Some(action)) = strats[i].on_order_book_update(&book) {
                     triggered_orders.push(action);
                }
                i += 1;
            }
        } // Drop active_strategies
        
        drop(books); // Drop books

        if !triggered_orders.is_empty() {
             Python::with_gil(|py| {
                 for o in triggered_orders {
                     if o.state == OrderState::PENDING_CANCEL {
                         if let Some(oid) = o.order_id {
                             let _ = self.cancel_order(py, oid);
                         }
                     } else {
                         let _ = self.send_order(py, o);
                     }
                 }
             });
        }
        
        Ok(())
    }

    
    pub fn reconcile_orderbook(&self, symbol: &str) -> PyResult<()> {
        eprintln!("OrderBook for {} is being reconciled.", symbol);
        // Request full snapshot
        let snapshot = self.adapter.get_order_book_snapshot(symbol)
             .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
             
        let mut books = self.order_books.lock().unwrap();
        let book = books.entry(symbol.to_string()).or_insert_with(|| OrderBook::new(symbol.to_string()));
        
        // Update book with snapshot (Replace entirely)
        *book = snapshot;
        
        // Re-validate?
        if !book.validate() {
             eprintln!("OrderBook for {} still invalid after reconciliation!", symbol);
        }
        
        Ok(())
    }

    pub fn start_gateway_listener(&self, receiver: Receiver<IncomingMessage>) -> PyResult<()> {
        let engine = self.clone();
    
        thread::spawn(move || {
            for msg in receiver {
                // Log it (Lazy / Async)
                {
                     let msg_clone = msg.clone();
                     engine.logger.lock().unwrap().log_lazy("MARKET_DATA".to_string(), Box::new(move || {
                        match &msg_clone {
                            IncomingMessage::OrderBookDelta(d) => {
                                let (bp, bv): (Vec<_>, Vec<_>) = d.bids.iter().map(|(p, q)| (*p, *q)).unzip();
                                let (ap, av): (Vec<_>, Vec<_>) = d.asks.iter().map(|(p, q)| (*p, *q)).unzip();
                                
                                serde_json::json!({
                                    "type": "OrderBookDelta", 
                                    "symbol": d.symbol, 
                                    "update_id": d.update_id,
                                    "data": {
                                        "bp": bp,
                                        "bv": bv,
                                        "ap": ap,
                                        "av": av
                                    }
                                })
                            },
                            IncomingMessage::Trade(t) => serde_json::json!({"type": "Trade", "symbol": t.symbol, "price": t.price, "qty": t.quantity}),
                            IncomingMessage::Execution{order_id, fill_qty, ..} => serde_json::json!({"type": "Execution", "order_id": order_id, "qty": fill_qty}),
                            IncomingMessage::OrderBookSnapshot(s) => serde_json::json!({
                                "type": "OrderBookSnapshot", 
                                "symbol": s.symbol,
                                "bids": s.bids,
                                "asks": s.asks 
                            }),
                        }
                    }));
                }
                // Process it
                match msg {
                    IncomingMessage::OrderBookDelta(_) | IncomingMessage::OrderBookSnapshot(_) => {
                         let _ = engine.on_order_book_information(msg);
                    },
                    IncomingMessage::Trade(_trade) => {
                         // engine.on_market_trade(trade); // TODO implement
                    },
                    IncomingMessage::Execution{order_id, fill_qty, fill_price} => {
                         engine.on_trade_update(&order_id, fill_qty, fill_price);
                    },
                }
            }
        });
        Ok(())
    }
}
    

