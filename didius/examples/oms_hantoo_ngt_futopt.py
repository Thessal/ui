import time
import sys
import datetime

# Assuming compiled module is available as didius
# Note: You must build the module using `maturin develop` or `cargo build --release` (plus moving .so) for this to import.
try:
    import didius
except ImportError:
    print("Error: didius module not found. Please build the rust extension (e.g., using `maturin develop`).")
    sys.exit(1)

def main():
    print("Initializing HantooNightAdapter...")
    try:
        # Create Adapter (Rust implementation wrapped in Python class)
        adapter = didius.HantooNightAdapter("auth/hantoo.yaml")
    except Exception as e:
        print(f"Failed to initialize adapter: {e}")
        return

    print("Initializing OMSEngine with S3 Logger...")
    # Initialize Engine with S3 logging configuration
    engine = didius.OMSEngine(
        adapter=adapter, 
        s3_bucket="didius", 
        s3_region="ap-northeast-2", 
        s3_prefix="logs"
    )

    print("Fetching Night Future list...")
    try:
        # Calls the exposed get_night_future_list function from adapter
        futures = [x["futs_shrn_iscd"] for x in adapter.get_night_future_list()]
        
        if not futures:
            print("Future list is empty. Using fallback.")
            # Fallback symbol if list is empty (e.g. 101TC000 is a common contract code format, but let's just pick a likely one or wait for list)
            # Actually, user said "subscribe first item", so if empty we might have issues.
            # Let's try to get options if futures fail, or just provide a dummy string if both fail, 
            # though usually tests might expect something specific.
            # Let's just assume we need at least one.
            symbols = []
        else:
            print(f"Successfully fetched {len(futures)} Night Future symbols.")
            print(f"First 5: {futures[:5]}")
            symbols = futures

    except Exception as e:
        print(f"Failed to fetch Night Future list: {e}.")
        symbols = []

    if not symbols:
         print("No symbols found. Cannot subscribe.")
         return

    print("Connecting...")
    adapter.set_debug_mode(False)
    adapter.connect()

    print("Wiring Gateway...")
    # Wire the internal Rust channels between Adapter and Engine
    # This enables the engine to process incoming WebSocket messages
    # Must be done BEFORE subscription because HantooNightAdapter starts the WS thread immediately upon subscription.
    engine.start_gateway(adapter)

    target_symbol = symbols[0]
    print(f"Subscribing to {target_symbol}...")
    # Note: HantooNightAdapter.subscribe takes a single string, unlike stock adapter
    adapter.subscribe(target_symbol)
    
    print("Waiting 5s for initial data...")
    time.sleep(5)
    
    print("Starting long-running loop. Logs will be flushed to S3 every 60s.")
    print("Press Ctrl-C to stop.")
    
    try:
        while True:
            time.sleep(10)
            now = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")
            print(f"[{now}] OMSEngine Running... (Subscribed: {target_symbol})")
            
            # Sample book display
            # get_order_book returns a dictionary
            book = engine.get_order_book(target_symbol)
            if book:
                bids = book.get("bids", {})
                asks = book.get("asks", {})
                
                # Calculate best bid/ask for display
                # Note: The structure of bids/asks in OMS book is {price: qty}
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
                print(f"  Sample Book [{target_symbol}]: Best Bid {best_bid_p} / Best Ask {best_ask_p} (UpdateID: {update_id})")
            else:
                # If no book yet, print simple status
                print(f"  Sample Book [{target_symbol}]: No data yet.")

    except KeyboardInterrupt:
        print("\nStopping...")
        pass

if __name__ == "__main__":
    main()
