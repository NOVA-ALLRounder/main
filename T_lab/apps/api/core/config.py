# =============================================================================
# T_lab Core - Configuration & Settings
# =============================================================================

from functools import lru_cache
from pydantic_settings import BaseSettings
from typing import Optional


class Settings(BaseSettings):
    """Application settings loaded from environment variables."""
    
    # API Keys
    openai_api_key: Optional[str] = None
    tavily_api_key: Optional[str] = None
    
    # Database
    database_url: str = "sqlite:///./tlab.db"
    
    # Redis
    redis_url: str = "redis://localhost:6379/0"
    
    # Server
    api_host: str = "0.0.0.0"
    api_port: int = 8000
    debug: bool = False
    
    # LLM Settings
    default_model: str = "gpt-4o"
    
    # Research Settings
    novelty_threshold: float = 0.6
    max_literature_results: int = 10
    
    class Config:
        env_file = ".env"
        env_file_encoding = "utf-8"
        extra = "ignore"


@lru_cache()
def get_settings() -> Settings:
    return Settings()
