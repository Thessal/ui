import numpy as np 

def calculate_stat(position_raw, position, x_logret):
    # Calculate stat 
    returns = np.nansum(position[:-1] * x_logret[1:], axis=1)
    stat = {
        "min_coverage": np.nanmin(np.mean(np.isfinite(position_raw))),
        "returns": np.nanmean(returns)*252,
        "sharpe": np.nanmean(returns)/np.nanstd(returns)*np.sqrt(252),
        "max_turnover": np.nanmin(np.nanmean(np.abs(np.diff(position)), axis=1)),
        "mdd": np.min(np.cumsum(returns)),
        "max_position": np.nanmax(np.abs(position))
    }
    return stat