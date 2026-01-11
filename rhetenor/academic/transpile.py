import butterflow
import morpho
import json
from glob import glob
from butterflow import lex, Parser, TypeChecker, Builder, Runtime
import numpy as np
import argparse
import os
from finetune_util import query_ollama


def load_docs(args):
    with open(args.syntax_file, "rt") as f:
        syntax_doc = f.read()
    stdlib_doc = ""
    for path in glob(args.stdlib_path+"/*.txt"):
        with open(path, "rt") as f:
            lines = f.readlines()
            signature = lines[lines.index("1. Type signature\n")+1]
            stdlib_doc += signature
    return syntax_doc, stdlib_doc


def load_metadata(args):
    # to morpho
    pool = []
    for path in glob(args.input_dir+"/*.json"):
        with open(path, "rt") as f:
            pool.append(json.load(f))
    # Apply selection rule
    rules = [(x.split(":")) for x in args.selection.split(",")]
    if not all((len(x) == 0 or len(x) == 2) for x in rules):
        raise Exception(f"Failed to parse the selection : {args.selection}")
    if rules:
        raise NotImplementedError(f"Not implemented idea selection yet")

    return pool


def user_prompt_build(user_prompt, x):
    return user_prompt.replace("{idea_text}", x)


def syntax_check():
    # to morpho
    pass


def semantics_check():
    # to morpho
    pass


def main():
    parser = argparse.ArgumentParser(
        description="Process text file with LLM via Ollama/OpenWebUI")

    # Required Arguments
    parser.add_argument("--input_dir", type=str,
                        required=True, help="Directory of input metadata")
    parser.add_argument("--selection", type=str, required=True,
                        help="Selection Rule with format: field:condition,field:condition,...")
    parser.add_argument("--prompt_file", type=str, required=True,
                        help="Path to JSON file containing prompt template")
    parser.add_argument("--syntax_file", type=str, required=False,
                        default="./morpho/docs/syntax.txt")
    parser.add_argument("--stdlib_path", type=str, required=False,
                        default="./morpho/docs-stdlib/")
    parser.add_argument("--endpoint", type=str, required=True,
                        help="Ollama API URL (e.g., http://localhost:11434)")
    parser.add_argument("--model", type=str, required=True,
                        help="Model name (e.g., deepseek-r1:70b)")
    parser.add_argument("--output_dir", type=str, required=True,
                        help="Directory to save the result")

    # Optional Arguments for Sampling
    parser.add_argument("--generate_count", type=int,
                        default=100, help="Code Generation Target Count")
    parser.add_argument("--sample_doc_count", type=int,
                        default=3, help="Sampling document count")
    parser.add_argument("--sample_line_count", type=int,
                        default=3, help="Sampling line per document")

    args = parser.parse_args()
    syntax_doc, stdlib_doc = load_docs(args)

    # Load config
    with open(args.prompt_file, "rt") as f:
        config = json.load(f)

    system_context = config["system"]
    system_context = system_context.replace("{syntax}", syntax_doc)
    system_context = system_context.replace("{functions}", stdlib_doc)

    user_prompt_template = config["user"]

    # Initialize pool
    library = morpho.LibraryIndexer(config=config)
    library.load("./proprietary/metadata/")
    # library.embed()
    # library.query(metadata= {"serialized":"test"}, n_results=10)
    transpiler = morpho.Transpiler(config=config, prompt_path="prompt.json")

    pool = load_metadata(args)

    user_prompt_build(user_prompt, x)

    print(f"\r\nDone!")


if __name__ == "__main__":
    raise NotImplemented
    main()
