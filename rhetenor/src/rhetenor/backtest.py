from typing import Any, Dict, List, Optional, Iterator, Tuple
from butterflow import lex, Parser, TypeChecker, Builder, Runtime
import numpy as np
import pandas as pd
import datetime


def s3_to_df(s3):
    df = pd.DataFrame({k: pd.DataFrame(v["data"], index=v["fields"]).stack(
    ) for k, v in s3.loaded_data_map.items()}).stack()
    df = df.sort_index().unstack(level=1).sort_index(axis=1)
    timestamps = pd.to_datetime(
        df.index.get_level_values(1), format="%Y%m%d%H%M%S")
    fields = df.index.get_level_values(0)
    df.index = pd.MultiIndex.from_arrays(
        [timestamps, fields], names=["timestamp", "field"])
    dfs = {f: df.swaplevel(0, 1, axis=0).loc[f].sort_index(
    ) for f in df.index.get_level_values(1).unique()}

    idxs = [v.index for v in dfs.values()]
    cols = [v.columns for v in dfs.values()]
    assert all([all(x == idxs[0]) for x in idxs])
    assert all([all(x == cols[0]) for x in cols])
    return df.sort_index(), dfs


def initialize_runtime(dfs: Optional[Dict[str, pd.DataFrame]] = None, add_logret=True):
    if type(dfs) != type(None):
        runtime_data = {f'data("{k}")': v for k, v in dfs.items()}
    else:
        runtime_data = {
            f'data("{x}")': np.load(f"data/{x}.npy")
            for x in ["open", "high", "low", "close", "volume"]}
    if add_logret:
        x_close = pd.DataFrame(runtime_data['data("close")']).ffill().values
        x_close_d1 = np.roll(runtime_data['data("close")'], shift=1, axis=0)
        x_close_d1[0] = x_close[0]
        x_logret = np.log(x_close / x_close_d1)
        runtime_data['data("price")'] = x_close
        runtime_data['data("returns")'] = x_logret  # logret
    return Runtime(data=runtime_data)


def compute(runtime, input_code: str, silent=True):
    tokens = lex(input_code)
    parser = Parser(tokens)
    ast = parser.parse()
    checker = TypeChecker(silent=silent)
    checker.check(ast)
    builder = Builder(silent=silent)
    graph = builder.build(ast)
    result = runtime.run(graph)
    return result


class MarketData:
    def __init__(self, data: pd.DataFrame):
        self.data = data


class Quotes(MarketData):
    ...


class Trades(MarketData):
    ...


class Bars(MarketData):
    # Emits (interval, bar) pairs where interval.left is the bar start time
    def __init__(self, data: pd.DataFrame, interval: datetime.timedelta):
        self.fields = ["open", "high", "low", "close", "volume"]
        self.columns = data.columns
        assert data.index.nlevels == 2
        assert data.index.get_level_values(0).dtype == "<M8[ns]"
        assert np.isin(np.array(self.fields),
                       data.index.get_level_values(1).unique()).all()
        self.interval = interval
        super().__init__(data)
        self.agg_fn = {
            "open": lambda df_x: df_x.iloc[0],
            "high": lambda df_x: df_x.max(axis=0),
            "low": lambda df_x: df_x.min(axis=0),
            "close": lambda df_x: df_x.iloc[-1],
            "volume": lambda df_x: df_x.sum(axis=0)
        }

    def _agg_row(self, agg: Dict[str, List[Any]], start_time: pd.Timestamp) -> Tuple[pd.Interval, Dict[str, pd.Series]]:
        end_time = start_time + self.interval
        interval = pd.Interval(left=start_time, right=end_time, closed='left')
        assert all((t in interval)
                   for t in agg["ts"]), f"{agg['ts']} not in Interval {interval}"
        res = {field: self.agg_fn[field](
            df_x=pd.DataFrame(agg[field])) for field in self.fields}
        return interval, res

    def __iter__(self) -> Iterator[Tuple[pd.Interval, Dict[str, pd.Series]]]:
        last_ts = self.data.index.get_level_values(0)[0]
        current_interval_start = pd.Timestamp(last_ts).floor(self.interval)
        def default_dict(): return {field: []
                                    for field in self.fields + ["ts"]}
        agg = default_dict()

        for (ts, field), row in self.data.iterrows():
            if not (field in self.agg_fn.keys()):
                continue
            if ts < last_ts:
                raise ValueError(
                    f"Data is not sorted. Timestamp {ts} < {last_ts}")

            bucket_start = pd.Timestamp(ts).floor(self.interval)
            if bucket_start > current_interval_start:
                # Yield previous bucket
                yield self._agg_row(agg, current_interval_start)

                # Reset
                current_interval_start = bucket_start
                agg = default_dict()

            # Update accumulators
            agg["ts"].append(ts)
            agg[field].append(row.values)

        # Yield last bucket
        yield self._agg_row(agg, current_interval_start)


class Position:
    # Emits (ts, position) pairs where ts is position calculatin time.
    def __init__(self, data: pd.DataFrame, interval: datetime.timedelta):
        self.data = data
        self.columns = data.columns
        self.interval = interval

    def __iter__(self) -> Iterator[Dict[str, pd.Series]]:
        last_ts = self.data.index[0]
        # properly handle start time based on data
        current_interval_start = None

        for ts, row in self.data.iterrows():
            if ts < last_ts:
                raise ValueError(
                    f"Data is not sorted. Timestamp {ts} < {last_ts}")
            bucket_start = pd.Timestamp(ts).floor(self.interval)

            if current_interval_start is None or bucket_start > current_interval_start:
                current_interval_start = bucket_start
                # ts is calculation time. executed at floor(ts+1)
                yield ts, row


