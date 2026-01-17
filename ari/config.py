"""
ARI (Autonomous Research Intelligence) Configuration

환경 변수 및 시스템 설정 관리
"""

import os
from pathlib import Path
from typing import Optional
from pydantic_settings import BaseSettings
from pydantic import Field
from dotenv import load_dotenv

# Load .env from project root
project_root = Path(__file__).parent.parent
env_path = project_root / ".env"
load_dotenv(env_path)


class OpenAISettings(BaseSettings):
    """OpenAI API 설정"""
    api_key: str = Field(default="", alias="OPENAI_API_KEY")
    model: str = "gpt-4o"
    embedding_model: str = "text-embedding-3-small"
    max_tokens: int = 4096
    temperature: float = 0.7
    
    class Config:
        env_file = ".env"
        extra = "ignore"


class VectorDBSettings(BaseSettings):
    """벡터 데이터베이스 설정"""
    persist_directory: str = "./data/chromadb"
    collection_name: str = "ari_papers"
    embedding_dimension: int = 1536
    
    class Config:
        extra = "ignore"


class CrawlerSettings(BaseSettings):
    """크롤러 설정"""
    arxiv_max_results: int = 50
    semantic_scholar_api_key: Optional[str] = Field(default=None, alias="SEMANTIC_SCHOLAR_API_KEY")
    request_timeout: int = 30
    retry_attempts: int = 3
    rate_limit_delay: float = 1.0  # seconds between requests
    
    class Config:
        env_file = ".env"
        extra = "ignore"


class ExperimentSettings(BaseSettings):
    """실험 실행 설정"""
    sandbox_timeout: int = 300  # 5 minutes
    max_debug_attempts: int = 5
    tree_search_max_depth: int = 10
    tree_search_max_nodes: int = 100
    
    class Config:
        extra = "ignore"


class PublicationSettings(BaseSettings):
    """논문 작성 설정"""
    latex_template_dir: str = "./templates/latex"
    output_dir: str = "./output/papers"
    max_review_rounds: int = 3
    
    class Config:
        extra = "ignore"


class ARIConfig(BaseSettings):
    """ARI 전체 설정"""
    # Sub-configs
    openai: OpenAISettings = Field(default_factory=OpenAISettings)
    vectordb: VectorDBSettings = Field(default_factory=VectorDBSettings)
    crawler: CrawlerSettings = Field(default_factory=CrawlerSettings)
    experiment: ExperimentSettings = Field(default_factory=ExperimentSettings)
    publication: PublicationSettings = Field(default_factory=PublicationSettings)
    
    # Global settings
    project_root: Path = Field(default_factory=lambda: Path(__file__).parent.parent)
    data_dir: Path = Field(default_factory=lambda: Path(__file__).parent / "data")
    output_dir: Path = Field(default_factory=lambda: Path(__file__).parent / "output")
    log_level: str = "INFO"
    
    class Config:
        env_file = ".env"
        extra = "ignore"
    
    def __init__(self, **data):
        super().__init__(**data)
        # Ensure directories exist
        self.data_dir.mkdir(parents=True, exist_ok=True)
        self.output_dir.mkdir(parents=True, exist_ok=True)
        (self.output_dir / "papers").mkdir(parents=True, exist_ok=True)
        (self.output_dir / "experiments").mkdir(parents=True, exist_ok=True)
        (self.output_dir / "logs").mkdir(parents=True, exist_ok=True)


# Global config instance
_config: Optional[ARIConfig] = None


def get_config() -> ARIConfig:
    """Get the global ARI configuration instance."""
    global _config
    if _config is None:
        _config = ARIConfig()
    return _config


def reload_config() -> ARIConfig:
    """Reload configuration from environment."""
    global _config
    _config = ARIConfig()
    return _config
