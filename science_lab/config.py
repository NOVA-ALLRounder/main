"""
Configuration module for Autonomous Science Discovery System
"""
import os
from pathlib import Path
from dotenv import load_dotenv

# Load environment variables
dotenv_path = Path(__file__).parent.parent / ".env"
load_dotenv(dotenv_path=dotenv_path)

# Base paths
BASE_DIR = Path(__file__).parent
DATA_DIR = BASE_DIR / "data"
REPORTS_DIR = BASE_DIR / "reports"

# Create directories if not exist
DATA_DIR.mkdir(exist_ok=True)
REPORTS_DIR.mkdir(exist_ok=True)

# API Keys
OPENAI_API_KEY = os.getenv("OPENAI_API_KEY", "")
SEMANTIC_SCHOLAR_API_KEY = os.getenv("SEMANTIC_SCHOLAR_API_KEY", "")

# LLM Configuration
LLM_MODEL = "gpt-4o-mini"  # 비용 효율적인 모델
LLM_TEMPERATURE = 0.7

# Database
DB_PATH = DATA_DIR / "science_lab.db"
VECTOR_DB_PATH = DATA_DIR / "chroma_db"

# Experiment settings
MAX_RETRY_COUNT = 5  # 자가 치유 디버깅 최대 재시도 횟수
CODE_EXECUTION_TIMEOUT = 60  # 코드 실행 타임아웃 (초)

# Novelty threshold
NOVELTY_THRESHOLD = 0.85  # 의미론적 유사도 임계값

# Server
HOST = "0.0.0.0"
PORT = 8000