class Backtester:
    def __init__(self, data: Any):
        self.data = self._preprocess_data(data)

    def _preprocess_data(self, data: Any):
        return data

    def _check_position(self, position: pd.DataFrame):
        assert hasattr(self.data, "columns") and hasattr(position, "columns")
        assert all(position.columns ==
                   self.data.columns), f"Position columns {position.columns} do not match data columns {self.data.columns}"

    def _preprocess_position(self, position: Position):
        # Neutralize
        x_position = position.data.values
        position_raw = x_position - \
            np.nanmean(x_position, axis=1, keepdims=True)
        # Long/Short balance
        if position_raw.shape[1] > 1:
            ls = position_raw / \
                np.nansum(np.where(position_raw >= 0, position_raw,
                          np.nan), axis=1, keepdims=True)
            ss = position_raw / \
                np.abs(np.nansum(np.where(position_raw < 0,
                       position_raw, np.nan), axis=1, keepdims=True))
            position_raw = np.where(position_raw >= 0, ls, ss)
        position_zerofilled = np.nan_to_num(position_raw, 0)
        position_nanfilled = pd.DataFrame(
            position_raw, index=position.data.index, columns=position.columns)
        position_zerofilled = pd.DataFrame(
            position_zerofilled, index=position.data.index, columns=position.columns)
        # return position_nanfilled (for coverage check), position_zerofilled (for backtesting)
        return Position(data=position_nanfilled, interval=position.interval), Position(data=position_zerofilled, interval=position.interval)

    def run(self, position: Position) -> List[Dict]:
        self._check_position(position)
        raise NotImplementedError


class CloseBacktester(Backtester):
    def __init__(self, data: Bars, fee: float = 0.0):
        super().__init__(data)
        self.fee = fee

    def _execution_assumption(self, old_position: pd.Series, new_position: pd.Series, last_price: pd.Series, bar: Dict[str, pd.Series]) -> Tuple[pd.Series, float, float, float, pd.Series]:
        # old_position: Position Dollar Values at prev_bar Close
        # new_position: Target Dollar Values at bar Close

        # 1. Calculate PnL from holding old_position from prev_bar.close to bar.close
        assert np.isfinite(last_price).all(), "last_price contains nans"
        prev_close = last_price
        curr_close = prev_close.where(np.isfinite(bar["close"]), bar["close"])

        # Avoid division by zero
        returns = np.zeros_like(prev_close)
        mask = (prev_close != 0) & (
            ~np.isnan(prev_close)) & (~np.isnan(curr_close))
        returns[mask] = (curr_close[mask] / prev_close[mask]) - 1.0

        # drift
        pnl_holding = np.nansum(old_position * returns)

        # Value of position before rebalance
        drifted_position = old_position * (1.0 + returns)

        # 2. Rebalance to new_position
        trade_amt = new_position - drifted_position
        trade_amt = np.where((bar["volume"] == 0) |
                             np.isnan(curr_close), 0, trade_amt)

        # Net PnL
        turnover = np.sum(np.abs(trade_amt))
        fee_cost = turnover * self.fee
        ret = pnl_holding - np.nansum(trade_amt) - fee_cost
        realized_position = drifted_position + trade_amt

        return realized_position, ret, fee_cost, turnover, last_price

    def run(self, position: Position):
        self._check_position(position)
        assert self.data.interval == position.interval

        pos_nanfilled, pos_zerofilled = self._preprocess_position(position)
        bars = iter(self.data)
        pos_iter = iter(pos_zerofilled)

        # Align starting time
        try:
            intv_bar, bar = next(bars)
            ts_pos, pos = next(pos_iter)

            # Catchup
            while intv_bar.left < ts_pos:
                intv_bar, bar = next(bars)

            while ts_pos < intv_bar.left:
                ts_pos, pos = next(pos_iter)

            last_price = bar["close"]
            prev_pos = pos  # np.zeros_like(pos) # ignore enter cost

        except StopIteration:
            return []

        # Consume
        results = []
        positions = dict()
        while True:
            try:
                intv_bar, bar = next(bars)
                ts_pos, pos = next(pos_iter)
                # print(intv_bar.left, ts_pos.floor(position.interval))
                # make sure bars and pos are aligned
                assert intv_bar.left == ts_pos.floor(position.interval)
                prev_pos, ret, fee, turnover, last_price = self._execution_assumption(
                    prev_pos, pos, last_price, bar)
                positions[intv_bar.left] = prev_pos
                results.append({"ts": intv_bar.left, "ret": ret,
                               "fee": fee, "turnover": turnover})
            except StopIteration:
                break
        pos_actual = Position(pd.DataFrame(positions).T, interval=pos_nanfilled.interval)
        df_results = pd.DataFrame(results).set_index("ts", drop=True)
        return df_results, pos_nanfilled, pos_zerofilled, pos_actual


class MarketBacktester(Backtester):
    # buy at ask price, sell at bid price
    def __init__(self, trades, quotes):
        pass


class VWAPBacktester(Backtester):
    def __init__(self, trades, quotes):
        pass
