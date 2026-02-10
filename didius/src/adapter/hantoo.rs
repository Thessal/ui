use crate::adapter::Adapter;
use crate::oms::account::AccountState;
use crate::oms::order::{Order, OrderSide, OrderType, OrderState};
use crate::oms::order_book::OrderBook;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Local};
use log::{error, info, warn};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::sync::mpsc;
use std::collections::HashMap;
use tungstenite::{connect, Message};
use url::Url;
use crate::adapter::IncomingMessage;
use rust_decimal::Decimal;
use std::str::FromStr;
use crate::oms::order_book::{OrderBookSnapshot};

use aes::Aes256;
use cbc::Decryptor;
use block_padding::Pkcs7;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use cbc::cipher::{BlockDecryptMut, KeyIvInit};

type Aes256CbcDec = Decryptor<Aes256>;

#[derive(Debug, Deserialize, Clone)]
pub struct HantooConfig {
    pub my_app: String,
    pub my_sec: String,
    pub prod: String, // Base URL
    #[serde(alias = "my_acct_stock")]
    pub my_acct: Option<String>,
    pub my_acct_future: Option<String>,
    #[serde(alias = "my_prod_stock")]
    pub my_prod: Option<String>,
    pub my_prod_future: Option<String>,
    pub my_htsid: Option<String>,
    pub ops: Option<String>, // WebSocket URL
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenData {
    token: String,
    #[serde(rename = "valid-date")]
    valid_date: String,
}

pub struct HantooAdapter {
    config: HantooConfig,
    token: Mutex<Option<String>>,
    token_exp: Mutex<Option<DateTime<Local>>>,
    client: Client,
    auth_dir: PathBuf,
    // WebSocket state
    approval_key: Mutex<Option<String>>,
    ws_thread: Mutex<Option<thread::JoinHandle<()>>>,
    // Map ClientOrderID -> (OrgNo, OrderNo)
    // Changed to Arc<Mutex> to share with WS thread
    order_map: Arc<Mutex<HashMap<String, HantooOrderInfo>>>,
    // Channel to Engine
    sender: Mutex<Option<mpsc::Sender<IncomingMessage>>>,
    // Subscribed Symbols
    subscribed_symbols: Mutex<Vec<String>>,
    // Debug flag for WS logging
    debug_ws: Arc<AtomicBool>,
    
    // Encryption
    ws_aes_iv: Arc<Mutex<Option<Vec<u8>>>>,
    ws_aes_key: Arc<Mutex<Option<Vec<u8>>>>,
}

#[derive(Debug, Clone)]
struct HantooOrderInfo {
    org_no: String,
    order_no: String,
    exchange: String
}

impl HantooAdapter {
    pub fn new(config_path: &str) -> Result<Self> {
        let config_str = fs::read_to_string(config_path)
            .map_err(|e| anyhow!("Failed to read hantoo config from {}: {}", config_path, e))?;
        let config: HantooConfig = serde_yaml::from_str(&config_str)
            .map_err(|e| anyhow!("Failed to parse hantoo config: {}", e))?;

        let adapter = HantooAdapter {
            config,
            token: Mutex::new(None),
            token_exp: Mutex::new(None),
            client: Client::new(),
            auth_dir: PathBuf::from("auth"),
            approval_key: Mutex::new(None),

            ws_thread: Mutex::new(None), 
            order_map: Arc::new(Mutex::new(HashMap::new())),
            sender: Mutex::new(None),
            subscribed_symbols: Mutex::new(Vec::new()),
            debug_ws: Arc::new(AtomicBool::new(false)),
            
            ws_aes_iv: Arc::new(Mutex::new(None)),
            ws_aes_key: Arc::new(Mutex::new(None)),
        };

        Ok(adapter)
    }

    pub(crate) fn config(&self) -> &HantooConfig {
        &self.config
    }
    
    pub(crate) fn client(&self) -> &Client {
        &self.client
    }

    pub(crate) fn set_monitor_internal(&self, sender: mpsc::Sender<IncomingMessage>) {
        let mut guard = self.sender.lock().unwrap();
        *guard = Some(sender);
    }
    
    pub fn subscribe_market(&self, symbols: &[String]) -> Result<()> {
        let mut guard = self.subscribed_symbols.lock().unwrap();
        for s in symbols {
            if !guard.contains(s) {
                guard.push(s.to_string());
            }
        }
        Ok(())
    }
    
