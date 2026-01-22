use crate::adapter::Adapter;
use crate::oms::account::{AccountState, Position};
use crate::oms::order::{Order, OrderSide, OrderType, OrderState};
use crate::oms::order_book::OrderBook;
use anyhow::{anyhow, Result};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::adapter::hantoo::HantooAdapter;
use tungstenite::{connect, Message};
use url::Url;
use std::thread;
use std::sync::mpsc;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::adapter::{IncomingMessage, Trade};
use crate::oms::order_book::{OrderBookDelta, PriceLevel};
use rust_decimal::Decimal;
use std::str::FromStr;
use chrono::Local;

// Constants for Night Future
const NIGHT_ORDER_TR_ID: &str = "STTN1101U"; // Night Future Order (Real)
const NIGHT_CANCEL_TR_ID: &str = "STTN1103U"; // Night Future Cancel (Assumed)
const NIGHT_BALANCE_TR_ID: &str = "CTFN6118R"; // Night Balance
const URL_ORDER: &str = "/uapi/domestic-futureoption/v1/trading/order";
const URL_CANCEL: &str = "/uapi/domestic-futureoption/v1/trading/order-rvsecncl";
const URL_BALANCE: &str = "/uapi/domestic-futureoption/v1/trading/inquire-ngt-balance";
const URL_LIST_FUTURE: &str = "/uapi/domestic-futureoption/v1/quotations/display-board-futures";
const URL_LIST_OPTION: &str = "/uapi/domestic-futureoption/v1/quotations/display-board-option-list";

const TR_ID_LIST_FUTURE: &str = "FHPIF05030200";
const TR_ID_LIST_OPTION: &str = "FHPIO056104C0";

#[derive(Debug, Clone)]
struct NightOrderInfo {
    org_no: String,
    order_no: String,
}

#[derive(Debug)]
enum NightIncomingEvent {
    Trade(Trade),
    Snapshot(crate::oms::order_book::OrderBookSnapshot),
    Notice(NightNotice),
}

#[derive(Debug)]
struct NightNotice {
    order_no: String,
    cntg_yn: String,
    fill_qty: String,
    fill_price: String,
    rfus_yn: String,
}

pub struct HantooNightAdapter {
    inner: HantooAdapter,
    order_map: Arc<Mutex<HashMap<String, NightOrderInfo>>>,
    ws_thread: Mutex<Option<thread::JoinHandle<()>>>,
    sender: Mutex<Option<mpsc::Sender<IncomingMessage>>>,
    debug_ws: Arc<AtomicBool>,
}

impl HantooNightAdapter {
    pub fn new(config_path: &str) -> Result<Self> {
        let inner = HantooAdapter::new(config_path)?;
        let acct = inner.config().my_acct.clone().unwrap_or_default();
        let prod = inner.config().my_prod.clone().unwrap_or_default();
        println!("HantooNightAdapter initialized with Account: {}, Prod: {}", acct, prod);
        Ok(HantooNightAdapter {
            inner,
            order_map: Arc::new(Mutex::new(HashMap::new())),
            ws_thread: Mutex::new(None),
            sender: Mutex::new(None),
            debug_ws: Arc::new(AtomicBool::new(false)),
        })
    }
    
    pub fn set_debug_mode(&self, enabled: bool) {
        self.debug_ws.store(enabled, Ordering::Relaxed);
    }

    pub fn set_monitor(&self, sender: mpsc::Sender<IncomingMessage>) {
        let mut guard = self.sender.lock().unwrap();
        *guard = Some(sender);
    }

    pub fn subscribe(&self, symbol: &str) -> Result<()> {
        let symbol = symbol.to_string();
        self.start_ws_thread(symbol)?;
        Ok(())
    }

