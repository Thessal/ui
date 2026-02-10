use pyo3::prelude::*;
use crate::state::State;
use crate::adapter::{Adapter, IncomingMessage};
use crate::message::Message;
use crate::logger::Logger;
use crate::logger::config::{LoggerConfig, LogDestinationInfo};
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::time::Duration;
use crate::oms::order::Order;

#[pyclass]
pub struct Client {
    adapter: Arc<dyn Adapter>,
    state: Arc<Mutex<State>>,
    receiver: Arc<Mutex<mpsc::Receiver<IncomingMessage>>>, 
    logger: Arc<Mutex<Logger>>,
}

#[pymethods]
impl Client {
    #[new]
    #[pyo3(signature = (venue, config_path=None, s3_bucket=None, s3_region=None, s3_prefix=None))]
    fn new(venue: String, config_path: Option<String>, s3_bucket: Option<String>, s3_region: Option<String>, s3_prefix: Option<String>) -> PyResult<Self> {
        let adapter: Arc<dyn Adapter> = match venue.as_str() {
            "hantoo" => {
                let config = config_path.ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Config path required for Hantoo"))?;
                let a = crate::adapter::hantoo::HantooAdapter::new(&config)
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
                Arc::new(a)
            },
            "hantoo_night" => {
                let config = config_path.ok_or_else(|| pyo3::exceptions::PyValueError::new_err("Config path required for Hantoo Night"))?;
                let a = crate::adapter::hantoo_ngt_futopt::HantooNightAdapter::new(&config)
                     .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
                Arc::new(a)
            },
            "mock" => {
                Arc::new(crate::adapter::mock::MockAdapter::new())
            },
            _ => return Err(pyo3::exceptions::PyValueError::new_err(format!("Unknown venue: {}", venue))),
        };
        
        let (sender, receiver) = mpsc::channel();
        
        // Initialize monitor on adapter
        adapter.set_monitor(sender.clone());

        // Initialize Logger
        let destination = if let (Some(bucket), Some(region)) = (s3_bucket, s3_region) {
            LogDestinationInfo::AmazonS3 { 
                bucket, 
                key_prefix: s3_prefix.unwrap_or_else(|| "logs".to_string()), 
                region 
            }
        } else {
             LogDestinationInfo::Console 
        };

        let config = LoggerConfig {
            destination,
            flush_interval_seconds: 60,
            batch_size: 8192,
        };
        let logger = Arc::new(Mutex::new(Logger::new(config)));
        logger.lock().unwrap().start();

        Ok(Client {
            adapter,
            state: Arc::new(Mutex::new(State::new())),
            receiver: Arc::new(Mutex::new(receiver)),
            logger,
        })
    }

    fn connect(&self) -> PyResult<()> {
        self.adapter.connect().map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
    
    fn disconnect(&self) -> PyResult<()> {
        self.adapter.disconnect().map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn place_order(&self, order: &Order) -> PyResult<bool> {
        self.adapter.place_order(order).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
    
    fn cancel_order(&self, order_id: &str) -> PyResult<bool> {
        self.adapter.cancel_order(order_id).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn update_order(&self, order_id: &str, price: Option<String>, qty: Option<i64>) -> PyResult<bool> {
        let price_dec = if let Some(p) = price {
            Some(crate::utils::parse_decimal(&p).map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?)
        } else {
            None
        };
        
        self.adapter.modify_order(order_id, price_dec, qty).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
    
    fn subscribe(&self, symbols: Vec<String>) -> PyResult<()> {
        self.adapter.subscribe(&symbols).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn fetch_message(&self, timeout_sec: f64) -> PyResult<Option<String>> {
        let rx = self.receiver.lock().unwrap();
        let timeout = Duration::from_secs_f64(timeout_sec);
        
        match rx.recv_timeout(timeout) {
            Ok(msg) => {
                // Apply to State
                {
                    let mut state = self.state.lock().unwrap();
                    state.apply(&msg);
                }
                
                // Return as JSON
                // IncomingMessage is alias to Message, so it implements Serialize
                let json = serde_json::to_string(&msg).map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                Ok(Some(json))
            },
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(pyo3::exceptions::PyRuntimeError::new_err("Channel disconnected")),
        }
    }
    
    /// Get a JSON snapshot of the account
    fn get_account_state(&self, account_id: &str) -> PyResult<Option<String>> {
        let state = self.state.lock().unwrap();
        if let Some(acct) = state.accounts.get(account_id) {
            let json = serde_json::to_string(acct).map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
            Ok(Some(json))
        } else {
            Ok(None)
        }
    }
    
    /// Get a JSON snapshot of the order book
    fn get_order_book(&self, symbol: &str) -> PyResult<Option<String>> {
        let state = self.state.lock().unwrap();
        if let Some(ob) = state.order_books.get(symbol) {
             let json = serde_json::to_string(ob).map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
             Ok(Some(json))
        } else {
            Ok(None)
        }
    }
}
