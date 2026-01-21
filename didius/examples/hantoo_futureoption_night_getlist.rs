use didius::adapter::hantoo_futureoption_night::HantooNightAdapter;
use anyhow::Result;

fn main() -> Result<()> {
    // 1. Initialize Adapter
    // Using the same config as other Hantoo examples
    let adapter = HantooNightAdapter::new("auth/hantoo.yaml")?;
    
    // 2. Get Night Future List
    println!("--- Fetching Night Future List ---");
    match adapter.get_night_future_list() {
        Ok(list) => {
            println!("Received {} items.", list.len());
            if !list.is_empty() {
                println!("First 5 items:");
                for item in list.iter().take(5) {
                    println!("{:?}", item);
                }
            }
        },
        Err(e) => eprintln!("Error fetching future list: {}", e),
    }

    // 3. Get Night Option List
    println!("\n--- Fetching Night Option List ---");
    match adapter.get_night_option_list() {
        Ok(list) => {
            println!("Received {} items.", list.len());
            if !list.is_empty() {
                println!("First 5 items:");
                for item in list.iter().take(5) {
                    println!("{:?}", item);
                }
            }
        },
        Err(e) => eprintln!("Error fetching option list: {}", e),
    }

    Ok(())
}
