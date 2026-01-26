import signal
from butterflow import lex, Parser, TypeChecker, Builder, Runtime
import numpy as np
from glob import glob
import json
from datetime import datetime, timedelta
import pandas as pd
from rhetenor import data
from rhetenor.backtest import normalize_position, compute
from rhetenor.stat import calculate_stat

cfgs = {"hantoo_config_path": "../auth/hantoo.yaml", "hantoo_token_path":
        "../auth/hantoo_token.yaml", "aws_config_path": "../auth/aws_rhetenor.yaml"}
s3_cfgs = {"auth_config_path": "../auth/aws_rhetenor.yaml"}
SILENT = True

s3 = data.S3KlineWrapper(exchange_code="UN", bucket="rhetenor", **s3_cfgs)
day_start = datetime.combine(datetime.now().date(), datetime.min.time())
s3.load(datetime_from=datetime.now()-timedelta(days=10), datetime_to=day_start)
df = pd.DataFrame({k: pd.DataFrame(v["data"], index=v["fields"]).stack(
) for k, v in s3.loaded_data_map.items()}).stack()
df.sort_index()
fields = ["open", "high", "low", "close", "volume"]
dfs = {k: df[k].unstack(level=0).sort_index(
    axis=0).sort_index(axis=1).astype(float) for k in fields}
assert all([(x.index == dfs["close"].index).all() for x in dfs.values()])
assert all([(x.columns == dfs["close"].columns).all() for x in dfs.values()])
runtime_data = {f'data("{k}")': v.values for k, v in dfs.items()}


# Initialize runtime
x_close = runtime_data['data("close")']
x_close_d1 = np.roll(runtime_data['data("close")'], shift=1, axis=0)
x_close_d1[0] = x_close[0]
x_logret = np.log(x_close / x_close_d1)
runtime_data['data("price")'] = x_close
runtime_data['data("returns")'] = x_logret  # logret
runtime = Runtime(data=runtime_data)

# Load generated alphas
generated = dict()
for f in glob("./generate*/*.json"):
    with open(f, "rt") as fp:
        fname = f.split("/")[-1][:-5]
        generated[fname] = json.load(fp)


def handler(signum, frame):
    raise Exception("Timeout")


signal.signal(signal.SIGALRM, handler)


# Calculate alphas and save stats
valid_jsons = []
invalid_jsons = []
for fname, g in generated.items():
    try:
        signal.alarm(20)
        input_code = g['generation_result']['code']
        position_input = compute(runtime, input_code, silent=SILENT)
        position_raw, position = normalize_position(position_input, x_logret)

        def avg_10(x): return pd.DataFrame(x).rolling(10).mean().values
        stat = calculate_stat(position_raw, position,
                              x_logret, include_pnl=True)
        stat_delay = calculate_stat(
            position_raw[:-10], position[:-10], x_logret[10:], include_pnl=True)
        stat_decay = calculate_stat(avg_10(
            position_raw[:-10]), avg_10(position[:-10]), x_logret[10:], include_pnl=True)
        stat_nobalance = calculate_stat(
            position_raw[:-10], position[:-10], (np.exp(x_logret)-1)[10:], include_pnl=True)

        valid_jsons.append(fname)
        # returns = stat.pop("returns_series")
        signal_id = fname.replace("/", "_").replace(".", "_")

        pd.Series({"path": fname, "stat": stat, "stat_delay": stat_delay, "stat_decay": stat_decay,
                  "stat_nobalance": stat_nobalance}).to_pickle(f"pnls/{signal_id}.pkl")
    except Exception as e:
        print(repr(e))
        stat = {"error": repr(e)}
        invalid_jsons.append(fname)
    g["calculation_result"] = stat
    # with open(f"./calculated_KRX/{fname}.json", "wt") as f:
    #     json.dump(g, f)
    print(
        f"[valid {len(valid_jsons)}, invalid {len(invalid_jsons)} / {len(generated)}]", end="\r")
print()

pd.Series(valid_jsons).to_csv("valid_jsons.csv")
pd.Series(invalid_jsons).to_csv("invalid_jsons.csv")
# [valid 3191, invalid 7885 / 11076]
