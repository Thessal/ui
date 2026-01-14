import json
from glob import glob
from typing import Dict
from morpho.score import SemanticTeacher, SyntaxTeacher
import argparse
from finetune_util import hasher, load_stdlib_doc, query_ollama, check_save, check_exist
import sys
import random


class Grader:
    def __init__(self, datadir="./data/npy"):
        self.teacher_syn = SyntaxTeacher()
        self.teacher_sem = SemanticTeacher(
            datadir=datadir, propritary=False)

    def check(self, code):
        graph, scores_1, error_msg_1 = self.teacher_syn.score(code)
        pnl, scores_2, error_msg_2 = self.teacher_sem.score(graph)
        scores = scores_1 | scores_2
        error_msg = error_msg_1 + "\n" + error_msg_2
        error_msg = "\n".join(set(error_msg.split("\n")))
        return scores, error_msg


def adapter(grader, resp: Dict):
    # Convert single chat (result of finetune-02) into chained history (format of finetune-03)
    content = resp["message"]["content"]

    scores = {}
    code = ""
    thought = ""
    error_msg = ""
    try:
        content_json = json.loads(content)
        thought = content_json["thought"]
        code = content_json["code"]

        scores, error_msg = grader.check(code)
    except:
        pass

    info = {
        "desired_response": resp["desired_response"],
        "history": [
            {
                "response": resp,
                "scores": scores,
                "code": code,
                "code_error_msg": error_msg,
                "code_generation_thought": thought,
            }
        ]
    }
    return info


def score_summary(scores: Dict):
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
    elif abs(scores["ret"]/(scores["std"]+scores["ret"]+1e-9)) < 0.001:
        information = 2.
    elif abs(scores["ret"]/(scores["std"]+scores["ret"]+1e-9)) < 0.01:
        information = 3.
    elif abs(scores["ret"]/(scores["std"]+scores["ret"]+1e-9)) < 0.03:
        information = 4.
    elif abs(scores["ret"]/(scores["std"]+scores["ret"]+1e-9)) >= 0.03:
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

    scores_summary = f"""syntax-lex : {scores['lex']}
syntax-parse : {scores['parse']}
syntax-type_check : {scores['type_check']}
syntax-runtime : {scores['build']}
semantics-position_balance : {position_balance}
semantics-information : {information}
semantics-turnover : {turnover}
"""
    return scores_summary


def improve(step2_result, grader, args, syntax, stdlib, prompt_config):
    # Improve and add it to the history
    system_context = prompt_config["system"]
    system_context = system_context.replace("{language_name}", "ButterFlow")
    system_context = system_context.replace(
        "{language_spec}", syntax + "\n" + stdlib)
    user_prompt = prompt_config["user"]
    last_history = step2_result["history"][-1]
    code = last_history["code"]
    error_msg = last_history["code_error_msg"]
    user_prompt = user_prompt.replace(
        "{pseudocode}", step2_result["desired_response"])
    user_prompt = user_prompt.replace("{code}", code)
    user_prompt = user_prompt.replace(
        "{quality_score}", score_summary(last_history["scores"]))
    user_prompt = user_prompt.replace("{error_msg}", error_msg)

    resp = query_ollama(endpoint=args.endpoint, model=args.model,
                        prompt=user_prompt, system_prompt=system_context, temperature=args.temperature)
    output_code = resp["message"]["content"]
    output_thinking = resp["message"]["thinking"]

    scores, error_msg = grader.check(output_code)
    step2_result["history"].append(
        {
            "response": resp,
            "scores": scores,
            "code": output_code,
            "code_error_msg": error_msg,
            "code_generation_thought": output_thinking,
        })

    return step2_result


def arg_parse():
    parser = argparse.ArgumentParser(
        description="Reverse-engineer Butterflow code into LLM query")

    parser.add_argument("--input_path", type=str, required=True,
                        default="./data/finetune-02/*.json")
    parser.add_argument("--butterflow_syntax", type=str,
                        required=False, default="./butterflow/docs/syntax.txt")
    parser.add_argument("--butterflow_stdlib", type=str,
                        required=False, default="./butterflow/docs-stdlib/*.txt")
    parser.add_argument("--prompt_file", type=str,
                        required=False, default="./prompts/finetune-03-improve.json")
    parser.add_argument("--endpoint", type=str, required=True,
                        help="Ollama API URL (e.g., http://localhost:11434)")
    parser.add_argument("--temperature", type=float,
                        required=False, default=0.8)
    parser.add_argument("--model", type=str, required=True)
    parser.add_argument("--output_dir", type=str, required=True)

    args = parser.parse_args()

    with open(args.butterflow_syntax, "rt") as f:
        syntax = f.read()

    stdlib = ""
    for path in glob(args.butterflow_stdlib):
        stdlib_doc = load_stdlib_doc(path)
        stdlib += f"{stdlib_doc['name']}: {' '.join(stdlib_doc['syntax'].split())}\n"

    with open(args.prompt_file, "rt") as f:
        prompt_config = json.load(f)

    return args, syntax, stdlib, prompt_config


def main():
    grader = Grader()
    args, syntax, stdlib, prompt_config = arg_parse()

    files = glob(args.input_path)
    random.shuffle(files)
    for i, path in enumerate(files):
        try:
            fname = path.split("/")[-1].split(".")[0]
            if not check_exist(args.output_dir, fname):
                with open(path, "rt") as f:
                    info = json.load(f)
                if "history" not in info:
                    info = adapter(grader, info)
                improved = improve(info, grader, args,
                                   syntax, stdlib, prompt_config)
                check_save(args.output_dir, fname, improved)
            print(f"[{i}/{len(files)}] ", end="\r")
        except KeyboardInterrupt:
            sys.exit()
        except Exception as e:
            print(e)
    print("Done")


# python finetune-03-improve.py --input_path "./data/finetune-02/*.json" --output_dir "./data/finetune-03/" --endpoint "http://192.168.0.23:11434" --model "gpt-oss:120b"
# python finetune-03-improve.py --input_path "./data/finetune-02/*.json" --output_dir "./data/finetune-03/" --endpoint "http://rocm.c-jk.com:11434" --model "gpt-oss:120b"
# for ITER in `seq 100`; do python finetune-03-improve.py --input_path "./data/finetune-03-2/*.json" --output_dir "./data/finetune-03-2/" --endpoint "http://192.168.0.23:11434" --model "gpt-oss:120b"; done
if __name__ == "__main__":
    main()
