from typing import Any, Dict, List, Optional, Iterator, Tuple
try:
    from butterflow import lex, Parser, TypeChecker, Builder, Runtime
except ImportError:
    print("Warning: butterflow not found. Using mock classes.")
    class Mock: pass
    lex = Mock
    Parser = Mock
    TypeChecker = Mock
    Builder = Mock
    Runtime = Mock

import numpy as np
import pandas as pd
import datetime

def unstack(df):
    # df -> dfs
    # [ts*field*instr] -> {field: [ts * instr]}
    dfs = {f: df.swaplevel(0, 1, axis=0).loc[f].sort_index(
    ) for f in df.index.get_level_values(1).unique()}
    idxs = [v.index for v in dfs.values()]
    cols = [v.columns for v in dfs.values()]
    assert all([all(x == idxs[0]) for x in idxs]), "s3 data failed to parse due to index consistency"
    assert all([all(x == cols[0]) for x in cols]), "s3 data failed to parse due to column consistency"
    return dfs

def stack(dfs):
    # dfs -> df
    # {field: [ts * instr]} -> [ts*field*instr]
    df = pd.concat(dfs).swaplevel(0,1).sort_index()
    return df
         
def s3_to_df(s3):
    # [ts*field*instr]
    records = s3.loaded_data_map
    # check date format
    if not all(len(k)==12 for k in records.keys()):
        print("Warning: Timestamp length is not 12 (YYYYMMDDHHMM) : skipped")

    fields = records[sorted(records.keys())[0]]["fields"]
    records = {pd.to_datetime(k, format="%Y%m%d%H%M"):v for k,v in records.items() if len(k)==12}

    records = {
        (ts, instr): dict(zip(info["fields"], bars)) 
        for ts, info in records.items()
        for instr, bars in info["data"].items()
        }
    df = pd.DataFrame(records).T
    df = df.stack().unstack(1)
    df = df.sort_index(axis=0).sort_index(axis=1)

    try:
        np.testing.assert_array_equal(df, stack(unstack(df)))
    except:
        raise Exception("Data is not invariant under stack transform")
    return df

def overnight_synthetic_bar(s3_df, open_time:int = 900, close_time:int = 1530):
    # s3_dfs : {field : DataFrame[time*instrs]}
    # s3_df : DataFrame[ts*field*instr]
    # TODO: let Bar class manage it 
    instrs = s3_df.columns
    volume_aggregation_method = "max"
    agg = {
        "open":lambda x: x.iloc[0], 
        "high":"max", 
        "low":"min", 
        "close":lambda x: x.iloc[-1], 
        "volume":volume_aggregation_method, 
        "acc_krw_vol":lambda x: x.iloc[-1], 
        }

    df = s3_df.unstack(1)

    output = []
    buffer = dict()
    for i,x in df.iterrows():
        time = i.hour*100 + i.minute
        if time == open_time:
            # Aggregete overnight data
            if buffer : 
                df_tmp = pd.DataFrame(buffer).T
                agg_methods = {(instr, field):agg[field] for instr, field in df_tmp.columns}
                synthetic_bar = df_tmp.agg(agg_methods)
                synthetic_time = sorted(buffer.keys())[-1]
                synthetic_bar = pd.DataFrame({synthetic_time:synthetic_bar.to_dict()}).T
                output.append(synthetic_bar)
                buffer.clear()
            buffer[i] = x
        elif time == close_time:
            # Intraday data
            buffer[i] = x
            output.append(pd.DataFrame(buffer).T)
            buffer.clear()
        else:
            buffer[i] = x
    output.append(pd.DataFrame(buffer).T)
    buffer.clear()
    
    result = pd.concat(output).stack(future_stack=True)
    result = result.sort_index(axis=0).sort_index(axis=1)
    assert len(result.columns) == len(instrs), "columns changes during dataframe operation"
    assert all(result.columns == instrs), "columns changes during dataframe operation"
    return result


