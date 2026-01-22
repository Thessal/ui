import sys
import os
import time
import logging
import select
import termios
import tty
from decimal import Decimal

# Ensure we can import didius
import didius

def get_input_with_timeout(prompt, timeout):
    sys.stdout.write(prompt)
    sys.stdout.flush()
    ready, _, _ = select.select([sys.stdin], [], [], timeout)
    if ready:
        return sys.stdin.readline().strip()
    return None

def main():
    # Setup Logger
    logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
    
    print("Initializing OMS Engine with HantooAdapter...")
    config_path = "./auth/hantoo.yaml"
    
    # 1. Initialize Adapter & Engine
    try:
        adapter = didius.HantooAdapter(config_path)
        adapter.set_debug_mode(False) # Enable Debug per user request
    except Exception as e:
        print(f"Failed to create adapter: {e}")
        return

    # S3 Logger
    omse = didius.OMSEngine(adapter, "didius", "ap-northeast-2", "logs") 
    oms = omse # Alias
    # 2. Ask user input FIRST (Before Connect)
    symbol = input("Enter Symbol to Monitor (e.g. 005930, 001360): ").strip()
    if not symbol:
        symbol = "005930" # Default Samsung
    
    ticksize = input("Enter ticksize: ").strip()
    if not ticksize:
        ticksize = "100" 
    ticksize = int(ticksize)
    assert ticksize > 0

    print(f"Subscribing to {symbol}...")
    try:
        adapter.subscribe_market([symbol])
    except Exception as e:
        print(f"Subscription failed: {e}")
        
    print(f"Starting Monitor Loop for {symbol}. Interval: 10s.")

    # Start Gateway Listener
    oms.start_gateway(adapter)
    
    # Start Engine (Connects WS)
    oms.start()
    
    print("Waiting 3s for connection...")
    time.sleep(3)
    
    # Initialize Account (Fetch from API)
    print("Initializing Account Data...")
    try:
        # Pass empty string or account ID if known. Adapter usually handles empty string if config has one.
        oms.get_balance_api("") 
    except Exception as e:
        print(f"Account Init Failed: {e}")
    
    loop_count = 0
    while True:
        loop_count += 1
        print(f"\n--- Loop {loop_count} ---")
        
        # 4. Print Info
        balance = oms.get_balance()
        # Extract won balance and symbol quantity.
        # Structure: {'balance': '...', 'locked': '...', 'positions': {'005930': {'quantity': ...}}}
        total_balance = balance.get('balance', '0')
        positions = balance.get('positions', {})
        pos_info = positions.get(symbol, {})
        pos_qty = pos_info.get('quantity', 0)
        
        print(f"Balance: {total_balance} KRW | Position {symbol}: {pos_qty}")
        
        # Get Book
        book = oms.get_order_book(symbol)
        bp = 0
        ap = 0
        if book:
            bids = book.get('bids', {})
            asks = book.get('asks', {})
            # Get best bid/ask
            # bids is dict {price: qty}. We need max price for bid, min for ask.
            if bids:
                bp = max([float(p) for p in bids.keys()])
            if asks:
                ap = min([float(p) for p in asks.keys()])
            
            print(f"Quote {symbol}: Bid {bp} / Ask {ap}")
        else:
            print(f"Quote {symbol}: No Book Data")
            
        if bp == 0 or ap == 0:
            print("Waiting for market data...")
            # If no price, we can't really trade intelligently, but we allow 'Do nothing' default
            # We skip trading logic if no price, to serve safe defaults?
            # Or assume user enters limit manually? 
            # Logic below depends on bp/ap.
        
        # 5. User Action
        print("Actions: [1] Buy  [2] Sell [3] Buy! [4] Sell! [5] Do Nothing")
        
        action = get_input_with_timeout("Select Action > ", 10)
        
        if not action:
            print("\nTimeout. Default: Do Nothing.")
            action = "5"
        
        qty = 1
        price = 0
        side = didius.OrderSide.BUY
        
        # Valid Prices
        # Note: If book is empty, bp/ap are 0.
        valid_quote = (bp > 0 and ap > 0)
        
        execute = False
        
        if action == "1": # Buy @ Min(bp, ap) - passive/makerish? Or taker?
            # "Buy @ Min(bp, ap)" -> Likely Min of BestBid and BestAsk? 
            # Usually meant to be passive? If Min(bp, ap) == bp (if spread exists).
            if valid_quote:
                price = min(bp, ap)
                side = didius.OrderSide.BUY
                execute = True
        elif action == "2": # Sell @ Max(bp, ap)
            if valid_quote:
                price = max(bp, ap)
                side = didius.OrderSide.SELL
                execute = True
        elif action == "3": # Aggressive Buy
            if valid_quote:
                price = max(bp, ap) + ticksize
                side = didius.OrderSide.BUY
                execute = True
        elif action == "4": # Aggressive Sell
            if valid_quote:
                price = min(bp, ap) - ticksize
                side = didius.OrderSide.SELL
                execute = True
        elif action == "5":
            print("Doing Nothing.")
        else:
            print("Invalid Input. Doing Nothing.")
            
        # 6. Execute in next loop (conceptually "beginning of next loop" or just now?)
        # "Execute the order in the beginning of next loop" -> implies wait then execute?
        # Typically one acts on current data. I will execute immediately for responsiveness 
        # unless strict interpretation of "beginning of next" is required. 
        # "beginning of next" implies state might change.
        # But prompts says "If there's action, execute the order in the beginning of next loop."
        # I will simpler execution: Determine action now, Sleep (already implicitly done by timeout?), 
        # actually the loop interval is "every 10 seconds". 
        # Since input waits up to 10s, if user inputs quickly (e.g. 1s), we have 9s left?
        # Or does the loop start, wait 10s for input, then loop again? 
        # If user Inputs, we process, then arguably we should wait the remainder or just loop.
        # I will execute immediately then continue loop.
        
        if execute:
            if price <= 0:
                 print("Invalid Price (0). Skipping.")
            else:
                 print(f"Executing {side} {qty} @ {price}")
                 order = didius.Order(
                    symbol=symbol,
                    side=side,
                    order_type=didius.OrderType.LIMIT,
                    quantity=qty,
                    price=str(int(price)) 
                 )
                 try:
                     oid = oms.place_order(order)
                     print(f"Order Sent: {oid}")
                 except Exception as e:
                     print(f"Order Failed: {e}")

    oms.stop()

if __name__ == "__main__":
    main()
