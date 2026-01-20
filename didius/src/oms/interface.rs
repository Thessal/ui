use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::oms::engine::OMSEngine;
use crate::oms::order::Order;
use crate::adapter::mock::MockAdapter;
use crate::logger::config::{LoggerConfig, LogDestinationInfo};
use crate::logger::Logger;

#[pyclass]
pub struct Interface {
    engine: Arc<OMSEngine>,
}

#[pymethods]
impl Interface {
    #[new]
    #[pyo3(signature = (_adapter=None))]
    fn new(_adapter: Option<PyObject>) -> Self {
        // Default logger config provided by user
        let config = LoggerConfig {
            destination: LogDestinationInfo::LocalFile { path: "logs/log.jsonl".to_string() },
            flush_interval_seconds: 60, // Default
            batch_size: 100, // Default
        };
        let logger = Arc::new(Mutex::new(Logger::new(config)));
        
        Interface {
            engine: Arc::new(OMSEngine::new(Arc::new(MockAdapter::new()), 1.0, logger)),
        }
    }

    #[pyo3(signature = (account_id=None))]
    fn start(&self, py: Python, account_id: Option<String>) -> PyResult<()> {
        self.engine.start(py, account_id)
    }

    fn stop(&self, py: Python) -> PyResult<()> {
        self.engine.stop(py)
    }

    fn place_order(&self, py: Python, order: Order) -> PyResult<String> {
        // Order is still PyClass, so we can accept it directly.
        self.engine.send_order(py, order.clone())?;
        Ok(order.order_id.unwrap_or_default())
    }
    
    fn cancel_order(&self, py: Python, order_id: String) -> PyResult<()> {
        self.engine.cancel_order(py, order_id)
    }
    
    fn get_order_book(&self, py: Python, symbol: String) -> PyResult<PyObject> {
        if let Some(book) = self.engine.get_order_book(&symbol) {
            // Convert OrderBook to Dict
            // Simple serialization:
            let dict = PyDict::new(py);
            dict.set_item("symbol", book.symbol)?;
            dict.set_item("last_update_id", book.last_update_id)?;
            dict.set_item("timestamp", book.timestamp)?;
            // Bids/Asks
            let bids_dict = PyDict::new(py);
            for (price, qty) in &book.bids {
                bids_dict.set_item(price.to_string(), qty)?;
            }
            dict.set_item("bids", bids_dict)?;
            
            let asks_dict = PyDict::new(py);
            for (price, qty) in &book.asks {
                asks_dict.set_item(price.to_string(), qty)?;
            }
            dict.set_item("asks", asks_dict)?;
            
            Ok(dict.into())
        } else {
            Ok(py.None())
        }
    }
    
    fn get_account(&self, py: Python) -> PyResult<PyObject> {
        let acc = self.engine.get_account();
        let dict = PyDict::new(py);
        dict.set_item("balance", acc.balance)?;
        dict.set_item("locked", acc.locked)?;
        
        let positions_dict = PyDict::new(py);
        for (sym, pos) in acc.positions {
            let p_dict = PyDict::new(py);
            p_dict.set_item("symbol", pos.symbol.clone())?;
            p_dict.set_item("quantity", pos.quantity)?;
            p_dict.set_item("average_price", pos.average_price)?;
            p_dict.set_item("current_price", pos.current_price)?;
            p_dict.set_item("unrealized_pnl", pos.unrealized_pnl())?;
            
            positions_dict.set_item(sym, p_dict)?;
        }
        dict.set_item("positions", positions_dict)?;
        
        Ok(dict.into())
    }
    
    fn init_symbol(&self, py: Python, symbol: String) -> PyResult<()> {
        self.engine.initialize_symbol(py, symbol)
    }

    // Callback wiring helpers if needed?
    // User might want to pass this Interface instance to Adapter callbacks?
    // For now, assume Adapter calls methods on Interface?
    // Engine logic is hidden.
    // We can expose `on_market_data` on Interface if Adapter calls it.
    
    fn on_market_data(&self, py: Python, data: PyObject) -> PyResult<()> {
        self.engine.on_market_data(py, data)
    }
}
