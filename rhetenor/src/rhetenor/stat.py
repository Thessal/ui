import numpy as np
import pandas as pd 

def calculate_stat(df_result:pd.DataFrame, position_raw:pd.DataFrame, position:pd.DataFrame):
    returns = df_result["ret"]
    fee = df_result["fee"] # TODO
    turnover = df_result["turnover"]

    cum_pnl = np.cumsum(returns)
    tvrs = np.nansum(np.abs(np.diff(position, axis=0)), axis=1)
    stat = {
        "min_coverage": np.nanmin(np.nanmean(np.isfinite(position_raw), axis=1)),
        "returns": np.nanmean(returns)*252,
        "sharpe": np.nanmean(returns)/np.nanstd(returns)*np.sqrt(252),
        "max_turnover": np.nanmax(tvrs),
        "mdd": np.nanmax(np.maximum.accumulate(cum_pnl) - cum_pnl),
        "max_position": np.nanmax(np.abs(position))
    }
    return stat