from butterflow import lex, Parser, TypeChecker, Builder, Runtime
import numpy as np
from glob import glob
import json
from datetime import datetime, timedelta
import pandas as pd
from rhetenor import data
from rhetenor.backtest import s3_to_df, initialize_runtime, compute, Bars, Position, CloseBacktester
from rhetenor.stat import calculate_stat

SILENT = True
TIMEOUT= False
DELAY = 1 # Use T data, Position calculated at T+1
SIM_INTERVAL = timedelta(minutes=10)
s3_cfgs = {"auth_config_path": "../auth/aws_rhetenor.yaml"}
s3 = data.S3KlineWrapper(exchange_code="UN", bucket="rhetenor", **s3_cfgs)
day_start = datetime.combine(datetime.now().date(), datetime.min.time())
# NOTE: timestamp inconsistency around 20260119 ~ 20260126
s3.load(datetime_from=datetime.now()-timedelta(days=10), datetime_to=day_start)

# Initialize runtime
df, dfs = s3_to_df(s3)
runtime = initialize_runtime(dfs=dfs, add_logret=True)
bars = Bars(data=df, interval=SIM_INTERVAL)
backtester = CloseBacktester(data=bars, fee=0.1 * 0.01)

# Load generated alphas
generated = dict()
for f in glob("./generate*/*.json"):
    with open(f, "rt") as fp:
        fname = f.split("/")[-1][:-5]
        generated[fname] = json.load(fp)


for f in glob("rhetenor/notebooks/generate*/*.json"):
    with open(f, "rt") as fp:
        fname = f.split("/")[-1][:-5]
        generated[fname] = json.load(fp)
fname = list(generated.keys())[0]
g = generated[fname]

if TIMEOUT:
    import signal
    def handler(signum, frame):
        raise Exception("Timeout")
    signal.signal(signal.SIGALRM, handler)

# Calculate alphas and save stats
valid_jsons = []
invalid_jsons = []
for fname, g in generated.items():
    try:
        if TIMEOUT:
            signal.alarm(60) # turn off when debugging
        input_code = g['generation_result']['code']
        position_input = compute(runtime, input_code, silent=SILENT)

        position = Position(
            data=pd.DataFrame(
                position_input, index=dfs["close"].index, columns=dfs["close"].columns).shift(DELAY),
            interval=SIM_INTERVAL
        )
        position_delay = Position(position.data.rolling(10).mean(), position.interval)
        position_decay = Position(position.data.shift(10), position.interval)

        df_results, pos_nanfilled, pos_zerofilled, pos_actual = backtester.run(position)
        df_results_delay, pos_nanfilled_delay, _, pos_actual_delay = backtester.run(position_delay)
        df_results_decay, pos_nanfilled_decay, _, pos_actual_decay = backtester.run(position_delay)

        stat = calculate_stat(df_results, pos_nanfilled.data, pos_actual.data)
        stat_delay = calculate_stat(df_results_delay, pos_nanfilled_delay.data, pos_actual_delay.data)
        stat_decay = calculate_stat(df_results_decay, pos_nanfilled_decay.data, pos_actual_decay.data)

        valid_jsons.append(fname)
        signal_id = fname.replace("/", "_").replace(".", "_")

        df_output = pd.Series({"path": fname, "stat": stat, "stat_delay": stat_delay, "stat_decay": stat_decay})
        df_output.to_pickle(f"pnls/{signal_id}.pkl")
    except Exception as e:
        print(repr(e))
        stat = {"error": repr(e)}
        invalid_jsons.append(fname)
    g["calculation_result"] = stat
    # with open(f"./calculated_KRX/{fname}.json", "wt") as f:
    #     json.dump(g, f)
    print()
    print(
        f"[valid {len(valid_jsons)}, invalid {len(invalid_jsons)} / {len(generated)}]", end="\r")
    print()

pd.Series(valid_jsons).to_csv("valid_jsons.csv")
pd.Series(invalid_jsons).to_csv("invalid_jsons.csv")
# [valid 3191, invalid 7885 / 11076]
