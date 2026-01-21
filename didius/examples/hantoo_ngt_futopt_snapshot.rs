use didius::adapter::hantoo_ngt_futopt::HantooNightAdapter;
use didius::adapter::Adapter;
use anyhow::Result;

fn main() -> Result<()> {
    // 1. Initialize Adapter
    println!("Initializing HantooNightAdapter...");
    let adapter = HantooNightAdapter::new("auth/hantoo.yaml")?;
    
    // 2. Get Night Future List to pick a valid symbol
    println!("Fetching Night Future List...");
    let list = adapter.get_night_future_list()?;
    
    if list.is_empty() {
        println!("No night futures available.");
        return Ok(());
    }
    
    // 3. Select first symbol
    let first_item = &list[0];
    let symbol = first_item["futs_shrn_iscd"].as_str().unwrap_or("").to_string();
    let name = first_item["hts_kor_isnm"].as_str().unwrap_or("Unknown");
    
    if symbol.is_empty() {
        println!("Invalid symbol in list.");
        return Ok(());
    }
    
    println!("Selected Symbol: {} ({})", symbol, name);
    
    // 4. Request Snapshot
    println!("Requesting OrderBook Snapshot (FHMIF10010000)...");
    let book = adapter.get_order_book_snapshot(&symbol)?;
    
    println!("\n--- Snapshot Result ---");
    println!("Symbol: {}", book.symbol);
    println!("Timestamp: {}", book.timestamp);
    println!("Bids: {} levels", book.bids.len());
    println!("Asks: {} levels", book.asks.len());
    
    println!("\nTop Bids:");
    // BTreeMap is sorted by key (Price). Iterate rev for highest bid.
    for (price, qty) in book.bids.iter().rev().take(5) {
        println!("  Price: {}, Qty: {}", price, qty);
    }
    
    println!("\nTop Asks:");
    // Iterate forward for lowest ask.
    for (price, qty) in book.asks.iter().take(5) {
        println!("  Price: {}, Qty: {}", price, qty);
    }
    
    Ok(())
}
