import numpy as np


def calculate_stat(position_raw, position, x_logret, include_pnl=False):
    # Calculate stat
    returns = np.nansum(position[:-1] * x_logret[1:], axis=1)
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
    if include_pnl: 
        stat["returns_series"] = returns
        stat["turnover_series"] = tvrs
    return stat