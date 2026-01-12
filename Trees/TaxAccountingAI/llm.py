import os
import subprocess
import sys
from pathlib import Path
from typing import Optional

# Configuration - Relative paths to AI-summary resources
BASE_DIR = Path(__file__).resolve().parent
AI_SUMMARY_DIR = BASE_DIR.parent / "AI-summary"
MODELS_DIR = AI_SUMMARY_DIR / "models"
GGUF_DIR = MODELS_DIR / "gguf"
LLAMA_CPP_DIR = MODELS_DIR / "llama.cpp"

# Target Model
MODEL_NAME = "gemma-3-4b-it-Q4_K_M.gguf"
MODEL_PATH = GGUF_DIR / MODEL_NAME

def get_llama_cli() -> Optional[Path]:
    """Find the llama-cli executable."""
    # Priority: Metal (Mac) > CPU
    metal_cli = LLAMA_CPP_DIR / "build_metal" / "bin" / "llama-cli"
    cpu_cli = LLAMA_CPP_DIR / "build_cpu" / "bin" / "llama-cli"
    
    if metal_cli.exists():
        return metal_cli
    if cpu_cli.exists():
        return cpu_cli
    
    return None

def generate_response(prompt: str, system_prompt: str = "") -> str:
    """Generate a response using the local Gemma model."""
    
    cli_path = get_llama_cli()
    if not cli_path:
        raise FileNotFoundError("Could not find llama-cli. Please ensure AI-summary project dependencies are built.")
        
    if not MODEL_PATH.exists():
        raise FileNotFoundError(f"Could not find model at {MODEL_PATH}")

    full_prompt = f"<start_of_turn>user\n{prompt}<end_of_turn>\n<start_of_turn>model\n"
    if system_prompt:
        full_prompt = f"<start_of_turn>system\n{system_prompt}<end_of_turn>\n" + full_prompt

    # Command arguments for llama-cli
    cmd = [
        str(cli_path),
        "-m", str(MODEL_PATH),
        "-p", full_prompt,
        "-n", "1024",           # Max tokens
        "--temp", "0.7",      # Temperature
        "-c", "4096",         # Context window
        "--no-display-prompt",
        "--simple-io",
        "-ngl", "99"          # GPU layers (try max offload)
        
    ]
    
    # Run subprocess
    try:
        result = subprocess.run(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            encoding="utf-8",
            check=False 
        )
        
        if result.returncode != 0:
            print(f"Error executing model: {result.stderr}", file=sys.stderr)
            return f"Error: {result.stderr}"
            
        return result.stdout.strip()
        
    except Exception as e:
        return f"Exception occurred: {str(e)}"

if __name__ == "__main__":
    # Test block
    print("Testing Gemma connection...")
    response = generate_response("Hello, are you ready to help with tax questions?")
    print("-" * 20)
    print(response)
    print("-" * 20)
