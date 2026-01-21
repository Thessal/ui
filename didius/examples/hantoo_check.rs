use didius::adapter::hantoo::HantooAdapter;
use didius::adapter::Adapter;
use std::env;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

fn main() {
    // Setup logger
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let auth_dir = Path::new("auth");
    if !auth_dir.exists() {
        fs::create_dir(auth_dir).unwrap();
    }
    
    // We expect hantoo.yaml possibly to not have my_htsid if user hasn't set it.
    // If testing with dummy, we add it.
    let config_path = "auth/hantoo.yaml";
    if !Path::new(config_path).exists() {
        println!("Creating dummy config for testing...");
        let dummy_config = r#"
my_app: "dummy_app_key"
my_sec: "dummy_secret"
prod: "https://openapivts.koreainvestment.com:29443"
my_acct: "12345678"
my_prod: "01"
my_htsid: "testuser"
ops: "ws://ops.koreainvestment.com:21000"
"#;
        fs::write(config_path, dummy_config).unwrap();
    }

    println!("Initializing HantooAdapter...");
    match HantooAdapter::new(config_path) {
        Ok(adapter) => {
            println!("Adapter initialized successfully.");
            
            // Connect (Token + WS)
            println!("Attempting to connect...");
            match adapter.connect() {
                Ok(_) => {
                    println!("Connected successfully.");
                    // Check WS thread is running? We can't easily check internal state without exposing it.
                    // But we can sleep and see if logs appear.
                    
                    println!("Sleeping 5 sec to allow WS connection attempt logging...");
                    thread::sleep(Duration::from_secs(5));
                    

                    println!("Requesting Account Snapshot...");
                    
                    // Read config to get account ID
                    let config_content = fs::read_to_string(config_path).unwrap_or_default();
                    // Simple parse using yaml would be better, but we don't have serde_yaml in scope here unless valid.
                    // Actually we do because deps are shared.
                    // But to avoid complex structs here, let's just parse logic or use serde_json Value from serde_yaml?
                    // We can use serde_yaml::Value.
                    
                    let mut real_acct = "12345678".to_string();
                    let mut real_prod = "01".to_string();
                    
                    if let Ok(value) = serde_yaml::from_str::<serde_json::Value>(&config_content) {
                         if let Some(acct) = value.get("my_acct_stock").or(value.get("my_acct")).and_then(|v| v.as_str()) {
                             real_acct = acct.to_string();
                         }
                         if let Some(prod) = value.get("my_prod").and_then(|v| v.as_str()) {
                             real_prod = prod.to_string();
                         }
                    }
                    
                    let full_acct = format!("{}{}", real_acct, real_prod);
                    println!("Using account: {}", full_acct);

                    match adapter.get_account_snapshot(&full_acct) {
                         Ok(acct) => println!("Account Snapshot: {:?}", acct),
                         Err(e) => println!("Account Snapshot failed (expected if dummy creds): {}", e),
                    }
                    
                },
                Err(e) => println!("Connection failed: {}", e),
            }
        },
        Err(e) => eprintln!("Failed to initialize adapter: {}", e),
    }
}
