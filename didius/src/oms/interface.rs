use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use crate::oms::engine::OMSEngine;
use crate::oms::order::Order;
use crate::adapter::mock::MockAdapter;
use crate::adapter::hantoo::HantooAdapter;
use crate::logger::config::{LoggerConfig, LogDestinationInfo};
use crate::logger::Logger;
use crate::adapter::Adapter;

#[pyclass(name = "HantooAdapter")]
pub struct PyHantooAdapter {
    pub(crate) adapter: Arc<HantooAdapter>,
}

#[pymethods]
impl PyHantooAdapter {
    #[new]
    fn new(config_path: String) -> PyResult<Self> {
        let adapter = HantooAdapter::new(&config_path)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(PyHantooAdapter { adapter: Arc::new(adapter) })
    }

    fn subscribe_market(&self, symbols: Vec<String>) -> PyResult<()> {
        self.adapter.subscribe_market(&symbols)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(())
    }

    fn connect(&self) -> PyResult<()> {
        self.adapter.connect()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(())
    }
    
    fn set_debug_mode(&self, enabled: bool) {
        self.adapter.set_debug_mode(enabled);
    }
}

#[pyclass(name = "OMSEngine")]
pub struct Interface {
    engine: Arc<OMSEngine>,
}

#[pymethods]
impl Interface {
    #[new]
    #[pyo3(signature = (adapter=None, s3_bucket=None, s3_region=None, s3_prefix=None))]
    fn new(adapter: Option<&PyHantooAdapter>, s3_bucket: Option<String>, s3_region: Option<String>, s3_prefix: Option<String>) -> Self {
        
        let destination = if let (Some(bucket), Some(region)) = (s3_bucket, s3_region) {
            LogDestinationInfo::AmazonS3 { 
                bucket, 
                key_prefix: s3_prefix.unwrap_or_else(|| "logs".to_string()), 
                region 
            }
        } else {
             LogDestinationInfo::Console // Default to console if no S3 provided, or local file? Example used Console.
        };

        let config = LoggerConfig {
            destination,
            flush_interval_seconds: 60,
            batch_size: 1024,
        };
        let logger = Arc::new(Mutex::new(Logger::new(config)));
        // Start Logger immediately? The Rust example does.
        logger.lock().unwrap().start();
        
        // Resolve Adapter
        let adapter_arc: Arc<dyn Adapter> = if let Some(py_adapter) = adapter {
            py_adapter.adapter.clone() as Arc<dyn Adapter>
        } else {
            Arc::new(MockAdapter::new()) as Arc<dyn Adapter>
        };
        
        Interface {
            engine: Arc::new(OMSEngine::new(adapter_arc, 1.0, logger)),
        }
    }

    fn start_gateway(&self, adapter: &PyHantooAdapter) -> PyResult<()> {
        let (tx, rx) = mpsc::channel();
        adapter.adapter.set_monitor(tx);
        self.engine.start_gateway_listener(rx)?;
        Ok(())
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
    
    fn on_market_data(&self, py: Python, data: PyObject) -> PyResult<()> {
        self.engine.on_market_data(py, data)
    }
}
