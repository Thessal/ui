from butterflow import lex, Parser, TypeChecker, Builder, Runtime
import numpy as np


def initialize_runtime():
    # Initialize runtime
    runtime_data = {
        f'data("{x}")': np.load(f"data/{x}.npy")
        for x in ["open", "high", "low", "close", "volume"]}
    x_close = runtime_data['data("close")']
    x_close_d1 = np.roll(runtime_data['data("close")'], shift=1, axis=0)
    x_close_d1[0] = x_close[0]
    x_logret = np.log(x_close / x_close_d1)
    runtime_data['data("price")'] = x_close
    runtime_data['data("returns")'] = x_logret  # logret
    runtime = Runtime(data=runtime_data)
    return runtime


def compute(runtime, input_code: str):
    tokens = lex(input_code)
    parser = Parser(tokens)
    ast = parser.parse()
    checker = TypeChecker()
    checker.check(ast)
    builder = Builder()
    graph = builder.build(ast)
    result = runtime.run(graph)
    return result


def normalize_position(position_input, x_logret):
    # normalize position
    assert (position_input.shape == x_logret.shape)
    position_raw = position_input - \
        np.nanmean(position_input, axis=1, keepdims=True)
    ls = position_raw / \
        np.nansum(np.where(position_raw >= 0, position_raw, np.nan),
                  axis=1, keepdims=True)
    ss = position_raw / \
        np.nansum(np.where(position_raw < 0, position_raw, np.nan),
                  axis=1, keepdims=True)
    position_raw = np.where(position_raw >= 0, ls, ss)
    position = np.nan_to_num(position_raw, 0)
    return position_raw, position


# # Batch backtest

# # Load generated alphas
# generated = dict()
# for f in glob(ALPHA_DIR):
#     with open(f,"rt") as fp:
#         fname = f.split("/")[-1][:-5]
#         generated[fname] = json.load(fp)

# # Calculate alphas and save stats
# import signal
# for fname, g in generated.items():
#     try:
#         # signal.alarm(10) # NOTE: causes truble when debugging
#         input_code = g['generation_result']['code']
#         position_input = compute(runtime, input_code)
#         position_raw, position = normalize_position(position_input, x_logret)
#         stat = calculate_stat(position_raw, position, x_logret)
#     except Exception as e:
#         stat = {"error": repr(e)}
#     result = {
#         "hash": g["hash"],
#         **stat,
#         **{"generated_"+k:v for k,v in g["generation_result"].items()},
#         **g["orig_metadata"]
#     }
#     # g["calculation_result"] = stat
#     with open(f"./calculated/{fname}.json", "wt") as f:
#         json.dump(result, f)