    // pub fn add_subscription(&self, _symbol: String) {
    //     // Placeholder driven by start_ws_thread using subscribed_symbols
    // }
    
    pub(crate) fn get_token(&self) -> Result<String> {
        {
            let token_guard = self.token.lock().unwrap();
            let exp_guard = self.token_exp.lock().unwrap();
            if let (Some(token), Some(exp)) = (token_guard.as_ref(), exp_guard.as_ref()) {
                 if *exp > Local::now() {
                     return Ok(token.clone());
                 }
            }
        }

        if let Ok(cached_token) = self.read_token_from_file() {
             let mut token_guard = self.token.lock().unwrap();
             *token_guard = Some(cached_token.clone());
             return Ok(cached_token);
        }

        self.refresh_token()
    }

    fn read_token_from_file(&self) -> Result<String> {
        let token_path = self.auth_dir.join("hantoo_token.yaml");
        if !token_path.exists() {
            return Err(anyhow!("Token file not found"));
        }

        let content = fs::read_to_string(token_path)?;
        let data: TokenData = serde_yaml::from_str(&content)?;

        let expiry = chrono::NaiveDateTime::parse_from_str(&data.valid_date, "%Y-%m-%d %H:%M:%S")
            .map_err(|e| anyhow!("Failed to parse token date: {}", e))?
            .and_local_timezone(Local)
            .unwrap();

        if expiry > Local::now() {
            let mut exp_guard = self.token_exp.lock().unwrap();
            *exp_guard = Some(expiry);
            return Ok(data.token);
        }

        Err(anyhow!("Token expired"))
    }

    fn refresh_token(&self) -> Result<String> {
        let url = format!("{}/oauth2/tokenP", self.config.prod);
        let body = serde_json::json!({
            "grant_type": "client_credentials",
            "appkey": self.config.my_app,
            "appsecret": self.config.my_sec
        });

        let resp = self.client.post(&url)
            .json(&body)
            .send()
            .map_err(|e| anyhow!("Token request failed: {}", e))?;

        if !resp.status().is_success() {
             let text = resp.text().unwrap_or_default();
             return Err(anyhow!("Token request error: {}", text));
        }

        let data: Value = resp.json()
             .map_err(|e| anyhow!("Failed to parse token response: {}", e))?;

        let access_token = data["access_token"].as_str()
            .ok_or_else(|| anyhow!("No access_token in response"))?
            .to_string();
            
        let expired_str = data["access_token_token_expired"].as_str()
            .ok_or_else(|| anyhow!("No expiration in response"))?
            .to_string();

        self.save_token_to_file(&access_token, &expired_str)?;
        
        // Update memory
        let mut token_guard = self.token.lock().unwrap();
        *token_guard = Some(access_token.clone());
        
        let expiry = chrono::NaiveDateTime::parse_from_str(&expired_str, "%Y-%m-%d %H:%M:%S")
            .map_err(|e| anyhow!("Failed to parse new token date: {}", e))?
            .and_local_timezone(Local)
            .unwrap();
            
        let mut exp_guard = self.token_exp.lock().unwrap();
        *exp_guard = Some(expiry);

        Ok(access_token)
    }

    fn save_token_to_file(&self, token: &str, expired_str: &str) -> Result<()> {
        if !self.auth_dir.exists() {
            fs::create_dir_all(&self.auth_dir)?;
        }
        let token_path = self.auth_dir.join("hantoo_token.yaml");
        
        let mut file = fs::File::create(token_path)?;
        writeln!(file, "token: {}", token)?;
        writeln!(file, "valid-date: {}", expired_str)?;
        
        Ok(())
    }

    pub(crate) fn get_ws_approval_key(&self) -> Result<String> {
        // Check memory
        {
            let key = self.approval_key.lock().unwrap();
            if let Some(k) = key.as_ref() {
                return Ok(k.clone());
            }
        }
        
        let url = format!("{}/oauth2/Approval", self.config.prod);
        let body = serde_json::json!({
            "grant_type": "client_credentials",
            "appkey": self.config.my_app,
            "secretkey": self.config.my_sec
        });

        let resp = self.client.post(&url)
            .json(&body)
            .send()
            .map_err(|e| anyhow!("WS Approval request failed: {}", e))?;

        if !resp.status().is_success() {
            let text = resp.text().unwrap_or_default();
            return Err(anyhow!("WS Approval error: {}", text));
        }

        let data: Value = resp.json()?;
        let key = data["approval_key"].as_str()
            .ok_or_else(|| anyhow!("No approval_key in response"))?
            .to_string();

        let mut guard = self.approval_key.lock().unwrap();
        *guard = Some(key.clone());
        
        Ok(key)
    }