    fn start_ws_thread(&self, symbol: String) -> Result<()> {
        let config = self.inner.config().clone();
        let ws_url_str = config.ops.clone().ok_or(anyhow!("No WebSocket URL (ops) in config"))?;
        let approval_key = self.inner.get_ws_approval_key()?;
        
        let sender = self.sender.lock().unwrap().clone();
        let debug_ws = self.debug_ws.clone();
        let order_map_clone = self.order_map.clone();

        let handle = thread::spawn(move || {
            let full_url = format!("{}/tryitout/H0STCNT0", ws_url_str); 
            let url = Url::parse(&full_url).expect("Invalid WS URL");

            info!("NightAdapter connecting to WebSocket: {}", url);
            match connect(url) {
                Ok((mut socket, _)) => {
                    info!("Night WS Connected.");

                    // Subscribe to Night Future Trade (H0MFCNT0)
                    // Note: TR_ID in body should be H0MFCNT0 for Night Future Trade?
                    // The example uses `H0MFCNT0` (Realtime Night Future Conclusion)
                    let tr_id = "H0MFCNT0";
                    let sub_body = serde_json::json!({
                        "header": {"approval_key": approval_key, "custtype": "P", "tr_type": "1", "content-type": "utf-8"},
                        "body": {"input": {"tr_id": tr_id, "tr_key": symbol}} 
                    });
                    
                    if let Err(e) = socket.write_message(Message::Text(sub_body.to_string())) {
                        error!("Failed to subscribe to trade: {}", e);
                        return;
                    }
                    info!("Subscribed to Night Future Trade {} (H0MFCNT0)", symbol);

                    // Subscribe to Night Future Asking Price (H0MFASP0)
                    let tr_id_ask = "H0MFASP0";
                    let sub_body_ask = serde_json::json!({
                        "header": {"approval_key": approval_key, "custtype": "P", "tr_type": "1", "content-type": "utf-8"},
                        "body": {"input": {"tr_id": tr_id_ask, "tr_key": symbol}} 
                    });
                    
                    if let Err(e) = socket.write_message(Message::Text(sub_body_ask.to_string())) {
                        error!("Failed to subscribe to ask: {}", e);
                        // Continue even if ask fails
                    } else {
                        info!("Subscribed to Night Future Ask {} (H0MFASP0)", symbol);
                    }

                    // Subscribe to Private Execution Notices (H0MFCNI0)
                    let my_htsid = config.my_htsid.clone().unwrap_or_default();
                    if !my_htsid.is_empty() {
                         let tr_id_notice = "H0MFCNI0";
                         let sub_body_notice = serde_json::json!({
                            "header": {"approval_key": approval_key, "custtype": "P", "tr_type": "1", "content-type": "utf-8"},
                            "body": {"input": {"tr_id": tr_id_notice, "tr_key": my_htsid}}
                         });
                         if let Err(e) = socket.write_message(Message::Text(sub_body_notice.to_string())) {
                             error!("Failed to subscribe to notice: {}", e);
                         } else {
                             info!("Subscribed to Night Future Notice (H0MFCNI0) for {}", my_htsid);
                         }
                    }

                    loop {
                        match socket.read_message() {
                            Ok(msg) => {
                                match msg {
                                    Message::Text(text) => {
                                        if debug_ws.load(Ordering::Relaxed) {
                                            println!("[{}] WS_RECV: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"), text);
                                        }
                                        if text.contains("PINGPONG") {
                                            let _ = socket.write_message(Message::Text(text)); 
                                            continue;
                                        }
                                        
                                        if let Some(first) = text.chars().next() {
                                            if first == '0' || first == '1' {
                                                if let Some(s) = &sender {
                                                    if let Some(event) = Self::parse_ws_message(&text) {
                                                        if let Some(m) = Self::process_event(event, &order_map_clone) {
                                                            let _ = s.send(m);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    },
                                    Message::Close(_) => break,
                                    _ => {}
                                }
                            },
                            Err(e) => {
                                error!("WS Error: {}", e);
                                break;
                            }
                        }
                    }
                },
                Err(e) => error!("Connection failed: {}", e),
            }
        });

        let mut thread_guard = self.ws_thread.lock().unwrap();
        *thread_guard = Some(handle);
        
        Ok(())
    }

    fn parse_ws_message(text: &str) -> Option<NightIncomingEvent> {
        // 0|TR_ID|KEY|Data...
        let parts: Vec<&str> = text.split('|').collect();
        if parts.len() < 4 { return None; }
        
        let tr_id = parts[1];
        // let symbol = parts[2]; // Unused locally if we parse fields
        let data_part = parts[3..].join("|");
        // Assuming ^ separator as per Stock examples, but need to verify for Night Future.
        // Usually KIS uses ^.
        let fields: Vec<&str> = data_part.split('^').collect();
        
        match tr_id {
            "H0MFCNT0" => { // Night Future Trade
                if fields.len() > 9 {
                    // Correct symbol is fields[0] (e.g., A05602)
                    let symbol = fields[0]; 
                    
                    let price_str = fields[5];
                    let qty_str = fields[9];
                    
                    let price = Decimal::from_str(price_str).unwrap_or_default();
                    let qty = qty_str.parse().unwrap_or(0);
                    
                    return Some(NightIncomingEvent::Trade(Trade {
                        symbol: symbol.to_string(),
                        price,
                        quantity: qty,
                        timestamp: Local::now().timestamp_millis() as f64 / 1000.0,
                    }));
                }
            },
            "H0MFASP0" => { // Night Future Asking Price
                if fields.len() > 31 {
                    let symbol = fields[0];
                    let mut asks = Vec::new();
                    let mut bids = Vec::new();
                    
                    // 5 Levels
                    for i in 0..5 {
                        let price_idx = 2 + i;
                        let qty_idx = 22 + i;
                        let price = Decimal::from_str(fields[price_idx]).unwrap_or_default();
                        let qty: i64 = fields[qty_idx].parse().unwrap_or(0);
                        if price > Decimal::ZERO { asks.push((price, qty)); }
                        
                        let price_idx_b = 7 + i;
                        let qty_idx_b = 27 + i;
                        let price_b = Decimal::from_str(fields[price_idx_b]).unwrap_or_default();
                        let qty_b: i64 = fields[qty_idx_b].parse().unwrap_or(0);
                        if price_b > Decimal::ZERO { bids.push((price_b, qty_b)); }
                    }
                    
                    return Some(NightIncomingEvent::Snapshot(crate::oms::order_book::OrderBookSnapshot {
                         symbol: symbol.to_string(),
                         bids: bids.clone(),
                         asks: asks.clone(),
                         update_id: Local::now().timestamp_millis(),
                         timestamp: Local::now().timestamp_millis() as f64 / 1000.0,
                    }));
                }
            },
            "H0MFCNI0" => { // Night Future Execution/Order Notice
                if fields.len() > 13 {
                    let order_no = fields[2].to_string();
                    let cntg_yn = fields[13].to_string();
                    let fill_qty = fields[9].to_string();
                    let fill_price = fields[10].to_string();
                    let rfus_yn = if fields.len() > 11 { fields[11].to_string() } else { "".to_string() };
                    
                    return Some(NightIncomingEvent::Notice(NightNotice {
                        order_no,
                        cntg_yn,
                        fill_qty,
                        fill_price,
                        rfus_yn,
                    }));
                }
            },
            _ => {
                // info!("Unknown TR_ID: {}", tr_id);
            }
        }
        None
    }

    fn process_event(event: NightIncomingEvent, order_map: &Mutex<HashMap<String, NightOrderInfo>>) -> Option<IncomingMessage> {
        match event {
            NightIncomingEvent::Trade(t) => Some(IncomingMessage::Trade(t)),
            NightIncomingEvent::Snapshot(s) => Some(IncomingMessage::OrderBookSnapshot(s)),
            NightIncomingEvent::Notice(n) => {
                let map = order_map.lock().unwrap();
                if let Some((client_id, _)) = map.iter().find(|(_, info)| info.order_no == n.order_no) {
                    if n.cntg_yn == "2" { // Execution
                        let fill_qty = n.fill_qty.parse::<i64>().unwrap_or(0);
                        let fill_price = Decimal::from_str(&n.fill_price).unwrap_or_default();
                        
                        info!("Night Execution: {} qty={} price={}", client_id, fill_qty, fill_price);
                        return Some(IncomingMessage::Execution {
                            order_id: client_id.clone(),
                            fill_qty,
                            fill_price,
                        });
                    } else { // Accept/Modify/Cancel/Reject
                         // Use n.rfus_yn if needed
                         let _ = n.rfus_yn; 
                         
                         let state = OrderState::NEW; // Default to NEW/OPEN
                         
                         return Some(IncomingMessage::OrderUpdate {
                             order_id: client_id.clone(),
                             state,
                             msg: None,
                             updated_at: Local::now().timestamp_millis() as f64 / 1000.0,
                         });
                    }
                } else {
                     // println!("Unknown OrderNo in Notice: {}", n.order_no);
                }
                None
            }
        }
    }

    pub fn get_night_future_list(&self) -> Result<Vec<Value>> {
        let token = self.inner.get_token()?;
        let client = self.inner.client();
        let config = self.inner.config();

        let url = format!("{}{}", config.prod, URL_LIST_FUTURE);

        let params = [
            ("FID_COND_MRKT_DIV_CODE", "F"),
            ("FID_COND_SCR_DIV_CODE", "20503"),
            ("FID_COND_MRKT_CLS_CODE", "MKI")
        ];

        let resp = client.get(&url)
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .header("appkey", &config.my_app)
            .header("appsecret", &config.my_sec)
            .header("tr_id", TR_ID_LIST_FUTURE)
            .header("custtype", "P")
            .query(&params)
            .send()?;

        let status = resp.status();
        if !status.is_success() {
             let text = resp.text().unwrap_or_default();
             return Err(anyhow!("List Future API failed: {} - {}", status, text));
        }

        let data: Value = resp.json()?;
        if data["rt_cd"].as_str().unwrap_or("") != "0" {
             return Err(anyhow!("API Error: {}", data["msg1"].as_str().unwrap_or("")));
        }

        // Return output (array)
        if let Some(list) = data["output"].as_array() {
            Ok(list.clone())
        } else {
            Ok(vec![])
        }
    }

    pub fn get_night_option_list(&self) -> Result<Vec<Value>> {
        let token = self.inner.get_token()?;
        let client = self.inner.client();
        let config = self.inner.config();

        let url = format!("{}{}", config.prod, URL_LIST_OPTION);

        let params = [
            ("FID_COND_SCR_DIV_CODE", "509"),
            // Optional params, seems empty string in example is fine or omitted?
            // "FID_COND_MRKT_DIV_CODE": "",
            // "FID_COND_MRKT_CLS_CODE": ""
        ];

        let resp = client.get(&url)
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .header("appkey", &config.my_app)
            .header("appsecret", &config.my_sec)
            .header("tr_id", TR_ID_LIST_OPTION)
            .header("custtype", "P")
            .query(&params)
            .send()?;

        let status = resp.status();
        if !status.is_success() {
             let text = resp.text().unwrap_or_default();
             return Err(anyhow!("List Option API failed: {} - {}", status, text));
        }

        let data: Value = resp.json()?;
        if data["rt_cd"].as_str().unwrap_or("") != "0" {
             return Err(anyhow!("API Error: {}", data["msg1"].as_str().unwrap_or("")));
        }

        // Return output (array)
        if let Some(list) = data["output"].as_array() {
            Ok(list.clone())
        } else {
            Ok(vec![])
        }
    }
}

impl Adapter for HantooNightAdapter {
    fn connect(&self) -> Result<()> {
        // Reuse inner logic to verify token
        let _ = self.inner.get_token()?;
        info!("HantooNightAdapter connected (Token valid)");
        // No WS for Night for now
        Ok(())
    }

    fn disconnect(&self) -> Result<()> {
        info!("HantooNightAdapter disconnected");
        Ok(())
    }

    fn place_order(&self, order: &Order) -> Result<bool> {
        let token = self.inner.get_token()?;
        let client = self.inner.client();
        let config = self.inner.config();
        
        let url = format!("{}{}", config.prod, URL_ORDER);
        
        // Price logic
        let price_str = if let Some(p) = order.price {
            p.to_string()
        } else {
            "0".to_string()
        };
        
        // Map OrderType to Codes
        let (nmpr_type, ord_dvsn) = match order.order_type {
            OrderType::LIMIT => ("01", "01"),
            OrderType::MARKET => ("02", "02"),
        };
        
        let side_cd = match order.side {
            OrderSide::BUY => "02",
            OrderSide::SELL => "01",
        };

        // Params for Night Future
        let body = serde_json::json!({
            "CANO": config.my_acct_future.as_deref().unwrap_or(""), 
            "ACNT_PRDT_CD": config.my_prod_future.as_deref().unwrap_or("01"),
            "SHTN_PDNO": order.symbol,     // Short Product No (e.g. 101W09)
            "ORD_QTY": order.quantity.to_string(),
            "UNIT_PRICE": price_str,
            "SLL_BUY_DVSN_CD": side_cd,
            "NMPR_TYPE_CD": nmpr_type,     // 01: Limit, 02: Market
            "ORD_DVSN_CD": ord_dvsn,       // Same as NMPR?
            "KRX_NMPR_CNDT_CD": "0",       // 0: None
            "ORD_PRCS_DVSN_CD": "02",      // Order Process: 02 (Transmit)
            "CTAC_TLNO": "",
            "FUOP_ITEM_DVSN_CD": ""
        });

        // Result Debug
        let body_str = serde_json::to_string_pretty(&body).unwrap_or_default();
        println!("Night Order Request: URL={} Body={}", url, body_str);

        let resp = client.post(&url)
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .header("appkey", &config.my_app)
            .header("appsecret", &config.my_sec)
            .header("tr_id", NIGHT_ORDER_TR_ID)
            .header("custtype", "P")
            .json(&body)
            .send()?;
            
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        println!("Night Order Response: Status={} Body={}", status, text);

        if status.is_success() {
             let data: Value = serde_json::from_str(&text).map_err(|e| anyhow!("Parse error: {}", e))?;
             
             if let Some(output) = data.get("output") {
                 let org_no = output["KRX_FWDG_ORD_ORGNO"].as_str().unwrap_or("").to_string();
                 let order_no = output["ODNO"].as_str().unwrap_or("").to_string();
                 
                 if !org_no.is_empty() && !order_no.is_empty() {
                     println!("Night Order Placed: Org={}, No={}", org_no, order_no);
                     
                     if let Some(client_id) = &order.order_id {
                         let mut map = self.order_map.lock().unwrap();
                         map.insert(client_id.clone(), NightOrderInfo { org_no, order_no });
                     }
                 }
                 Ok(true)
             } else {
                 // Check if rt_cd != 0
                 let msg = data["msg1"].as_str().unwrap_or("Unknown");
                 println!("Night Order Failed (RT!=0): {}", msg);
                 Ok(false)
             }
        } else {
             println!("Night Order Request Failed: {}", text);
             Ok(false)
        }
    }

    fn cancel_order(&self, order_id: &str) -> Result<bool> {
        let token = self.inner.get_token()?;
        let client = self.inner.client();
        let config = self.inner.config();
        
        let (org_no, order_no) = {
            let map = self.order_map.lock().unwrap();
            match map.get(order_id) {
                Some(i) => (i.org_no.clone(), i.order_no.clone()),
                None => return Err(anyhow!("Order not found in map")),
            }
        };

        let url = format!("{}{}", config.prod, URL_CANCEL);
        
        // Cancel Body
        let body = serde_json::json!({
            "CANO": config.my_acct_future.as_deref().unwrap_or(""), 
            "ACNT_PRDT_CD": config.my_prod_future.as_deref().unwrap_or("01"),
            "KRX_FWDG_ORD_ORGNO": org_no,
            "ORGN_ODNO": order_no,
            "ORD_DVSN": "00", 
            "RVSE_CNCL_DVSN_CD": "02", // Cancel
            "ORD_QTY": "0", // Cancel all
            "ORD_UNPR": "0",
            "QTY_ALL_ORD_YN": "Y",
            "EXCG_ID_DVSN_CD": "KRX" // Assume this is still required?
        });

        let resp = client.post(&url)
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .header("appkey", &config.my_app)
            .header("appsecret", &config.my_sec)
            .header("tr_id", NIGHT_CANCEL_TR_ID)
            .header("custtype", "P")
            .json(&body)
            .send()?;
            
        if resp.status().is_success() {
             info!("Night Cancel Success for {}", order_id);
             Ok(true)
        } else {
             let t = resp.text().unwrap_or_default();
             error!("Night Cancel Failed: {}", t);
             Ok(false)
        }
    }

    fn get_order_book_snapshot(&self, symbol: &str) -> Result<OrderBook> {
        let token = self.inner.get_token()?;
        let client = self.inner.client();
        let config = self.inner.config();
        
        let url = format!("{}{}", config.prod, "/uapi/domestic-futureoption/v1/quotations/inquire-asking-price");
        
        // Assume 'F' for Futures. Could be 'JF' for Equity Futures. Defaulting to 'F'.
        let params = [
            ("FID_COND_MRKT_DIV_CODE", "CM"), //CME for night market
            ("FID_INPUT_ISCD", symbol)
        ];

        let resp = client.get(&url)
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .header("appkey", &config.my_app)
            .header("appsecret", &config.my_sec)
            .header("tr_id", "FHMIF10010000") // ASK PRICE TR_ID
            .header("custtype", "P")
            .query(&params)
            .send()?;
            
        if !resp.status().is_success() {
             let status = resp.status();
             let t = resp.text().unwrap_or_default();
             return Err(anyhow!("Snapshot API Failed {}: {}", status, t));
        }
        
        let data: Value = resp.json()?;
        if data["rt_cd"].as_str().unwrap_or("") != "0" {
             return Err(anyhow!("API Error: {}", data["msg1"].as_str().unwrap_or("")));
        }
        
        let mut ob = OrderBook::new(symbol.to_string());
        ob.timestamp = Local::now().timestamp_millis() as f64 / 1000.0;
        ob.last_update_id = Local::now().timestamp_millis();

        // output1 usually contains the grid.
        let mut source = data["output1"].as_object();
        if source.is_none() {
            source = data["output2"].as_object();
        }
        // If output1 had no keys, maybe output2?
        // Actually, let's try output1 then output2 if not found keys.
        
        let parse_from = |out: &serde_json::Map<String, Value>, ob: &mut OrderBook| {
            for i in 1..=5 {
                let ask_price_key = format!("askp{}", i);
                let bid_price_key = format!("bidp{}", i);
                let futs_ask_price_key = format!("futs_askp{}", i);
                let futs_bid_price_key = format!("futs_bidp{}", i);
                let ask_qty_key = format!("askp_rsqn{}", i);
                let bid_qty_key = format!("bidp_rsqn{}", i);
                
                let ap_str = out.get(&futs_ask_price_key).or_else(|| out.get(&ask_price_key)).and_then(|v| v.as_str()).unwrap_or("0");
                let bp_str = out.get(&futs_bid_price_key).or_else(|| out.get(&bid_price_key)).and_then(|v| v.as_str()).unwrap_or("0");
                let aq_str = out.get(&ask_qty_key).and_then(|v| v.as_str()).unwrap_or("0");
                let bq_str = out.get(&bid_qty_key).and_then(|v| v.as_str()).unwrap_or("0");
                
                let ap = Decimal::from_str(ap_str).unwrap_or_default();
                let bp = Decimal::from_str(bp_str).unwrap_or_default();
                let aq: i64 = aq_str.parse().unwrap_or(0);
                let bq: i64 = bq_str.parse().unwrap_or(0);
                
                if ap > Decimal::ZERO { ob.asks.insert(ap, aq); }
                if bp > Decimal::ZERO { ob.bids.insert(bp, bq); }
            }
        };

        if let Some(out1) = data["output1"].as_object() {
             parse_from(out1, &mut ob);
        }
        if ob.asks.is_empty() && ob.bids.is_empty() {
             if let Some(out2) = data["output2"].as_object() {
                 parse_from(out2, &mut ob);
             }
        }
        
        Ok(ob)
    }

    fn get_account_snapshot(&self, _account_id: &str) -> Result<AccountState> {
        let token = self.inner.get_token()?;
        let client = self.inner.client();
        let config = self.inner.config();
        
        let url = format!("{}{}", config.prod, URL_BALANCE);
        
        let cano = config.my_acct_future.as_deref().unwrap_or("");
        let prdt = config.my_prod_future.as_deref().unwrap_or("01");

        let params = [
            ("CANO", cano),
            ("ACNT_PRDT_CD", prdt),
            ("MGNA_DVSN", "01"), 
            ("EXCC_STAT_CD", "1"), 
            ("ACNT_PWD", ""),
            ("CTX_AREA_FK200", ""),
            ("CTX_AREA_NK200", "")
        ];

        let resp = client.get(&url)
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .header("appkey", &config.my_app)
            .header("appsecret", &config.my_sec)
            .header("tr_id", NIGHT_BALANCE_TR_ID)
            .query(&params)
            .send()?;

        if !resp.status().is_success() {
            return Err(anyhow!("Balance API failed: {}", resp.status()));
        }

        let data: Value = resp.json()?;
        if data["rt_cd"].as_str().unwrap_or("") != "0" {
             return Err(anyhow!("API Error: {}", data["msg1"].as_str().unwrap_or("")));
        }

        let mut acct = AccountState::new();
        
        // Output2: Balance
        if let Some(out2) = data["output2"].as_object() {
             // Try 'dnca_tot_amt' like Stock? Or Night specific?
             if let Some(val) = out2.get("dnca_tot_amt") {
                 if let Some(s) = val.as_str() {
                     acct.balance = Decimal::from_str(s).unwrap_or_default();
                 }
             }
        }
        
        // Output1: Positions
        if let Some(out1) = data["output1"].as_array() {
            for item in out1 {
                let symbol = item.get("pdno").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let qty_str = item.get("hldg_qty").and_then(|v| v.as_str()).unwrap_or("0");
                
                let price_str = item.get("pchs_avg_pric")
                    .or_else(|| item.get("avg_unpr"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("0");
                    
                let curr_str = item.get("prpr")
                    .or_else(|| item.get("trad_pric"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("0");
                
                let qty = qty_str.parse::<i64>().unwrap_or(0);
                if qty > 0 {
                    let avg = Decimal::from_str(price_str).unwrap_or_default();
                    let curr = Decimal::from_str(curr_str).unwrap_or_default();
                    
                    let pos = Position::new(symbol.clone(), qty, avg, curr);
                    acct.positions.insert(symbol, pos);
                }
            }
        }

        Ok(acct)
    }


}

