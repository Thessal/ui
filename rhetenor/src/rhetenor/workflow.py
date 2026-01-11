# TODO: shuffle function document order

import butterflow
import morpho
import json
from glob import glob
from butterflow import lex, Parser, TypeChecker, Builder, Runtime
import numpy as np
from rhetenor.util import generate_code_metadata


class WorkFlow:
    pass


class Transpile(WorkFlow):
    def __init__(self, config_path, metadata_path):
        config = morpho.load_json(config_path)
        self.library = morpho.LibraryIndexer(config=config)
        self.library.load(metadata_path)
        # Chromadb indexing
        # self.library.embed()
        # self.library.query(metadata= {"serialized":"test"}, n_results=10)
        self.transpiler = morpho.Transpiler(
            config=config, prompt_path="prompt.json")

    def run(self):
        all_docs_hash = self.library.get_hash_list()
        for hash in all_docs_hash:
            metadata = self.library.get_by_hash(hash)
            idea_text = metadata["serialized"]
            generation_result = self.transpiler.generate(
                system_context_args=dict(), user_prompt_args={"idea_text": idea_text})

    def save_result(generation_result, orig_metadata):
        # TODO: result into metadata format
        for i, strat in enumerate(generation_result.strategies):
            # output = {
            #     "hash": hash,
            #     "orig_metadata": orig_metadata,
            #     "generation_result": strat.__dict__
            # }
            output_data = "\n".join(
                ["#"+strat.name, "#"+strat.description, strat.code])
            orig_metadata_path = ""
            output_metadata = generate_code_metadata(
                path="TODO", agent="machine#FFFFFF", related_path=orig_metadata_path)

            raise NotImplementedError
            #      with open(f"./generated/{hash}_{i:06d}.json", "wt") as f:
            #         json.dump(output, f)
            # except:
            #     try:
            #         with open(f"./generated/log.txt", "a") as f:
            #             f.write(f"Error: {hash}, {i}\n")
            #     except:
            #         print("Error")
