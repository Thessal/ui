import sys
import os
import time
import logging

# Ensure we can import didius
# Assuming running from project root or examples folder
import didius

def main():
    logging.basicConfig(level=logging.INFO)
    print("Initializing OMS Engine with HantooAdapter...")

    # Load Adapter
    config_path = "./auth/hantoo.yaml"
    
    # Python Adapter Wrapper
    # depending on how HantooAdapter is exposed in Python. 
    # Usually `didius.HantooAdapter` directly if exposed in lib.rs?
    # I need to check `src/lib.rs` to see what is exposed.
    # Assuming `didius.HantooAdapter` based on typical PyO3.
    
    try:
        adapter = didius.HantooAdapter(config_path)
    except Exception as e:
        print(f"Failed to create adapter: {e}")
        # Try finding the class
        print(dir(didius))
        return

    # Create Interface (OMSEngine Wrapper)
    oms = didius.OMSEngine(adapter)  # Interface class is exposed as OMSEngine in PyClass name=OMSEngine

    print("Starting OMS...")
    # Checking account_id is optional but good for internal init
    # We can fetch account from adapter config or just start
    oms.start()
    
    print("\n--- 1. Status Check ---")
    status = oms.get_oms_status()
    print(f"Status: {status}")

    print("\n--- 2. Balance Check (Internal) ---")
    # Should be empty initially
    acct = oms.get_balance()
    print(f"Internal Balance: {acct}")
    
    print("\n--- 3. Balance Check (API) ---")
    # This triggers API call
    # We need the account no from config, or pass empty string if adapter handles it?
    # Adapter uses config if short/empty.
    # Let's pass a dummy or 10-digit if we knew it.
    try:
        acct_api = oms.get_balance_api("") # Empty -> uses config
        print(f"API Balance: {acct_api}")
        print(f"Updated Internal Balance: {oms.get_balance()}")
    except Exception as e:
        print(f"Balance Check Failed: {e}")

    print("\n--- 4. Place Order ---")
    # Symbol: Samsung Electronics (005930)
    symbol = "005930" 
    
    # Price: We need a reasonable price or string.
    # Assuming "50000" KRW approx.
    # If this is Real/Paper? HantooAdapter usually requires valid grid.
    # We will use a low price to avoid fill if possible, or limit.
    price = "50000" 
    qty = 1
    
    order = didius.Order(
        symbol=symbol,
        side=didius.OrderSide.BUY,
        order_type=didius.OrderType.LIMIT,
        quantity=qty,
        price=price # String now
    )
    
    print(f"Sending Order: {order}")
    try:
        order_id = oms.place_order(order)
        print(f"Order Sent. ID: {order_id}")
    except Exception as e:
        print(f"Place Order Failed: {e}")
        return

    print("\n--- 5. Monitoring Order ---")
    for i in range(5):
        time.sleep(1)
        orders = oms.get_orders()
        if order_id in orders:
            o = orders[order_id]
            print(f"[{i}] Order State: {o.state}")
            if o.state == didius.OrderState.NEW:
                print("Order is Active on Exchange!")
                break
            elif o.state == didius.OrderState.REJECTED:
                print(f"Order Rejected: {o.error_message}")
                break
        else:
            print("Order not tracked yet...")
            
    print("\n--- 6. Cancel Order ---")
    try:
        oms.cancel_order(order_id)
        print("Cancel Request Sent.")
    except Exception as e:
        print(f"Cancel Failed: {e}")

    time.sleep(2)
    orders = oms.get_orders()
    if order_id in orders:
        final_state = orders[order_id].state
        print(f"Final State: {final_state}")
    
    print("\n--- End Verification ---")
    oms.stop()

if __name__ == "__main__":
    main()
