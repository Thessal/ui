import time
import sys
import datetime

# Assuming compiled module is available as didius
# Note: You must build the module using `maturin develop` or `cargo build --release` (plus moving .so) for this to import.
try:
    import didius
    from didius import utils
except ImportError:
    print("Error: didius module not found. Please build the rust extension (e.g., using `maturin develop`).")
    sys.exit(1)

def main():
    print("Initializing HantooAdapter...")
    try:
        # Create Adapter (Rust implementation wrapped in Python class)
        adapter = didius.HantooAdapter("auth/hantoo.yaml")
    except Exception as e:
        print(f"Failed to initialize adapter: {e}")
        return

    print("Initializing OMSEngine with S3 Logger...")
    # Initialize Engine with S3 logging configuration
    # This matches the modified rust/examples/oms_hantoo_stock.rs
    engine = didius.OMSEngine(
        adapter=adapter, 
        s3_bucket="didius", 
        s3_region="ap-northeast-2", 
        s3_prefix="logs"
    )

    print("Attempting to download KOSPI50 constituents...")
    try:
        # Calls the exposed download_kospi_50 function from utils module
        symbols = utils.download_kospi_50()
        if not symbols:
            print("Downloaded list is empty. Using fallback.")
            symbols = ["005930", "000660"]
        else:
            print(f"Successfully downloaded {len(symbols)} KOSPI50 constituents.")
            print(f"First 5: {symbols[:5]}")
    except Exception as e:
        print(f"Failed to download KOSPI50: {e}. Using fallback.")
        symbols = ["005930", "000660"]

    print(f"Subscribing to {len(symbols)} symbols...")
    adapter.subscribe_market(symbols)

    print("Connecting...")
    adapter.set_debug_mode(False)
    adapter.connect()

    print("Wiring Gateway...")
    # Wire the internal Rust channels between Adapter and Engine
    # This enables the engine to process incoming WebSocket messages
    engine.start_gateway(adapter)
    
    print("Waiting 5s for initial data...")
    time.sleep(5)
    
    print("Starting long-running loop. Logs will be flushed to S3 every 60s.")
    print("Press Ctrl-C to stop.")
    
    try:
        while True:
            time.sleep(10)
            now = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")
            print(f"[{now}] OMSEngine Running... (Subscribed: {len(symbols)})")
            
            # Optional: Sample book display
            if symbols:
                sample = symbols[0]
                # get_order_book returns a dictionary
                book = engine.get_order_book(sample)
                if book:
                    bids = book.get("bids", {})
                    asks = book.get("asks", {})
                    
                    # Calculate best bid/ask for display
                    best_bid_p = "N/A"
                    if bids:
                        try:
                            # Bids keys are strings, convert to float to find max
                            best_bid_p = str(max(float(p) for p in bids.keys()))
                        except ValueError: pass

                    best_ask_p = "N/A"
                    if asks:
                        try:
                            # Asks keys are strings, convert to float to find min
                            best_ask_p = str(min(float(p) for p in asks.keys()))
                        except ValueError: pass
                         
                    update_id = book.get("last_update_id", 0)
                    print(f"  Sample Book [{sample}]: Best Bid {best_bid_p} / Best Ask {best_ask_p} (UpdateID: {update_id})")
    except KeyboardInterrupt:
        print("\nStopping...")
        pass

if __name__ == "__main__":
    main()
