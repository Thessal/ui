
import os
import sys
import numpy as np
import pandas as pd
from typing import List, Dict, Any

# Mock butterflow if not available
try:
    from .backtest import initialize_runtime, compute
    from butterflow import Runtime
except ImportError:
    print("Warning: butterflow not found. Using mock runtime.")
    class Runtime:
        def __init__(self, data, check_corruption): pass
        def run(self, graph): return np.zeros(100) # Mock result
    
    def initialize_runtime(dfs=None, add_logret=False, check_corruption=False):
        return Runtime(data={}, check_corruption=check_corruption)
    
    def compute(runtime, code, silent=True):
        return np.random.randn(100) # Mock signal

# Try importing hftbacktest
try:
    from hftbacktest import (
        BacktestAsset, 
        HashMapMarketDepthBacktest, 
        collect_locals,
        BUY, SELL, GTX, LIMIT
    )
    from numba import njit
except ImportError:
    print("Warning: hftbacktest not found. Please ensure it is installed.")
    # Define mocks for hftbacktest if needed for structure verification

import json

def run_backtest(strategy_file: str, data_path: str = None, output_path: str = None, json_path: str = None):
    print(f"Running backtest for: {strategy_file}")
    
    # 1. Load Strategy
    if not os.path.exists(strategy_file):
        raise FileNotFoundError(f"Strategy file not found: {strategy_file}")
    
    with open(strategy_file, 'r') as f:
        strategy_code = f.read()

    # 2. Initialize Runtime & Data
    try:
        runtime = initialize_runtime(add_logret=True)
    except Exception as e:
        print(f"Runtime initialization failed: {e}")
        return

    # 3. Compute Signal
    print("Computing signals using Butterflow...")
    # Let exception propagate to cli.py which prints traceback
    signal_result = compute(runtime, strategy_code, silent=True)
    
    print(f"Result Type: {type(signal_result)}")
    if isinstance(signal_result, dict):
        print(f"Result Keys: {list(signal_result.keys())}")
    else:
        print(f"Result Value: {signal_result}")


    if signal_result is None:
        print("No result from strategy.")
        return

    print("Signal computed.")
    # signal_result is likely a numpy array or vector corresponding to timestamps
    
    # 4. Integrate with HftBacktest -> Export signals
    
    # Calculate PnL, Position, Turnover using CloseBacktester
    try:
        from .backtest import Bars, CloseBacktester, Position
        import datetime
        
        # Create Bars from runtime data (accessing private _data for now as runtime structure is opaque/mocked)
        # In real butterflow, runtime.data should hold the data.
        # Here we assume runtime.data is a dict consistent with backtest.py expectations or we reconstruct it.
        # Since we initialized runtime with `initialize_runtime(add_logret=True)`, let's see what we have.
        # For simplicity in this fix, we will try to reconstruct a DataFrame from runtime.data if possible,
        # or load data again if needed.
        # Actually, `initialize_runtime` uses `np.load` in `backtest.py`.
        
        # Let's re-load data as DataFrame to create Bars
        # We need a proper DataFrame struct [ts, field, instr] -> but `backtest.py` implies [ts*field*instr] or similar.
        # Looking at `backtest.py`: initialize_runtime loads npy files.
        # Let's try to mock the data loading for Backtester since we don't have easy access to runtime's internal data in a structured way here.
        # OR better: use the `runtime.data` if accessible.
        
        # Re-loading data for Backtester consistency
        data_dir = "data"
        if os.path.isdir(data_dir):
            fields = ["open", "high", "low", "close", "volume"]
            dfs = {}
            # Assuming single asset or handling multiple. The mock/backtest.py seems to handle 1 set of npy as single asset or simple dict.
            # `backtest.py` `initialize_runtime` loads "open.npy" etc.
            # We need to construct a DataFrame that `Bars` accepts.
            # `Bars` expects `data` (DataFrame) and `interval`.
            # `Bars` data iteration expects (ts, field) index? No, `iterrows` on `data`.
            # `Bars` `__init__` asserts `data.index.nlevels == 2`.
            
            # Use `pd.DataFrame` construction from npy files
            # Assuming 100 points as per create_dummy_data.py
            # Timestamps: let's generate mock timestamps
            
            loaded_data = {f: np.load(os.path.join(data_dir, f"{f}.npy")) for f in fields}
            length = len(loaded_data["open"])
            
            # Create timestamp index
            start_time = pd.Timestamp("2023-01-01 09:00:00")
            timestamps = [start_time + datetime.timedelta(minutes=i) for i in range(length)]
            
            # Construct MultiIndex DataFrame: (timestamp, field) -> value?
            # `Bars` iteration: `for (ts, field), row in self.data.iterrows():`
            # This implies index is (ts, field).
            
            frames = []
            for f in fields:
                df = pd.DataFrame(loaded_data[f], index=timestamps, columns=["Asset1"]) # Assuming 1 asset "Asset1"
                df["field"] = f
                df = df.reset_index().set_index(["index", "field"])
                frames.append(df)
            
            full_df = pd.concat(frames).sort_index()
            full_df.index.names = ["ts", "field"]
            
            # Create Bars
            interval = datetime.timedelta(minutes=1)
            bars = Bars(full_df, interval)
            
            # Convert signal to Position
            # Signal is likely 1D array for the asset.
            # Position expects "ts, position" pairs?
            # `Position` class: `__init__(data, interval)`
            # `Position` data: DataFrame.
            
            # Construct Position DataFrame
            # signal_result assumed to be 1D array of floats (weights/positions)
            if isinstance(signal_result, np.ndarray):
                pos_data = pd.DataFrame(signal_result, index=timestamps, columns=["Asset1"])
                position = Position(pos_data, interval)
                
                # Run Backtest
                backtester = CloseBacktester(bars, fee=0.0005) # Assume some fee
                df_results, pos_nan, pos_zero, pos_actual = backtester.run(position)
                
                # Prepare Output
                output_data = {
                    "signal": signal_result.tolist(),
                    "timestamps": [t.timestamp() for t in timestamps], # UNIX timestamp
                    "pnl": df_results["ret"].cumsum().tolist(), # Unrealized PnL cumsum? Or "ret_realized"?
                    # usually pnl curve is cumsum of (ret + ret_realized - fee - slippage)?
                    # let's look at `CloseBacktester`: 
                    # ret: unrealized_return (change in value of held position)
                    # ret_realized: realized_return (cash change from trading)
                    # fee: fee cost
                    # slippage: slippage cost
                    # Total PnL step = ret + ret_realized - fee - slippage
                    
                    "pnl_cumulative": (df_results["ret"] + df_results["ret_realized"] - df_results["fee"] - df_results["slippage"]).cumsum().tolist(),
                    "position": pos_actual.data["Asset1"].tolist(), # Actual position held
                    "turnover": df_results["turnover"].tolist()
                }
                
                if json_path:
                    with open(json_path, 'w') as f:
                        json.dump(output_data, f)
                    print(f"JSON result (with metrics) saved to {json_path}")
                    
                # Setup Plot if requested
                if output_path:
                   import matplotlib.pyplot as plt
                   plt.figure(figsize=(10, 10))
                   plt.subplot(3, 1, 1)
                   plt.plot(output_data["pnl_cumulative"], label="Cumulative PnL")
                   plt.legend()
                   plt.subplot(3, 1, 2)
                   plt.plot(output_data["position"], label="Position")
                   plt.legend()
                   plt.subplot(3, 1, 3)
                   plt.plot(output_data["turnover"], label="Turnover")
                   plt.legend()
                   plt.savefig(output_path)
                   print(f"Result plot saved to {output_path}")

            else:
                print("Signal result is not a numpy array. Skipping PnL calculation.")
                # Fallback to simple JSON export
                if json_path:
                    output_data = {"signal": str(signal_result)}
                    with open(json_path, 'w') as f:
                        json.dump(output_data, f)

        else:
            print("Data directory not found. Generating dummy data for visual feedback...")
            # Automatically generate dummy data
            fields =["open", "high", "low", "close", "volume"]
            # Create a 'data' folder
            os.makedirs(data_dir, exist_ok=True)
            # Create 100 data points
            N = 100
            open_p = np.random.rand(N) * 100
            close_p = open_p + np.random.randn(N)
            high_p = np.maximum(open_p, close_p) + np.abs(np.random.randn(N))
            low_p = np.minimum(open_p, close_p) - np.abs(np.random.randn(N))
            volume = np.random.rand(N) * 1000
            
            np.save(os.path.join(data_dir, "open.npy"), open_p)
            np.save(os.path.join(data_dir, "close.npy"), close_p)
            np.save(os.path.join(data_dir, "high.npy"), high_p)
            np.save(os.path.join(data_dir, "low.npy"), low_p)
            np.save(os.path.join(data_dir, "volume.npy"), volume)
            
            print(f"Dummy data created in {data_dir}. IMPORTANT: PnL calculation relies on this mock data and signal.")
            
            # Recursive call or inline processing? Inline is safer to match logic above.
            # We can just copy the logic or try loading again.
            # Let's just fall through to trying to load again? No, we are in the `else` block of `if os.path.isdir`.
            # To avoid code duplication, we could wrap the calc in a function, but for now let's just re-execute the logic block.
            # OR better: structure logic so it runs if `data_dir` exists OR if we create it.
            
            # Re-running the loading logic:
            loaded_data = {f: np.load(os.path.join(data_dir, f"{f}.npy")) for f in fields}
            length = len(loaded_data["open"])
            
            start_time = pd.Timestamp("2023-01-01 09:00:00")
            timestamps = [start_time + datetime.timedelta(minutes=i) for i in range(length)]
            
            frames = []
            for f in fields:
                df = pd.DataFrame(loaded_data[f], index=timestamps, columns=["Asset1"]) 
                df["field"] = f
                df = df.reset_index().set_index(["index", "field"])
                frames.append(df)
            
            full_df = pd.concat(frames).sort_index()
            full_df.index.names = ["ts", "field"]
            
            interval = datetime.timedelta(minutes=1)
            bars = Bars(full_df, interval)
            
            if isinstance(signal_result, np.ndarray):
                # Ensure signal length matches data length for dummy data
                # Dummy data N=100. Signal might be N=100.
                if len(signal_result) != N:
                     print(f"Warning: Signal length {len(signal_result)} != Data length {N}. Padding/Truncating.")
                     if len(signal_result) < N:
                         signal_result = np.pad(signal_result, (0, N - len(signal_result)))
                     else:
                         signal_result = signal_result[:N]
                
                pos_data = pd.DataFrame(signal_result, index=timestamps, columns=["Asset1"])
                position = Position(pos_data, interval)
                
                backtester = CloseBacktester(bars, fee=0.0005) 
                df_results, pos_nan, pos_zero, pos_actual = backtester.run(position)
                
                output_data = {
                    "signal": signal_result.tolist(),
                    "timestamps": [t.timestamp() for t in timestamps],
                    "pnl": df_results["ret"].cumsum().tolist(),
                    "pnl_cumulative": (df_results["ret"] + df_results["ret_realized"] - df_results["fee"] - df_results["slippage"]).cumsum().tolist(),
                    "position": pos_actual.data["Asset1"].tolist(),
                    "turnover": df_results["turnover"].tolist()
                }
                
                if json_path:
                    with open(json_path, 'w') as f:
                        json.dump(output_data, f)
                    print(f"JSON result (with metrics) saved to {json_path}")
            else:
                 if json_path:
                    with open(json_path, 'w') as f:
                        json.dump({"signal": str(signal_result)}, f)

    except Exception as e:
        print(f"Error calculating PnL metrics: {e}")
        import traceback
        traceback.print_exc()
        # Fallback
        if json_path:
             with open(json_path, 'w') as f:
                json.dump({"signal": str(signal_result), "error": str(e)}, f)



def run_hftbacktest(signal, data_path):
    pass
