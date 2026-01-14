#%%
import pandas as pd 
import json
from glob import glob

pd.set_option('display.max_columns', 500)
pd.set_option('display.width', 1000)
#%%

# Inspection
def inspect(inspect_idx, path="data/finetune-03/*.json"):
    dfs = {}
    jsons = glob(path)
    results = {0:[], inspect_idx:[]}
    for path in jsons:
        name = path.split("/")[-1].split("_")[0]
        with open(path, "rt") as f:
            y = json.load(f)
            for iterations in [0, inspect_idx]:
                scores = y["history"][iterations]["scores"]
                scores["name"] = name
                results[iterations].append(scores)
    for k,v in results.items():
        dfs[k] = pd.DataFrame(v)
    print("count")
    print(len(dfs[0]))
    print("")
    print("improvement")
    print(dfs[inspect_idx].count() - dfs[0].count())
    print("")
    print("by_name")
    print(dfs[inspect_idx].groupby("name").count())
    print("by_name")
    print(dfs[inspect_idx].groupby("name").mean())
    # print(dfs)

# inspect(1, path="data/finetune-03/*.json")
# %%

# score verification
import random


def sample(path="data/finetune-03/*.json"):
    path = random.choice(glob(path))
    with open(path, "rt") as f:
        y = random.choice(json.load(f)["history"])
        scores = y["scores"]
        code = y["code"]
    return scores, code


def score_summary(scores):
    if scores["position_concentration"] == None:
        position_balance = 1.
    elif scores["position_concentration"] < 0.05:
        position_balance = 5.
    elif scores["position_concentration"] < 0.15:
        position_balance = 4.
    elif scores["position_concentration"] < 0.20:
        position_balance = 3.
    else:
        position_balance = 1.

    if scores["ret"] == None:
        information = 1.
    elif abs(scores["ret"]/(scores["std"]+abs(scores["ret"])+1e-9)) < 0.001:
        information = 2.
    elif abs(scores["ret"]/(scores["std"]+abs(scores["ret"])+1e-9)) < 0.01:
        information = 3.
    elif abs(scores["ret"]/(scores["std"]+abs(scores["ret"])+1e-9)) < 0.03:
        information = 4.
    elif abs(scores["ret"]/(scores["std"]+abs(scores["ret"])+1e-9)) >= 0.03:
        information = 5.
    else:
        information = 1.

    if scores["tvr"] == None:
        turnover = 1.
    elif (scores["tvr"] < 0.005) or (0.7 < scores["tvr"]):
        turnover = 2.
    elif (scores["tvr"] < 0.01) or (0.4 < scores["tvr"]):
        turnover = 3.
    elif (scores["max_tvr"] / scores["tvr"] > 8):
        turnover = 4.
    elif (scores["max_tvr"] / scores["tvr"] <= 8):
        turnover = 5.
    else:
        turnover = 1.

    scores_summary = {
        "syntax-lex" : scores['lex'],
        "syntax-parse" : scores['parse'],
        "syntax-type_check" : scores['type_check'],
        "syntax-runtime" : scores['build'],
        "semantics-position_balance" : position_balance,
        "semantics-information" : information,
        "semantics-turnover" : turnover,
    }
    return scores_summary


def score_fn(scores):
    scores_summary = score_summary(scores)
    return sum(v for v in scores_summary.values() if type(v) == float)


# scores, code = sample()
# print(code, "\n\nSCORE:", score_fn(scores))
# print("\n\n")
# scores, code = sample()
# print(code, "\n\nSCORE:", score_fn(scores))

def side_by_side(a,b, w=50):
    aa = a.split("\n")
    bb = b.split("\n")
    ll = max(len(a),len(b))
    aa.extend(["\n"]*ll)
    bb.extend(["\n"]*ll)
    aa = [(a+" "*w)[:w] for a in aa[:ll]]
    bb = [(b+" "*w)[:w] for b in bb[:ll]]
    for a,b in zip(aa,bb):
        print(a + " | " + b)
# %%


scores_1, code_1 = sample()
sample_1 = (code_1, score_fn(scores_1))
scores_2, code_2 = sample()
sample_2 = (code_2, score_fn(scores_2))

side_by_side("[1]","[2]")
side_by_side(sample_1[0], sample_2[0])
choice = input("Which one is better? [1/2]:")
pred = '1' if sample_1[1] > sample_2[1] else '2'
print(f"The score function says: {pred} ({sample_1[1]}, {sample_2[1]})")
print(scores_1)
print(scores_2)

#%%

