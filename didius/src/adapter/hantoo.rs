use crate::adapter::Adapter;
use crate::oms::account::AccountState;
use crate::oms::order::{Order, OrderSide, OrderType};
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
use std::thread;
use std::collections::HashMap;
use tungstenite::{connect, Message};
use url::Url;

#[derive(Debug, Deserialize, Clone)]
pub struct HantooConfig {
    pub my_app: String,
    pub my_sec: String,
    pub prod: String, // Base URL
    #[serde(alias = "my_acct_stock")]
    pub my_acct: Option<String>,
    pub my_prod: Option<String>,
    pub my_htsid: Option<String>,
    pub ops: Option<String>, // WebSocket URL (e.g., ws://ops.koreainvestment.com:21000)
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
    order_map: Mutex<HashMap<String, HantooOrderInfo>>,
}

#[derive(Debug, Clone)]
struct HantooOrderInfo {
    org_no: String,
    order_no: String,
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

            ws_thread: Mutex::new(None), // Thread handle
            order_map: Mutex::new(HashMap::new()),
        };

        Ok(adapter)
    }

    fn get_token(&self) -> Result<String> {
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

    fn get_ws_approval_key(&self) -> Result<String> {
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

    fn start_ws_thread(&self) -> Result<()> {
        let ws_url_str = self.config.ops.clone().ok_or(anyhow!("No WebSocket URL (ops) in config"))?;
        let approval_key = self.get_ws_approval_key()?;
        let my_htsid = self.config.my_htsid.clone().unwrap_or_default();
        
        // We clone config values needed for the thread.
        // In a real app, we might need a channel to communicate back to the Adapter or Engine.
        // For now, we will just print messages or log them, as the Adapter trait doesn't strictly 
        // mandate a callback mechanism in its signature yet (it is sync).
        // To support callbacks, we'd need to change Adapter to take a callback or generic.
        // Assuming strict adherence to current Adapter trait, we just run the loop.
        
        let handle = thread::spawn(move || {
            let full_url = format!("{}/tryitout/H0STCNT0", ws_url_str); // Typical suffix
            let url = Url::parse(&full_url).expect("Invalid WS URL");

            info!("Connecting to WebSocket: {}", url);
            match connect(url) {
                Ok((mut socket, response)) => {
                    info!("WebSocket Connected. Response: {:?}", response);

                    // Subscribe to Account Execution (H0STCNI0)
                    if !my_htsid.is_empty() {
                         let tr_id = if ws_url_str.contains("openapivts") { "H0STCNI9" } else { "H0STCNI0" };
                         let sub_body = serde_json::json!({
                            "header": {
                                "approval_key": approval_key,
                                "custtype": "P",
                                "tr_type": "1",
                                "content-type": "utf-8"
                            },
                            "body": {
                                "input": {
                                    "tr_id": tr_id,
                                    "tr_key": my_htsid
                                }
                            }
                         });
                         if let Err(e) = socket.write_message(Message::Text(sub_body.to_string())) {
                             error!("Failed to subscribe to execution: {}", e);
                         } else {
                             info!("Subscribed to Execution Notice ({}) for {}", tr_id, my_htsid);
                         }
                    }

                    // Loop
                    loop {
                        match socket.read_message() {
                            Ok(msg) => {
                                match msg {
                                    Message::Text(text) => {
                                        // Handle PingPong
                                        // Format: 0|TR_ID|... or JSON
                                        if text.contains("PINGPONG") {
                                            let _ = socket.write_message(Message::Text(text)); // Echo
                                            continue;
                                        }
                                        
                                        // Parse Data
                                        // TODO: Callback logic
                                        // info!("WS Recv: {}", text); 
                                        
                                        if let Some(first_char) = text.chars().next() {
                                            if first_char == '0' || first_char == '1' {
                                                // Real data
                                                // Split by |
                                                let parts: Vec<&str> = text.split('|').collect();
                                                if parts.len() >= 4 {
                                                    let tr_id = parts[1];
                                                    // H0UNASP0 = Asking Price, H0STCNI0 = Execution
                                                    match tr_id {
                                                        "H0UNASP0" => {
                                                            // Handle Asking Price
                                                        },
                                                        "H0STCNI0" | "H0STCNI9" => {
                                                            info!("Execution Update: {}", text);
                                                        },
                                                        _ => {}
                                                    }
                                                }
                                            }
                                        }
                                    },
                                    Message::Close(_) => {
                                        info!("WS Closed");
                                        break;
                                    },
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
                Err(e) => {
                    error!("WebSocket connection failed: {}", e);
                }
            }
        });

        let mut thread_guard = self.ws_thread.lock().unwrap();
        *thread_guard = Some(handle);

        Ok(())
    }
}

impl Adapter for HantooAdapter {
    fn connect(&self) -> Result<()> {
        let _ = self.get_token()?;
        info!("HantooAdapter connected (token verified)");
        
        // Start WS
        if let Err(e) = self.start_ws_thread() {
            warn!("Failed to start WebSocket: {}", e);
        }

        Ok(())
    }

    fn disconnect(&self) -> Result<()> {
        info!("HantooAdapter disconnected");
        Ok(())
    }

    fn place_order(&self, order: &Order) -> Result<bool> {
        let token = self.get_token()?;
        let url = format!("{}/uapi/domestic-stock/v1/trading/order-cash", self.config.prod);
        
        // Updated TR_ID based on order_cash.py
        let is_virtual = self.config.prod.contains("openapivts");
        let tr_id = match order.side {
            // "TTTC0012U" (Buy), "TTTC0011U" (Sell)
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
            "EXCG_ID_DVSN_CD": "KRX", // Required by order_cash.py
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
             // Parse Body to get OrgNo and OrderNo
             // Response format: {"rt_cd": "0", "msg1": "...", "output": {"KRX_FWDG_ORD_ORGNO": "...", "ODNO": "...", "ORD_TMD": "..."}}
             
             // We can read text once
             let text = resp.text().unwrap_or_default();
             let data: Value = serde_json::from_str(&text).map_err(|e| anyhow!("Failed to parse response: {}", e))?;
             
             if let Some(output) = data.get("output") {
                 let org_no = output["KRX_FWDG_ORD_ORGNO"].as_str().unwrap_or("").to_string();
                 let order_no = output["ODNO"].as_str().unwrap_or("").to_string();
                 
                 if !org_no.is_empty() && !order_no.is_empty() {
                     info!("Order Placed: OrgNo={}, OrderNo={}", org_no, order_no);
                     
                     // Store in map
                     if let Some(client_id) = &order.order_id {
                          let info = HantooOrderInfo { org_no, order_no };
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
        
        // Find OrgNo and OrderNo
        let (org_no, order_no) = {
            let map = self.order_map.lock().unwrap();
            match map.get(order_id) {
                Some(info) => (info.org_no.clone(), info.order_no.clone()),
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
            "ORD_DVSN": "00", // Usually 00 for Cancel? Check docs. 02 is for Code? No, ORD_DVSN is Order Division (Limit/Market). For cancel, usually inherit or 00.
            "RVSE_CNCL_DVSN_CD": "02", // 01: Modify, 02: Cancel
            "ORD_QTY": "0", // 0 for Cancel All usually? Or specific quantity. The example uses "0" with QTY_ALL_ORD_YN="Y" often, or total qty. Let's assume Cancel All Y.
            "ORD_UNPR": "0",
            "QTY_ALL_ORD_YN": "Y",
            "EXCG_ID_DVSN_CD": "KRX"
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
        Ok(OrderBook::new(symbol.to_string()))
    }

    fn get_account_snapshot(&self, _account_id: &str) -> Result<AccountState> {
        let token = self.get_token()?;
        let url = format!("{}/uapi/domestic-stock/v1/trading/inquire-balance", self.config.prod);
        
        let cano_config = self.config.my_acct.as_deref().unwrap_or("");
        let prdt_config = self.config.my_prod.as_deref().unwrap_or("01");
        
        // If account_id is provided and valid (length 10), use it. Otherwise use config.
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

        // Logic to parse output1 (Holdings) and output2 (Balance) from inquire_balance.py structure
        let mut acct = AccountState::new();
        
        // Output2: Balance
        // dnca_tot_amt: Deposit
        if let Some(output2) = data.get("output2").and_then(|v| v.as_array()).and_then(|a| a.first()) {
            if let Some(deposit_str) = output2["dnca_tot_amt"].as_str() {
                if let Ok(bal) = deposit_str.parse::<f64>() {
                    acct.balance = bal;
                }
            }
        }
        
        // Output1: Positions
        use crate::oms::account::Position;
        if let Some(output1) = data.get("output1").and_then(|v| v.as_array()) {
            for item in output1 {
                let symbol = item["pdno"].as_str().unwrap_or_default().to_string();
                let qty_str = item["hldg_qty"].as_str().unwrap_or("0");
                let price_str = item["pchs_avg_pric"].as_str().unwrap_or("0");
                let curr_str = item["prpr"].as_str().unwrap_or("0");
                
                let qty = qty_str.parse::<i64>().unwrap_or(0);
                if qty > 0 {
                    let avg_price = price_str.parse::<f64>().unwrap_or(0.0);
                    let curr_price = curr_str.parse::<f64>().unwrap_or(0.0);
                    let pos = Position::new(symbol, qty, avg_price, curr_price);
                    acct.positions.insert(pos.symbol.clone(), pos);
                }
            }
        }

        Ok(acct)
    }
}
