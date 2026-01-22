use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use crate::oms::order::{Order, OrderState, ExecutionStrategy, OrderSide, OrderType};
use crate::oms::order_book::OrderBook;
use crate::oms::account::AccountState;
use crate::adapter::Adapter;
use crate::logger::Logger;
use crate::logger::message::Message;
use uuid::Uuid;
use chrono::Local;
use std::sync::mpsc::Receiver;
use crate::adapter::{IncomingMessage, Trade};
use rust_decimal::Decimal;
use rust_decimal::prelude::{FromPrimitive, FromStr};
use crate::strategy::base::StrategyAction;
use anyhow::anyhow;

#[derive(Clone)]
pub struct OMSEngine {
    adapter: Arc<dyn Adapter>,
    order_books: Arc<Mutex<HashMap<String, OrderBook>>>,
    account: Arc<Mutex<AccountState>>,
    orders: Arc<Mutex<HashMap<String, Order>>>,
    is_running: Arc<Mutex<bool>>,
    margin_requirement: Decimal,

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
            margin_requirement: Decimal::from_f64(margin_requirement).unwrap_or(Decimal::ONE),
            active_strategies: Arc::new(Mutex::new(Vec::new())),
            logger,
        }
    }

    pub fn start(&self, _py: Python, account_id: Option<String>) -> PyResult<()> {
        self.start_internal(account_id).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    pub fn start_internal(&self, account_id: Option<String>) -> anyhow::Result<()> {
        let _running = {
            let mut r = self.is_running.lock().unwrap();
            if *r {
                return Ok(());
            }
            *r = true;
            true
        };
        
        self.adapter.connect().map_err(|e| anyhow::anyhow!(e.to_string()))?;
        
        if let Some(acc) = account_id {
            // self.initialize_account(_py, acc)?; // initialize_account also takes Py
            // We need pure rust initialize_account too.
            self.initialize_account_internal(acc).map_err(|e| anyhow::anyhow!(e.to_string()))?;
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
        self.stop_internal().map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    pub fn stop_internal(&self) -> anyhow::Result<()> {
        {
            let mut r = self.is_running.lock().unwrap();
            *r = false;
        }
        // Stop logger
        {
            let mut l = self.logger.lock().unwrap();
            l.stop();
        }

        self.adapter.disconnect().map_err(|e| anyhow::anyhow!(e.to_string()))?;
        Ok(())
    }

    pub fn initialize_symbol(&self, _py: Python, symbol: String) -> PyResult<()> {
        self.initialize_symbol_internal(symbol).map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    pub fn initialize_symbol_internal(&self, symbol: String) -> anyhow::Result<()> {
        let snapshot = self.adapter.get_order_book_snapshot(&symbol)?;
        let mut books = self.order_books.lock().unwrap();
        books.insert(symbol.clone(), snapshot);
        Ok(())
    }
    
    pub fn initialize_account(&self, _py: Python, account_id: String) -> PyResult<()> {
        self.initialize_account_internal(account_id).map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    pub fn initialize_account_internal(&self, account_id: String) -> anyhow::Result<()> {
        let snapshot = self.adapter.get_account_snapshot(&account_id)?;
        let mut acct = self.account.lock().unwrap();
        *acct = snapshot;
        Ok(())
    }

    pub fn send_order(&self, py: Python, order: Order) -> PyResult<String> {
        self.send_order_internal(order).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    pub fn send_order_internal(&self, mut order: Order) -> anyhow::Result<String> {
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
                    let oid = order.order_id.clone().unwrap_or_default();
                    // Rejection
                    {
                         let mut orders = self.orders.lock().unwrap();
                         if let Some(oid_ref) = &order.order_id {
                             order.state = OrderState::REJECTED;
                             order.error_message = Some("FOK verification failed".into());
                             orders.insert(oid_ref.clone(), order.clone());
                         }

                    }
                    
                    let msg = Message::new(
                        "ORDER_REJECTED".to_string(),
                        serde_json::json!({
                            "reason": "IOC No Liquidity",
                            "order_id": oid
                        })
                    );
                    self.logger.lock().unwrap().log(msg);

                    return Ok(oid);
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
                    let oid = order.order_id.clone().unwrap_or_default();
                    // Reject
                    {
                         let mut orders = self.orders.lock().unwrap();
                         if let Some(oid_ref) = &order.order_id {
                             order.state = OrderState::REJECTED;
                             order.error_message = Some("IOC: No liquidity".into());
                             orders.insert(oid_ref.clone(), order.clone());
                         }
                    }
                    
                    let msg = Message::new(
                        "ORDER_REJECTED".to_string(),
                        serde_json::json!({
                            "reason": "IOC No Liquidity",
                            "order_id": oid
                        })
                    );
                    self.logger.lock().unwrap().log(msg);

                    return Ok(oid);
                }
                
                // Modify Quantity
                order.quantity = fillable;
            },
            ExecutionStrategy::CHAIN => {
                order.state = OrderState::CREATED;
                
                // Parse Strategy Params
                if let Some(price_str) = order.strategy_params.get("trigger_price") {
                    if let Ok(trigger_price) = Decimal::from_str(price_str) {
                         let side_str = order.strategy_params.get("trigger_side").map(|s| s.as_str()).unwrap_or("BUY");
                         let trigger_side = match side_str {
                             "SELL" => OrderSide::SELL,
                             _ => OrderSide::BUY,
                         };
                         let ts_str = order.strategy_params.get("trigger_timestamp").map(|s| s.as_str()).unwrap_or("0");
                         let trigger_timestamp = ts_str.parse::<f64>().unwrap_or(0.0);
                         
                         // Chained Order Params
                         let chained_symbol = order.strategy_params.get("chained_symbol").unwrap_or(&order.symbol).clone();
                         let chained_side_str = order.strategy_params.get("chained_side").map(|s| s.as_str()).unwrap_or("BUY");
                         let chained_side = match chained_side_str {
                             "SELL" => OrderSide::SELL,
                             _ => OrderSide::BUY,
                         };
                         let chained_qty_str = order.strategy_params.get("chained_quantity").map(|s| s.as_str()).unwrap_or("0");
                         let chained_qty = chained_qty_str.parse::<i64>().unwrap_or(0);
                         let chained_price = order.strategy_params.get("chained_price").cloned();
                         
                         let chained_order = Order::new(
                             chained_symbol,
                             chained_side,
                             OrderType::LIMIT,
                             chained_qty,
                             chained_price,
                             Some(ExecutionStrategy::NONE),
                             None,
                             None
                         );
                         
                         let strat = crate::strategy::chain::ChainStrategy::new(
                             order.order_id.clone().unwrap(),
                             trigger_side,
                             trigger_price,
                             trigger_timestamp,
                             chained_order
                         );
                         
                         {
                             let mut strats = self.active_strategies.lock().unwrap();
                             strats.push(Box::new(strat));
                         }
                    } else {
                        // Error parsing trigger price
                        println!("Failed to parse trigger price for Chain Order");
                    }
                } else {
                     println!("Missing trigger params for Chain Order");
                }

                let mut orders = self.orders.lock().unwrap();
                let oid = order.order_id.clone().unwrap_or_default();
                orders.insert(oid.clone(), order.clone());
            }
            ExecutionStrategy::STOP_LOSS | ExecutionStrategy::TAKE_PROFIT => {
                // Register Strategy
                // Strategy needs to handle Decimal
                // let trigger_price = order.stop_price.unwrap_or(0.0);
                // let strategy = crate::strategy::stop::StopStrategy::new(trigger_price, order.clone());
                
                // {
                //     let mut strats = self.active_strategies.lock().unwrap();
                //     strats.push(Box::new(strategy));
                // }
                
                // // Store Order as PENDING/CREATED but DO NOT send to adapter yet.
                //  {
                //      let mut orders = self.orders.lock().unwrap();
                //      if let Some(oid) = &order.order_id {
                //          order.state = OrderState::CREATED; // Waiting for trigger
                //          orders.insert(oid.clone(), order.clone());
                //      }
                // }
                // return Ok(());
                // Temporarily disable strategy for Decimal migration until Strategy Traits are updated.
                println!("Strategy disabled during migration");
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
        
        let success = self.adapter.place_order(&order)?;
        
        if !success {
             let mut orders = self.orders.lock().unwrap();
             if let Some(oid) = &order.order_id {
                 if let Some(o) = orders.get_mut(oid) {
                     o.update_state(OrderState::REJECTED, Some("Adapter Send Failed".into()));
                 }
             }
        }

        Ok(order.order_id.unwrap_or_default())
    }

    pub fn cancel_order(&self, _py: Python, order_id: String) -> PyResult<()> {
        self.cancel_order_internal(order_id).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    pub fn cancel_order_internal(&self, order_id: String) -> anyhow::Result<()> {
        let mut orders = self.orders.lock().unwrap();
        if let Some(order) = orders.get_mut(&order_id) {
            order.update_state(OrderState::PENDING_CANCEL, None);
        } else {
             return Err(anyhow::anyhow!("Order not found"));
        }
        // Release lock before calling adapter to avoid potential deadlock? 
        // Adapter call might be slow.
        drop(orders);
        
        let success = self.adapter.cancel_order(&order_id)?;
            
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
    pub fn on_trade_update(&self, order_id: &str, fill_qty: i64, fill_price: Decimal) {
        let mut orders = self.orders.lock().unwrap();
        
        if let Some(order) = orders.get_mut(order_id) {
             let old_filled = order.filled_quantity;
             let new_filled = old_filled + fill_qty;
             let total_qty = order.quantity;
             
             let _is_fully_filled = new_filled >= total_qty;
             
             // Update Order
             order.filled_quantity = new_filled;
             // Update avg price (simplified)
             let old_qty_dec = Decimal::from_i64(old_filled).unwrap_or_default();
             let fill_qty_dec = Decimal::from_i64(fill_qty).unwrap_or_default();
             let new_qty_dec = Decimal::from_i64(new_filled).unwrap_or_default();
             
             if new_qty_dec > Decimal::ZERO {
                 let old_val = old_qty_dec * order.average_fill_price;
                 let fill_val = fill_qty_dec * fill_price;
                 order.average_fill_price = (old_val + fill_val) / new_qty_dec;
             }
             
             order.state = if new_filled >= total_qty { OrderState::FILLED } else { OrderState::PARTIALLY_FILLED };
             order.updated_at = Local::now().timestamp_millis() as f64 / 1000.0;
             
             // Update Account (Balance/Positions)
             {
                 let mut acct = self.account.lock().unwrap();
                 let symbol = order.symbol.clone();
                 let side = match order.side { OrderSide::BUY => "BUY", OrderSide::SELL => "SELL" };
                 
                 acct.on_execution(symbol, side.to_string(), fill_qty, fill_price, Decimal::ZERO); // Assuming 0 fee for now
             }
        }
    }

    // Handle cancellation
    pub fn on_order_status_update(&self, order_id: &str, state: OrderState, msg: Option<String>) {
        let mut orders = self.orders.lock().unwrap();
        if let Some(order) = orders.get_mut(order_id) {
             order.update_state(state.clone(), msg);
        }
        
        // Notify Strategies (TODO: Move to async execution to avoid blocking)
        {
            let mut strats = self.active_strategies.lock().unwrap();
            let mut actions = Vec::new();
             for strat in strats.iter_mut() {
                 if let Ok(action) = strat.on_order_status_update(order_id, state.clone()) {
                     if !matches!(action, StrategyAction::None) {
                         actions.push(action);
                     }
                 }
             }
             drop(strats);
             
             for action in actions { // Someday we could do internal matching of opposite actions (e.g. buy and sell at same price)
                 match action {
                     StrategyAction::PlaceOrder(o) => {
                         let _ = self.send_order_internal(o);
                     },
                     StrategyAction::CancelOrder(oid) => {
                         let _ = self.cancel_order_internal(oid);
                     },
                     StrategyAction::None => {}
                 }
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
    
    pub fn get_orders(&self) -> HashMap<String, Order> {
        self.orders.lock().unwrap().clone()
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
        // Strategy Execution (TODO: Move to async execution to avoid blocking market data processing)
        {
            let mut strats = self.active_strategies.lock().unwrap();
            let mut actions = Vec::new();
            
            for strat in strats.iter_mut() {
                if let Ok(action) = strat.on_order_book_update(&book) {
                    if !matches!(action, StrategyAction::None) {
                        actions.push(action);
                    }
                }
            }
            drop(strats); // Release lock before processing actions (which might lock orders/adapter)
            // TODO: Internal netting 
            for action in actions {
                match action {
                    StrategyAction::PlaceOrder(o) => {
                         let _ = self.send_order_internal(o);
                    },
                    StrategyAction::CancelOrder(oid) => {
                         let _ = self.cancel_order_internal(oid);
                    },
                    StrategyAction::None => {}
                }
            }
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
                                let (bp, bv): (Vec<_>, Vec<_>) = d.bids.iter().map(|(p, q)| (p.to_string(), *q)).unzip();
                                let (ap, av): (Vec<_>, Vec<_>) = d.asks.iter().map(|(p, q)| (p.to_string(), *q)).unzip();
                                
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
                            IncomingMessage::Trade(t) => serde_json::json!({"type": "Trade", "symbol": t.symbol, "price": t.price.to_string(), "qty": t.quantity}),
                            IncomingMessage::Execution{order_id, fill_qty, ..} => serde_json::json!({"type": "Execution", "order_id": order_id, "qty": fill_qty}),
                            IncomingMessage::OrderBookSnapshot(s) => serde_json::json!({
                                "type": "OrderBookSnapshot", 
                                "symbol": s.symbol,
                                "bids": s.bids,
                                "asks": s.asks 
                            }),
                            IncomingMessage::OrderUpdate{order_id, state, ..} => serde_json::json!({"type": "OrderUpdate", "order_id": order_id, "state": format!("{:?}", state)}),
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
                    IncomingMessage::OrderUpdate{order_id, state, msg, ..} => {
                        engine.on_order_status_update(&order_id, state, msg);
                    }
                }
            }
        });
        Ok(())
    }
}

    