    pub fn set_debug_mode(&self, enabled: bool) {
        self.debug_ws.store(enabled, Ordering::Relaxed);
    }

    fn start_ws_thread(&self) -> Result<()> {
        let ws_url_str = self.config.ops.clone().ok_or(anyhow!("No WebSocket URL (ops) in config"))?;
        let approval_key = self.get_ws_approval_key()?;
        let my_htsid = self.config.my_htsid.clone().unwrap_or_default();
        
        // Sender clone
        let sender = self.sender.lock().unwrap().clone();
        
        // Get symbols to subscribe
        let symbols: Vec<String> = self.subscribed_symbols.lock().unwrap().clone();
        let debug_ws = self.debug_ws.clone();
        let order_map_clone = self.order_map.clone();
        
        let aes_iv = self.ws_aes_iv.clone();
        let aes_key = self.ws_aes_key.clone();
        
        let handle = thread::spawn(move || {
            let full_url = format!("{}/tryitout/H0STCNT0", ws_url_str); // Typical suffix
            let url = Url::parse(&full_url).expect("Invalid WS URL");

            info!("Connecting to WebSocket: {}", url);
            match connect(url) {
                Ok((mut socket, response)) => {
                    info!("WebSocket Connected. Response: {:?}", response);

                    // Subscribe to Execution (Private)
                    if !my_htsid.is_empty() {
                         let tr_id = if ws_url_str.contains("openapivts") { "H0STCNI9" } else { "H0STCNI0" };
                         let sub_body = serde_json::json!({
                            "header": {"approval_key": approval_key, "custtype": "P", "tr_type": "1", "content-type": "utf-8"},
                            "body": {"input": {"tr_id": tr_id, "tr_key": my_htsid}}
                         });
                         let _ = socket.send(Message::Text(sub_body.to_string()));
                    }
                    
                    // Subscribe to Market Data for all symbols
                    for target_symbol in symbols {
                        // Trade
                        {
                            let sub_body = serde_json::json!({
                                "header": {"approval_key": approval_key, "custtype": "P", "tr_type": "1", "content-type": "utf-8"},
                                "body": {"input": {"tr_id": "H0SCCNT0", "tr_key": target_symbol}} // H0SCCNT0 is "Realtime Stock Conclusion" (KOSPI)
                            }); 
                            let _ = socket.send(Message::Text(sub_body.to_string()));
                        }
                        
                        // Asking Price (Total - 10 levels) H0UNASP0
                        {
                            let sub_body = serde_json::json!({
                                "header": {"approval_key": approval_key, "custtype": "P", "tr_type": "1", "content-type": "utf-8"},
                                "body": {"input": {"tr_id": "H0UNASP0", "tr_key": target_symbol}} 
                            });
                            let _ = socket.send(Message::Text(sub_body.to_string()));
                        }
                        
                        info!("Subscribed to {} Trade/Ask(Total)", target_symbol);
                    }

                    // Loop
                    loop {
                        match socket.read() {
                            Ok(msg) => {
                                match msg {
                                    Message::Text(text) => {
                                        if debug_ws.load(Ordering::Relaxed) {
                                            println!("[{}] WS_RECV: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"), text);
                                        }
                                        if text.contains("PINGPONG") {
                                            let _ = socket.send(Message::Text(text)); 
                                            continue;
                                        }
                                        
                                        // Check for Subscription Response with IV/Key
                                        if text.contains("SUBSCRIBE SUCCESS") && text.contains("iv") {
                                            if let Ok(val) = serde_json::from_str::<Value>(&text) {
                                                if let Some(output) = val.get("body").and_then(|b| b.get("output")) {
                                                    let iv_str = output["iv"].as_str().unwrap_or("");
                                                    let key_str = output["key"].as_str().unwrap_or("");
                                                    if !iv_str.is_empty() && !key_str.is_empty() {
                                                        info!("Received Encryption Keys: IV={}, Key={}", iv_str, key_str);
                                                        *aes_iv.lock().unwrap() = Some(iv_str.as_bytes().to_vec());
                                                        *aes_key.lock().unwrap() = Some(key_str.as_bytes().to_vec());
                                                    }
                                                }
                                            }
                                        }
                                        
                                        if let Some(first) = text.chars().next() {
                                            if first == '0' || first == '1' { // Data
                                                if let Some(s) = &sender {
                                                    // Pass keys to parse
                                                    let iv = aes_iv.lock().unwrap().clone();
                                                    let key = aes_key.lock().unwrap().clone();
                                                    
                                                    if let Some(msg) = Self::parse_ws_message(&text, &order_map_clone, iv, key) {
                                                        let _ = s.send(msg);
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
    
    fn parse_ws_message(text: &str, order_map: &Mutex<HashMap<String, HantooOrderInfo>>, iv_opt: Option<Vec<u8>>, key_opt: Option<Vec<u8>>) -> Option<IncomingMessage> {
        let parts: Vec<&str> = text.split('|').collect();
        if parts.len() < 4 { return None; }
        
        let tr_id = parts[1];
        let symbol = parts[2];
        let data_part = parts[3..].join("|"); 
        
        // Decryption Logic for H0STCNI0/9
        let final_data = if tr_id == "H0STCNI0" || tr_id == "H0STCNI9" {
             // Check if it looks encrypted (Base64 is alphanumeric + +/=)
             // And if we have keys
             if let (Some(iv), Some(key)) = (iv_opt, key_opt) {
                 // Try to decode base64
                 if let Ok(mut ciphertext) = BASE64.decode(&data_part) {
                     // Decrypt
                     let decryptor = Aes256CbcDec::new_from_slices(&key, &iv).ok()?;
                     // We need a buffer copy because decrypt works in place usually or writes to out
                     // Using decrypt_padded_mut
                     if let Ok(plaintext_slice) = decryptor.decrypt_padded_mut::<Pkcs7>(&mut ciphertext) {
                         if let Ok(s) = String::from_utf8(plaintext_slice.to_vec()) {
                              // success
                              s
                         } else {
                             data_part
                         }
                     } else {
                         data_part 
                     }
                 } else {
                     data_part
                 }
             } else {
                 data_part
             }
        } else {
            data_part
        };

        if tr_id == "H0STCNI0" || tr_id == "H0STCNI9" {
            // Decrypted successfully
        }

        let fields: Vec<&str> = final_data.split('^').collect();
        
        match tr_id {
            "H0STCNT0" | "H0SCCNT0" => { // Trade
                if fields.len() > 2 {
                    let price = Decimal::from_str(fields[1]).unwrap_or_default();
                    let qty = if fields.len() > 12 { fields[12].parse().unwrap_or(0) } else { 0 };
                   
                    return Some(IncomingMessage::MarketTrade {
                        symbol: symbol.to_string(),
                        price,
                        quantity: qty,
                        timestamp: Local::now().timestamp_millis() as f64 / 1000.0,
                    });
                }
            },
            "H0STASP0" => { // Asking Price
                if fields.len() > 5 {
                     let mut asks = Vec::new();
                     let mut bids = Vec::new();
                     
                     let a1 = Decimal::from_str(fields[3]).unwrap_or_default();
                     let b1 = Decimal::from_str(fields[4]).unwrap_or_default();
                     let aq1 = fields[5].parse().unwrap_or(0);
                     let bq1 = fields[6].parse().unwrap_or(0);
                     
                     asks.push((a1, aq1));
                     bids.push((b1, bq1));
                     
                     let delta = OrderBookSnapshot {
                         symbol: symbol.to_string(),
                         bids,
                         asks,
                         update_id: Local::now().timestamp_millis(),
                         timestamp: Local::now().timestamp_millis() as f64 / 1000.0,
                     };
                     return Some(IncomingMessage::OrderBookSnapshot(delta));
                }
            },
            "H0STCNI0" | "H0STCNI9" => { // Execution Notice
                 // fields parsing based on ccnl_notice.py
                 // 0: CUST_ID, 1: ACNT_NO, 2: ODER_NO, 3: ODER_QTY, ... 
                 // 9: CNTG_QTY, 10: CNTG_UNPR, ...
                 // 12: RFUS_YN, 13: CNTG_YN (1: Accept, 2: Execute)
                 
                 if fields.len() > 14 {
                     let order_no = fields[2];
                     let cntg_yn = fields[13]; // 1 or 2
                     
                     // Find Client Order ID
                     let map = order_map.lock().unwrap();
                     if let Some((client_id, info)) = map.iter().find(|(_, info)| info.order_no == order_no) {
                          info!("Hantoo Parse: Found Order Map for OrderNo: {} -> ClientID: {}", order_no, client_id);
                          info!("Hantoo Parse: fields[9](qty)={}, fields[10](price)={}, cntg_yn={}, rfus_yn={}", fields[9], fields[10], cntg_yn, fields[12]);

                          if cntg_yn == "2" { // Execution
                               let fill_qty = fields[9].parse::<i64>().unwrap_or(0);
                               let fill_price = Decimal::from_str(fields[10]).unwrap_or_default();
                               
                               info!("Hantoo Parse: Execution for {}, qty={}, price={}", client_id, fill_qty, fill_price);
                               return Some(IncomingMessage::Execution {
                                   order_id: client_id.clone(),
                                   fill_qty,
                                   fill_price,
                               });
                          } else if cntg_yn == "1" { // Accepted / Modify / Cancel
                               let rfus_yn = fields[12];
                               
                               let state = if rfus_yn == "Y" { 
                                   OrderState::REJECTED 
                               } else { 
                                   // Simply NEW for now, could be CANCELED if msg implies. 
                                   // But H0STCNI0 is complex. 
                                   // For MVP, if it is not refused, we assume NEW or PENDING->NEW.
                                   OrderState::NEW 
                               };
                               
                               info!("Hantoo Parse: Update for {}, state={:?}", client_id, state);
                               return Some(IncomingMessage::OrderStatus {
                                   order_id: client_id.clone(),
                                   state,
                                   filled_qty: 0,
                                   filled_price: None,
                                   msg: None,
                                   updated_at: Local::now().timestamp_millis() as f64 / 1000.0,
                               });
                          }
                     } else {
                         // Unknown order (maybe manual order not in OMS). Log/Ignore?
                         // Print map keys for debugging
                         let keys: Vec<_> = map.values().map(|v| v.order_no.clone()).collect();
                         warn!("Received notice for unknown order_no: {}. Known OrderNos: {:?}", order_no, keys);
                     }
                 } else {
                     warn!("H0STCNI0 received but insufficient fields: len={}", fields.len());
                 }
             },
            "H0UNASP0" => { // Asking Price (Total - 10 levels)
                if fields.len() > 42 {
                    let symbol = fields[0]; 
                    
                    let mut asks = Vec::new();
                    let mut bids = Vec::new();

                    for i in 0..10 {
                         let ask_p_idx = 3 + i;
                         let bid_p_idx = 13 + i;
                         let ask_q_idx = 23 + i;
                         let bid_q_idx = 33 + i;

                         let ap = Decimal::from_str(fields[ask_p_idx]).unwrap_or_default();
                         let bp = Decimal::from_str(fields[bid_p_idx]).unwrap_or_default();
                         let aq: i64 = fields[ask_q_idx].parse().unwrap_or(0);
                         let bq: i64 = fields[bid_q_idx].parse().unwrap_or(0);

                         if ap > Decimal::ZERO { asks.push((ap, aq)); }
                         if bp > Decimal::ZERO { bids.push((bp, bq)); }
                    }

                 return Some(IncomingMessage::OrderBookSnapshot(crate::oms::order_book::OrderBookSnapshot {
                     symbol: symbol.to_string(),
                     bids: bids.clone(),
                     asks: asks.clone(),
                     update_id: Local::now().timestamp_millis(),
                     timestamp: Local::now().timestamp_millis() as f64 / 1000.0,
                 }));
                }
            },
            _ => {}
        }
        None
    }
}

impl Adapter for HantooAdapter {
    fn set_monitor(&self, sender: std::sync::mpsc::Sender<IncomingMessage>) {
        self.set_monitor_internal(sender);
    }
    fn connect(&self) -> Result<()> {
        let _ = self.get_token()?;
        info!("HantooAdapter connected (token verified)");
        
        if let Err(e) = self.start_ws_thread() {
            warn!("Failed to start WebSocket: {}", e);
        }

        Ok(())
    }
    
    fn subscribe(&self, symbols: &[String]) -> Result<()> {
        self.subscribe_market(symbols)
    }

    fn disconnect(&self) -> Result<()> {
        info!("HantooAdapter disconnected");
        Ok(())
    }

    fn place_order(&self, order: &Order) -> Result<bool> {
        let token = self.get_token()?;
        let url = format!("{}/uapi/domestic-stock/v1/trading/order-cash", self.config.prod);
        
        let is_virtual = self.config.prod.contains("openapivts");
        let tr_id = match order.side {
            OrderSide::BUY => if is_virtual { "VTTC0012U" } else { "TTTC0012U" },
            OrderSide::SELL => if is_virtual { "VTTC0011U" } else { "TTTC0011U" },
        };
        
        let price_str = if let Some(p) = order.price {
            p.to_string()
        } else {
            "0".to_string()
        };
        
        let ord_dvsn = match order.order_type {
            OrderType::LIMIT => "00",
            OrderType::MARKET => "01",
        };

        let body = serde_json::json!({
            "CANO": self.config.my_acct.as_deref().unwrap_or(""), 
            "ACNT_PRDT_CD": self.config.my_prod.as_deref().unwrap_or("01"),
            "PDNO": order.symbol,
            "ORD_DVSN": ord_dvsn,
            "ORD_QTY": order.quantity.to_string(),
            "ORD_UNPR": price_str,
            "EXCG_ID_DVSN_CD": order.exchange, 
            "SLL_TYPE": "", 
            "CNDT_PRIC": ""
        });

        let resp = self.client.post(&url)
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .header("appkey", &self.config.my_app)
            .header("appsecret", &self.config.my_sec)
            .header("tr_id", tr_id)
            .header("custtype", "P")
            .json(&body)
            .send()?;
            
        if resp.status().is_success() {
             let text = resp.text().unwrap_or_default();
             let data: Value = serde_json::from_str(&text).map_err(|e| anyhow!("Failed to parse response: {}", e))?;

             if data["rt_cd"].as_str().unwrap_or("") != "0" {
                 let msg = data["msg1"].as_str().unwrap_or("Unknown error");
                 error!("Order placement API error: {}", msg);
                 return Ok(false);
             }
             
             if let Some(output) = data.get("output") {
                 let org_no = output["KRX_FWDG_ORD_ORGNO"].as_str().unwrap_or("").to_string();
                 let order_no = output["ODNO"].as_str().unwrap_or("").to_string();
                 let exchange = order.exchange.clone();

                 if !org_no.is_empty() && !order_no.is_empty() {
                     info!("Order Placed: OrgNo={}, OrderNo={}, Exhange={}", org_no, order_no, exchange);
                     
                     if let Some(client_id) = &order.order_id {
                          let info = HantooOrderInfo { org_no, order_no, exchange };
                          let mut map = self.order_map.lock().unwrap();
                          map.insert(client_id.clone(), info);
                     }
                 }
             }

             Ok(true)
        } else {
             let text = resp.text().unwrap_or_default();
             error!("Order placement failed: {}", text);
             Ok(false)
        }
    }

    fn cancel_order(&self, order_id: &str) -> Result<bool> {
        let token = self.get_token()?;
        let url = format!("{}/uapi/domestic-stock/v1/trading/order-rvsecncl", self.config.prod);
        
        let (org_no, order_no, exchange) = {
            let map = self.order_map.lock().unwrap();
            match map.get(order_id) {
                Some(info) => (info.org_no.clone(), info.order_no.clone(), info.exchange.clone()),
                None => {
                    return Err(anyhow!("Order ID not found in local map: {}", order_id));
                }
            }
        };

        let is_virtual = self.config.prod.contains("openapivts");
        let tr_id = if is_virtual { "VTTC0013U" } else { "TTTC0013U" };
        
        let cano = self.config.my_acct.as_deref().unwrap_or("");
        let prdt = self.config.my_prod.as_deref().unwrap_or("01");

        let body = serde_json::json!({
            "CANO": cano,
            "ACNT_PRDT_CD": prdt,
            "KRX_FWDG_ORD_ORGNO": org_no,
            "ORGN_ODNO": order_no,
            "ORD_DVSN": "00", 
            "RVSE_CNCL_DVSN_CD": "02",
            "ORD_QTY": "0", 
            "ORD_UNPR": "0",
            "QTY_ALL_ORD_YN": "Y",
            "EXCG_ID_DVSN_CD": exchange
        });

        let resp = self.client.post(&url)
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .header("appkey", &self.config.my_app)
            .header("appsecret", &self.config.my_sec)
            .header("tr_id", tr_id)
            .header("custtype", "P")
            .json(&body)
            .send()?;

        if resp.status().is_success() {
             let text = resp.text().unwrap_or_default();
             let data: Value = serde_json::from_str(&text)?;
             if data["rt_cd"].as_str().unwrap_or("") == "0" {
                 info!("Order Cancelled: {}", order_id);
                 Ok(true)
             } else {
                 let msg = data["msg1"].as_str().unwrap_or("Unknown error");
                 error!("Cancel failed API error: {}", msg);
                 Err(anyhow!("Cancel failed: {}", msg))
             }
        } else {
             let text = resp.text().unwrap_or_default();
             error!("Cancel Request failed: {}", text);
             Ok(false)
        }
    }

    fn get_order_book_snapshot(&self, symbol: &str) -> Result<OrderBook> {
        let token = self.get_token()?;
        let url = format!("{}/uapi/domestic-stock/v1/quotations/inquire-asking-price-exp-ccn", self.config.prod);
        
        let tr_id = "FHKST01010200";
        
        let params = [
            ("FID_COND_MRKT_DIV_CODE", "J"),
            ("FID_INPUT_ISCD", symbol)
        ];

        let resp = self.client.get(&url)
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .header("appkey", &self.config.my_app)
            .header("appsecret", &self.config.my_sec)
            .header("tr_id", tr_id)
            .query(&params)
            .send()?;

        if !resp.status().is_success() {
             return Err(anyhow!("Snapshot failed: {}", resp.status()));
        }

        let data: Value = resp.json()?;
        if data["rt_cd"].as_str().unwrap_or("") != "0" {
             return Err(anyhow!("API Error: {}", data["msg1"].as_str().unwrap_or("")));
        }
        
        let mut ob = OrderBook::new(symbol.to_string());
        if let Some(out1) = data["output1"].as_object() {
            fn get_dec(obj: &serde_json::Map<String, Value>, key: &str) -> Decimal {
                obj.get(key)
                   .and_then(|v| v.as_str())
                   .and_then(|s| Decimal::from_str(s).ok())
                   .unwrap_or_default()
            }
            fn get_qty(obj: &serde_json::Map<String, Value>, key: &str) -> i64 {
                obj.get(key)
                   .and_then(|v| v.as_str())
                   .and_then(|s| s.parse().ok())
                   .unwrap_or(0)
            }

            for i in 1..=10 {
                let ap = get_dec(out1, &format!("askp{}", i));
                let aq = get_qty(out1, &format!("askp_rsqn{}", i));
                if ap > Decimal::ZERO { ob.asks.insert(ap, aq); }

                let bp = get_dec(out1, &format!("bidp{}", i));
                let bq = get_qty(out1, &format!("bidp_rsqn{}", i));
                if bp > Decimal::ZERO { ob.bids.insert(bp, bq); }
            }
        }
        ob.timestamp = Local::now().timestamp_millis() as f64 / 1000.0;
        
        Ok(ob)
    }

    fn get_account_snapshot(&self, _account_id: &str) -> Result<AccountState> {
        let token = self.get_token()?;
        let url = format!("{}/uapi/domestic-stock/v1/trading/inquire-balance", self.config.prod);
        
        let cano_config = self.config.my_acct.as_deref().unwrap_or("");
        let prdt_config = self.config.my_prod.as_deref().unwrap_or("01");
        
        let (cano, prdt) = if _account_id.len() >= 10 {
             (&_account_id[0..8], &_account_id[8..])
        } else {
             (cano_config, prdt_config)
        };
        
        let is_virtual = self.config.prod.contains("openapivts");
        let tr_id = if is_virtual { "VTTC8434R" } else { "TTTC8434R" };

        let params = [
            ("CANO", cano),
            ("ACNT_PRDT_CD", prdt),
            ("AFHR_FLPR_YN", "N"),
            ("OFL_YN", ""),
            ("INQR_DVSN", "02"),
            ("UNPR_DVSN", "01"),
            ("FUND_STTL_ICLD_YN", "N"),
            ("FNCG_AMT_AUTO_RDPT_YN", "N"),
            ("PRCS_DVSN", "00"),
            ("CTX_AREA_FK100", ""),
            ("CTX_AREA_NK100", "")
        ];

        let resp = self.client.get(&url)
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .header("appkey", &self.config.my_app)
            .header("appsecret", &self.config.my_sec)
            .header("tr_id", tr_id)
            .query(&params)
            .send()?;

        if !resp.status().is_success() {
             let text = resp.text().unwrap_or_default();
             return Err(anyhow!("Account snapshot failed: {}", text));
        }

        let data: Value = resp.json()?;
        if data["rt_cd"].as_str().unwrap_or("") != "0" {
             let msg = data["msg1"].as_str().unwrap_or("Unknown error");
             return Err(anyhow!("API Error: {}", msg));
        }

        let mut acct = AccountState::new();
        
        if let Some(output2) = data.get("output2").and_then(|v| v.as_array()).and_then(|a| a.first()) {
            if let Some(deposit_str) = output2["dnca_tot_amt"].as_str() {
                if let Ok(bal) = Decimal::from_str(deposit_str) {
                    acct.balance = bal;
                }
            }
        }
        
        use crate::oms::account::Position;
        if let Some(output1) = data.get("output1").and_then(|v| v.as_array()) {
            for item in output1 {
                let symbol = item["pdno"].as_str().unwrap_or_default().to_string();
                let qty_str = item["hldg_qty"].as_str().unwrap_or("0");
                let price_str = item["pchs_avg_pric"].as_str().unwrap_or("0");
                let curr_str = item["prpr"].as_str().unwrap_or("0");
                
                let qty = qty_str.parse::<i64>().unwrap_or(0);
                let price = Decimal::from_str(price_str).unwrap_or_default();
                let curr = Decimal::from_str(curr_str).unwrap_or_default();
                
                if qty != 0 {
                    acct.positions.insert(
                        symbol.clone(),
                        Position::new(symbol, qty, price, curr)
                    );
                }
            }
        }
        Ok(acct)
    }

    fn modify_order(&self, order_id: &str, price: Option<Decimal>, qty: Option<i64>) -> Result<bool> {
        let token = self.get_token()?;
        let url = format!("{}/uapi/domestic-stock/v1/trading/order-rvsecncl", self.config.prod);

        let (org_no, order_no, exchange) = {
            let map = self.order_map.lock().unwrap();
            match map.get(order_id) {
                Some(info) => (info.org_no.clone(), info.order_no.clone(), info.exchange.clone()),
                None => {
                    return Err(anyhow!("Order ID not found in local map: {}", order_id));
                }
            }
        };

        let is_virtual = self.config.prod.contains("openapivts");
        let tr_id = if is_virtual { "VTTC0013U" } else { "TTTC0013U" };

        let cano = self.config.my_acct.as_deref().unwrap_or("");
        let prdt = self.config.my_prod.as_deref().unwrap_or("01");

        let price_str = price.map(|p| p.to_string()).unwrap_or("0".to_string());
        let qty_str = qty.map(|q| q.to_string()).unwrap_or("0".to_string());
        
        let qty_all_ord_yn = if qty.unwrap_or(0) == 0 { "Y" } else { "N" };
        let ord_dvsn = if price.is_some() { "00" } else { "01" };

        let body = serde_json::json!({
            "CANO": cano,
            "ACNT_PRDT_CD": prdt,
            "KRX_FWDG_ORD_ORGNO": org_no,
            "ORGN_ODNO": order_no,
            "ORD_DVSN": ord_dvsn,
            "RVSE_CNCL_DVSN_CD": "01", // Modify
            "ORD_QTY": qty_str,
            "ORD_UNPR": price_str,
            "QTY_ALL_ORD_YN": qty_all_ord_yn, 
            "EXCG_ID_DVSN_CD": exchange,
            "CNDT_PRIC": ""
        });

        let resp = self.client.post(&url)
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .header("appkey", &self.config.my_app)
            .header("appsecret", &self.config.my_sec)
            .header("tr_id", tr_id)
            .header("custtype", "P")
            .json(&body)
            .send()?;

        if resp.status().is_success() {
             let text = resp.text().unwrap_or_default();
             let data: Value = serde_json::from_str(&text)?;
             if data["rt_cd"].as_str().unwrap_or("") == "0" {
                 info!("Order Modified: {}", order_id);
                 
                 if let Some(output) = data.get("output") {
                     let new_order_no = output["ODNO"].as_str().unwrap_or("");
                     let new_org_no = output["KRX_FWDG_ORD_ORGNO"].as_str().unwrap_or("");
                     if !new_order_no.is_empty() && !new_org_no.is_empty() {
                         let mut map = self.order_map.lock().unwrap();
                         if let Some(info) = map.get_mut(order_id) {
                             info.order_no = new_order_no.to_string();
                             info.org_no = new_org_no.to_string();
                         }
                     }
                 }
                 
                 Ok(true)
             } else {
                 let msg = data["msg1"].as_str().unwrap_or("Unknown error");
                 error!("Modify failed API error: {}", msg);
                 Ok(false)
             }
        } else {
             let text = resp.text().unwrap_or_default();
             error!("Modify Request failed: {}", text);
             Ok(false)
        }
    }
}
