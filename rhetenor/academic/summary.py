# NOTE: This python code is Machine-generated

import argparse
import json
import os
import requests
import time
from typing import List, Dict, Any
from datetime import datetime
import hashlib


def generate_summary_metadata(path, model, prompt_path, **kwargs):
    return {
        "data_type": "summary",
        "last_validated": datetime.today().strftime("%Y-%m-%d"),
        "path": path,
        "model": model,
        "prompt_path": prompt_path,
        **kwargs
    }
# -------------------------------------------------------------------------
# 1. Utils
# -------------------------------------------------------------------------


def chunk_text(text: str, chunk_size: int, overlap: int) -> List[str]:
    """
    Splits text into chunks of `chunk_size` characters with `overlap`.
    """
    if overlap >= chunk_size:
        raise ValueError("Overlap must be smaller than chunk size.")

    chunks = []
    start = 0
    text_len = len(text)

    while start < text_len:
        end = start + chunk_size
        chunk = text[start:end]
        chunks.append(chunk)

        # Stop if we've reached the end
        if end >= text_len:
            break

        # Move the window forward
        start += (chunk_size - overlap)

    return chunks


def hasher(data):
    return hashlib.sha1(data.encode('utf-8')).hexdigest()


def check_exist(output_dir, output_name):
    for ext in [".txt", ".json"]:
        output_filename = f"{os.path.basename(output_name)}{ext}"
        output_path = os.path.join(output_dir, output_filename)
        if os.path.exists(output_path):
            return output_path
    return None


def check_save(output_dir, output_name, content):
    ext = ".json" if type(content) == dict else ".txt"
    output_filename = f"{os.path.basename(output_name)}{ext}"
    output_path = os.path.join(output_dir, output_filename)
    if os.path.exists(output_path):
        raise Exception(f"Output file {output_path} exists. Not overwriting.")
    else:
        if type(content) == str:
            with open(output_path, 'wt', encoding='utf-8') as f:
                f.write(content)
        elif type(content) == dict:
            with open(output_path, 'wt', encoding='utf-8') as f:
                json.dump(content, f, indent=4, ensure_ascii=False)
        else:
            raise Exception(f"Could not save {type(content)}")
    return output_path

# -------------------------------------------------------------------------
# 2. LLM Interaction Functions
# -------------------------------------------------------------------------


def query_ollama(endpoint: str, model: str, prompt: str, system_prompt: str = None) -> Dict[str, Any]:
    """
    Sends a request to the Ollama API (compatible with OpenWebUI).
    """
    # Ensure endpoint ends with the correct path if not provided
    # Standard Ollama generates at /api/generate (for raw completion)
    # or /api/chat (for chat messages). Using /api/chat is usually safer for modern models.

    url = f"{endpoint.rstrip('/')}/api/chat"

    messages = []
    if system_prompt:
        messages.append({"role": "system", "content": system_prompt})
    messages.append({"role": "user", "content": prompt})

    payload = {
        "model": model,
        "messages": messages,
        "stream": False,  # We want the whole response at once
        "options": {
            "temperature": 0.0,  # Keep it deterministic for extraction tasks
            # "num_ctx": 4096     # Adjust context window if necessary
        }
    }

    try:
        response = requests.post(url, json=payload)
        response.raise_for_status()
        return response.json()
    except requests.exceptions.RequestException as e:
        print(f"Error calling LLM: {e}")
        return {"error": str(e), "message": {"content": ""}}


# -------------------------------------------------------------------------
# 3. Main Execution
# -------------------------------------------------------------------------


def main():
    parser = argparse.ArgumentParser(
        description="Process text file with LLM via Ollama/OpenWebUI")

    # Required Arguments
    parser.add_argument("--input_file", type=str, required=True,
                        help="Path to the source text file")
    parser.add_argument("--prompt_file", type=str, required=True,
                        help="Path to JSON file containing prompt template")
    parser.add_argument("--endpoint", type=str, required=True,
                        help="Ollama API URL (e.g., http://localhost:11434)")
    parser.add_argument("--model", type=str, required=True,
                        help="Model name (e.g., deepseek-r1:70b)")
    parser.add_argument("--metadata_dir", type=str,
                        required=True, help="Directory to save the metadata")
    parser.add_argument("--output_dir", type=str, required=True,
                        help="Directory to save the result")

    # Optional Arguments for Chunking
    parser.add_argument("--chunk_size", type=int,
                        default=2000, help="Chunk size in characters")
    parser.add_argument("--overlap", type=int, default=200,
                        help="Overlap size in characters")

    args = parser.parse_args()

    # 1. Setup and Validation
    if not os.path.exists(args.output_dir):
        os.makedirs(args.output_dir)

    # 2. Load Resources
    print(f"Loading text from: {args.input_file}")
    with open(args.input_file, 'r', encoding='utf-8') as f:
        full_text = f.read()

    print(f"Loading prompt template from: {args.prompt_file}")
    with open(args.prompt_file, 'r', encoding='utf-8') as f:
        template_data = json.load(f)
        # Assumes template_json has keys like "system_prompt" (optional) and "user_prompt_template"
        user_template = template_data.get("user_prompt_template", "{text}")
        system_prompt = template_data.get("system_prompt", None)

    # 3. Chunking
    print(
        f"Chunking text (Size: {args.chunk_size}, Overlap: {args.overlap})...")
    chunks = chunk_text(full_text, args.chunk_size, args.overlap)
    print(f"Total chunks created: {len(chunks)}")

    # 4. Processing Loop
    results = []

    for i, chunk in enumerate(chunks):
        print(f"Processing chunk {i+1}/{len(chunks)}...", end="\r")

        # Inject chunk into template
        # We assume the template uses Python string formatting with a {text} placeholder
        formatted_prompt = user_template.replace("{text}", chunk)
        hashed = hasher(repr(formatted_prompt))
        if check_exist(args.metadata_dir, hashed):
            print(f"Skipping {hashed}")
            continue

        # Call LLM
        start_time = time.time()
        llm_response = query_ollama(
            args.endpoint, args.model, formatted_prompt, system_prompt)
        duration = time.time() - start_time

        # Extract content safely
        content = ""
        if "message" in llm_response:
            content = llm_response["message"].get("content", "")

        __log = check_save(args.output_dir, hashed, llm_response)
        summary_path = check_save(args.output_dir, hashed, content)
        info = generate_summary_metadata(
            path=summary_path, model=args.model, prompt_path=args.prompt_file,
            original_path=args.input_file,
            additional_info={"duration": duration}
        )
        __metadata_path = check_save(args.metadata_dir, hashed, info)

    print(f"\r\nDone!")


if __name__ == "__main__":
    main()
