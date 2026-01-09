import argparse
from glob import glob
import json
import random
from finetune_util import hasher, load_stdlib_doc, query_ollama, check_save
import os


def expand():
    parser = argparse.ArgumentParser(
        description="Expand")

    parser.add_argument("--input_dir", type=str,
                        required=False, default="./data/finetune-01/*.json")
    parser.add_argument("--butterflow_syntax", type=str,
                        required=False, default="./butterflow/docs/syntax.txt")
    parser.add_argument("--butterflow_stdlib", type=str,
                        required=False, default="./butterflow/docs-stdlib/*.txt")
    parser.add_argument("--prompt_file", type=str,
                        required=False, default="./prompts/finetune-02-expand.json")
    parser.add_argument("--endpoint", type=str, required=True,
                        help="Ollama API URL (e.g., http://localhost:11434)")
    parser.add_argument("--n_stdlib_docs", type=int,
                        required=False, default=10)
    parser.add_argument("--temperature", type=float,
                        required=False, default=0.8)
    parser.add_argument("--model", type=str, required=True)
    parser.add_argument("--output_dir", type=str, required=True)

    args = parser.parse_args()

    with open(args.butterflow_syntax, "rt") as f:
        syntax = f.read()

    syntax += "\n"
    for path in glob(args.butterflow_stdlib):
        stdlib_doc = load_stdlib_doc(path)
        syntax += f"{stdlib_doc['name']}: {' '.join(stdlib_doc['description'].split())}\n"

    # TODO: document embedding / knowledge graph
    stdlib_all = []
    for path in glob(args.butterflow_stdlib):
        stdlib_doc = load_stdlib_doc(path)
        stdlib_all.append(
            f"{stdlib_doc['syntax']} : {' '.join(stdlib_doc['syntax'].split())}\n")

    # Load result of step 01
    codebase = []
    for path in glob(args.input_dir + "/*.json"):
        with open(path, "rt") as f:
            llm_response = json.load(f)
        try:
            llm_result = llm_response["message"]["content"]
            llm_json = json.loads(llm_result)
            llm_prompt = llm_json["user_prompt"]
            path_orig = llm_response["args"]["input_code"]
            with open(path_orig, "rt") as f:
                code = f.read()
            codebase.append(
                {"prompt": llm_prompt, "desired_response": code, "path_finetune-01": path, "path_orig": path_orig})
        except Exception as e:
            print(str(e))
            continue

    # prompt
    with open(args.prompt_file, "rt") as f:
        config = json.load(f)

    # loop
    for i, prompt_response in enumerate(codebase):
        print(f"  [{i}/{len(codebase)}]  ", end="\r")
        prompt = prompt_response["prompt"]
        desired_response = prompt_response["desired_response"]
        path_step1 = prompt_response["path_finetune-01"]
        path_orig = prompt_response["path_orig"]
        # sample document. TODO: document embedding
        rag_stdlib = "".join(random.sample(stdlib_all, args.n_stdlib_docs))
        # FIXME : Using result of finetune-01 as a random feature. May lack coverage.
        random_feature_list = prompt
        {
            "description": "Uses the spec to hallucinate NEW code scenarios, expanding the dataset.",
            "system": "You are a code generator for {language_name}. You will generate valid code based on random semantic combinations from the documentation.\n\nLANGUAGE SPEC:\n{language_spec}\nSTANDARD LIB:\n{rag_stdlib}",
            "user": "Generate a valid source code snippet that utilizes the following language features: {random_feature_list}.\n1. First, describe the goal in natural language (`thought`).\n2. Then, write the code (`code`).\n\nResponse format (JSON):\n{\n  \"thought\": \"...\",\n  \"code\": \"...\"\n}"
        }
        # prompt building
        system_context = config["system"]
        system_context = system_context.replace(
            "{language_name}", "ButterFlow")
        system_context = system_context.replace("{language_spec}", syntax)
        system_context = system_context.replace("{rag_stdlib}", rag_stdlib)
        user_prompt = config["user"]
        user_prompt = user_prompt.replace(
            "{random_feature_list}", random_feature_list)

        result = query_ollama(endpoint=args.endpoint, model=args.model,
                              prompt=user_prompt, system_prompt=system_context, temperature=args.temperature)
        result["args"] = vars(args)
        result["system_context"] = system_context
        result["user_prompt"] = user_prompt
        result["path_finetune-01"] = path_step1
        result["path_orig"] = path_orig
        result["desired_response"] = desired_response
        code_name = os.path.splitext(os.path.split(path_orig)[-1])[0]
        output_name = code_name + "_" + hasher(repr(result))
        check_save(args.output_dir, output_name, result)
    print("\n  Done  ")


if __name__ == "__main__":
    # python finetune-02-expand.py --input_dir ./data/finetune-01/ --endpoint http://rocm.c-jk.com:11434 --model gpt-oss:120b --output_dir ./data/finetune-02/
    # python finetune-02-expand.py --input_dir ./data/finetune-01/ --endpoint http://192.168.0.23:11434 --model gpt-oss:120b --output_dir ./data/finetune-02/
    expand()
