from datetime import datetime, timedelta
import pandas as pd
from rhetenor import data
import time
cfgs = {"hantoo_config_path": "../auth/hantoo.yaml", "hantoo_token_path":
        "../auth/hantoo_token.yaml", "aws_config_path": "../auth/aws_rhetenor.yaml"}
master = data.download_master(
    market="kospi", verbose=True, auth_config_path=cfgs["aws_config_path"])
kospi_syms = [k for k, v in master.items() if v["kospi50"] == "Y"]
logger = data.HantooKlineLogger(symbols=kospi_syms, exchange_code="UN", prefix = "hantoo_stk_kline_1m", **cfgs)
for x in range(100):
    time.sleep(10*60)
    a = logger.fetch_and_update()
    print(a)