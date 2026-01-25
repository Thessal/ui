from glob import glob
from datetime import datetime
import json
import os
from typing import Dict
import butterflow
import morpho
import json
from glob import glob
from butterflow import lex, Parser, TypeChecker, Builder, Runtime
import numpy as np

# Utility to batch generate metadata, when the spec has been changed


class MetadataManager():
    def __init__(self):
        self.metadata_dir = "./metadata"
        self.paths, self.cursor = self.load_metadata_all()
        self.print_state()

    # Load metadata database
    def load_metadata_all(self):
        files = sorted(glob(self.metadata_dir+"/*.json"))
        indices = sorted(
            [int(os.path.splitext(os.path.split(x)[-1])[0]) for x in files])
        cursor = indices[-1] if indices else 0
        paths = dict()  # key: document path, value: metadata path
        for file in files:
            with open(file, "rt") as f:
                try:
                    paths[json.load(f)["path"]] = file
                except:
                    raise ValueError(f"Failed to process {file}")
        return paths, cursor

    def print_state(self):
        print(
            f"[Manager] {len(self.paths)} files loaded. last index = {self.cursor}")

    def check_duplicate(self, path):
        if path in self.paths:
            raise ValueError(f"{path} already exist in {self.paths[path]}")

    def save_metadata(self, metadata: Dict):
        self.cursor += 1
        filename = self.metadata_dir+f"/{self.cursor:08d}.json"
        assert not os.path.exists(filename)
        with open(filename, "wt") as f:
            self.paths[metadata["path"]] = filename
            json.dump(metadata, f)


def generate_code_metadata(path, agent, related_path, **kwargs):
    raise NotImplementedError


def generate_summary_metadata(path, model, prompt_path, **kwargs):
    return {
        "data_type": "summary",
        "last_validated": datetime.today().strftime("%Y-%m-%d"),
        "path": path,
        "model": model,
        "prompt_path": prompt_path,
        **kwargs
    }


def generate_document_metadata(path, source, agent, **kwargs):
    return {
        "data_type": "document",
        "last_validated": datetime.today().strftime("%Y-%m-%d"),
        "path": path,
        # "data_source": ["web-conversation", "web-article", "book", "academic-article", "other"],
        "data_source": source,
        # "environment": ["research", "paper-trading", "live"],
        "environment": "research",
        "agent": agent,
        **kwargs
    }


# Prompt template generator, useful when the language spec has been updated.
# Generate new prompt and overwrite the old prompt
# save_prompt_json(build_system_context())
def build_system_context(self):
    with open("docs/syntax.txt", "rt") as f:
        syntax = f.read()
    with open("docs/examples.txt", "rt") as f:
        examples = f.read()
    functions = ""
    for path in glob("docs-stdlib/*.txt"):
        with open(path, "rt") as f:
            lines = f.readlines()
            signature = lines[lines.index("1. Type signature\n")+1]
            functions += signature
    return """You are a quant developer, who implements given logic and python code into in-house language. 

The description of in-house language: 
"""+syntax+"""

Supported functions:
"""+functions+"""
Example code:
"""+examples


def save_prompt_json(system_context, user_prompt, user_prompt_args: Dict):
    config = {
        "system_context": system_context.split("\n"),
        "system_context_args": {},
        "user_prompt": user_prompt.split("\n"),
        "user_prompt_args": user_prompt_args
    }
    with open("prompt.json", "wt") as f:
        json.dump(config, f)
