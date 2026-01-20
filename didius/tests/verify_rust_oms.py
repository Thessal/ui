
import sys
import os
import time
import json
import shutil

# Add target/release to path to import generated module
sys.path.append("target/release")

if "didius_oms" in sys.modules:
    del sys.modules["didius_oms"]
    
try:
    import didius_oms 
except ImportError:
     print("Direct import failed.")
     sys.exit(1)

print("Module loaded successfully!")
print(f"Module file: {didius_oms.__file__}")


# Mock Adapter
class MockAdapter:
    def connect(self):
        print("Mock: Connected")
    def disconnect(self):
        print("Mock: Disconnected")
    def place_order(self, order):
        print(f"Mock: Place Order {order.symbol} {order.quantity}")
        return True
    def get_order_book_snapshot(self, symbol):
        print(f"Mock: Get Snapshot {symbol}")
        # Return dict as expected by Rust now
        return {
            "symbol": symbol,
            "bids": {},
            "asks": {},
            "last_update_id": 0,
            "timestamp": 0.0
        }
    def get_account_snapshot(self, account_id):
        print(f"Mock: Get Account Snapshot {account_id}")
        return {
            "balance": 10000.0,
            "locked": 0.0,
            "positions": {}
        }

def test_interface():
    print("Testing Interface (IPC)...")
    adapter = MockAdapter()
    
    # Instantiate Interface
    # Note: OMSEngine is hidden now.
    interface = didius_oms.Interface(adapter)
    
    interface.start(None)
    
    # Place Order
    order = didius_oms.Order(
        symbol="AAPL",
        side=didius_oms.OrderSide.BUY,
        order_type=didius_oms.OrderType.LIMIT,
        quantity=10,
        price=150.0,
        strategy=didius_oms.ExecutionStrategy.NONE,
        strategy_params=None,
        stop_price=None
    )
    
    oid = interface.place_order(order)
    print(f"Order Placed ID: {oid}")
    
    # Init Symbol
    interface.init_symbol("AAPL")
    
    # Get OrderBook (Dict)
    book = interface.get_order_book("AAPL")
    print(f"OrderBook: {book}")
    assert book["symbol"] == "AAPL"
    assert "bids" in book
    
    # Get Account (Dict)
    acc = interface.get_account()
    print(f"Account: {acc}")
    assert acc["balance"] == 0.0 # Default was 0.0 in constructor, did we init?
    # Interface start called init_account if id provided. We passed None.
    # So it should be 0.0 unless we mock init.
    
    interface.stop()
    print("Interface Tests OK")

if __name__ == "__main__":
    test_interface()
    print("\nALL TESTS PASSED")