def initialize_runtime(dfs: Optional[Dict[str, pd.DataFrame]] = None, add_logret=False, check_corruption=False):
    if type(dfs) != type(None):
        runtime_data = {f'data("{k}")': v.values for k, v in dfs.items()}
    else:
        runtime_data = {
            f'data("{x}")': np.load(f"data/{x}.npy")
            for x in ["open", "high", "low", "close", "volume"]}
    if add_logret:
        print("DEPECATED: logret")
        x_close = pd.DataFrame(runtime_data['data("close")']).ffill().values
        x_close_d1 = np.roll(runtime_data['data("close")'], shift=1, axis=0)
        x_close_d1[0] = x_close[0]
        x_logret = np.log(x_close / x_close_d1)
        runtime_data['data("price")'] = x_close
        runtime_data['data("returns")'] = x_logret  # logret
    return Runtime(data=runtime_data, check_corruption=check_corruption)


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
        assert data.index.nlevels == 2, "bar initialization failure due to input data index depth"
        assert data.index.get_level_values(0).dtype == "<M8[ns]", "bar initialization failure due to input timestamp dtype"
        assert np.isin(np.array(self.fields),
                    data.index.get_level_values(1).unique()).all(), "bar initialization failure due to unrecognized fields"
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
            df_x=pd.DataFrame(agg[field], columns=self.columns)) for field in self.fields}
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
        assert hasattr(self.data, "columns") and hasattr(position, "columns"), "Attribute position missing"
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
                np.maximum(np.nansum(np.where(position_raw >= 0, position_raw,
                        np.nan), axis=1, keepdims=True), 1e-6)
            ss = position_raw / \
                np.maximum(np.abs(np.nansum(np.where(position_raw < 0,
                    position_raw, np.nan), axis=1, keepdims=True)), 1e-6)
            position_raw = np.where(position_raw >= 0, ls, ss)

        # Zero fill
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

    def _execution_assumption(self, old_position: pd.Series, new_position: pd.Series, prev_bar: Dict[str, pd.Series], bar: Dict[str, pd.Series]) -> Tuple[pd.Series, float, float, float, float, float, pd.Series]:
        # old_position: Position Dollar Values at prev_bar Close
        # new_position: Target Dollar Values at bar Close

        # 1. Forward-fill bars, calculate retrns
        curr_bar = {f: bar[f].where(np.isfinite(bar[f]), prev_bar[f]).fillna(0) for f in bar.keys() if f != "volume"}
        curr_bar["volume"] = bar["volume"].fillna(0)

        # 2. Drift return of current position, occured during the last period
        returns = (curr_bar["open"]-prev_bar["open"]).div(prev_bar["open"])
        returns.loc[~np.isfinite(returns)]=0
        drifted_position = old_position * (1.0 + returns)
        unrealized_return = (drifted_position - old_position).sum()

        # 3. Rebalance to new_position
        trade_amt = (new_position - drifted_position)

        # 4 Costs 
        turnover = np.sum(np.abs(trade_amt))
        fee_cost = turnover * self.fee
        # Slippage assumption : If limit order at prev close is not matched, send agressive order.
        # This slippage estimation may not work as intended, if weight is negative 
        buy_target_price = prev_bar["close"] * (1 + 0.000)
        sell_target_price = prev_bar["close"] * (1 - 0.000)
        aggression = 0.005
        slippage_buy = np.where((trade_amt > 0) & (buy_target_price<=bar["low"]), bar["high"]/buy_target_price - 1, prev_bar["close"]/buy_target_price - 1)
        slippage_sell = np.where((trade_amt < 0) & (sell_target_price>=bar["high"]), bar["low"]/sell_target_price - 1, prev_bar["close"]/sell_target_price - 1)
        
        # 5. Realized trades
        trade_amt = trade_amt.where(bar["volume"]>0, 0)
        trade_amt = trade_amt.where(slippage_buy-slippage_sell < aggression, 0)

        slippage = np.abs(trade_amt * (slippage_buy + slippage_sell))
        slippage_cost = np.sum(slippage)

        realized_return = -trade_amt.sum()
        realized_position = drifted_position + trade_amt

        return realized_position, unrealized_return, realized_return, fee_cost, slippage_cost, turnover, curr_bar

    def run(self, position: Position):
        self._check_position(position)
        assert self.data.interval == position.interval, "data interval mismatch between bar and position in the backtester"

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

            last_bar = bar
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
                assert intv_bar.left == ts_pos.floor(position.interval), f"record align miss in backtester : \nBar: {intv_bar}, {intv_bar.left}\nPosition: {ts_pos}, {ts_pos.floor(position.interval)}"
                prev_pos, ret_unreal, ret_real, fee, slippage, turnover, last_bar = self._execution_assumption(
                    prev_pos, pos, last_bar, bar)
                positions[intv_bar.left] = prev_pos
                results.append({"ts": intv_bar.left, "ret": ret_unreal, "ret_realized": ret_real,
                               "fee": fee, "slippage": slippage, "turnover": turnover})
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
