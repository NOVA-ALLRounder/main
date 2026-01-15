"""
Embedding Models

임베딩 모델 래퍼 클래스
"""

from abc import ABC, abstractmethod
from typing import List, Optional
from openai import OpenAI

import sys
import os
sys.path.append(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
from config import get_config


class EmbeddingModel(ABC):
    """임베딩 모델 베이스 클래스"""
    
    @property
    @abstractmethod
    def dimension(self) -> int:
        """임베딩 벡터 차원"""
        pass
    
    @abstractmethod
    def embed(self, texts: List[str]) -> List[List[float]]:
        """
        텍스트 임베딩 생성
        
        Args:
            texts: 임베딩할 텍스트 리스트
        
        Returns:
            임베딩 벡터 리스트
        """
        pass
    
    def embed_single(self, text: str) -> List[float]:
        """단일 텍스트 임베딩"""
        return self.embed([text])[0]


class OpenAIEmbedding(EmbeddingModel):
    """OpenAI 임베딩 모델"""
    
    MODEL_DIMENSIONS = {
        "text-embedding-3-small": 1536,
        "text-embedding-3-large": 3072,
        "text-embedding-ada-002": 1536,
    }
    
    def __init__(
        self,
        model: str = "text-embedding-3-small",
        api_key: Optional[str] = None
    ):
        config = get_config()
        self.model = model
        self.api_key = api_key or config.openai.api_key
        self.client = OpenAI(api_key=self.api_key)
        self._dimension = self.MODEL_DIMENSIONS.get(model, 1536)
    
    @property
    def dimension(self) -> int:
        return self._dimension
    
    def embed(self, texts: List[str]) -> List[List[float]]:
        """OpenAI API를 사용하여 임베딩 생성"""
        if not texts:
            return []
        
        # Handle batch size limits
        batch_size = 100
        all_embeddings = []
        
        for i in range(0, len(texts), batch_size):
            batch = texts[i:i + batch_size]
            response = self.client.embeddings.create(
                model=self.model,
                input=batch
            )
            batch_embeddings = [item.embedding for item in response.data]
            all_embeddings.extend(batch_embeddings)
        
        return all_embeddings


class LocalEmbedding(EmbeddingModel):
    """로컬 임베딩 모델 (sentence-transformers)"""
    
    def __init__(self, model_name: str = "all-MiniLM-L6-v2"):
        try:
            from sentence_transformers import SentenceTransformer
            self.model = SentenceTransformer(model_name)
            self._dimension = self.model.get_sentence_embedding_dimension()
        except ImportError:
            raise ImportError(
                "sentence-transformers is required for local embeddings. "
                "Install with: pip install sentence-transformers"
            )
    
    @property
    def dimension(self) -> int:
        return self._dimension
    
    def embed(self, texts: List[str]) -> List[List[float]]:
        """로컬 모델로 임베딩 생성"""
        if not texts:
            return []
        
        embeddings = self.model.encode(texts, convert_to_numpy=True)
        return embeddings.tolist()


class ScientificEmbedding(EmbeddingModel):
    """과학 도메인 특화 임베딩 모델 (Specter2, SciBERT 등)"""
    
    MODELS = {
        "specter2": "allenai/specter2",
        "scibert": "allenai/scibert_scivocab_uncased",
        "biobert": "dmis-lab/biobert-base-cased-v1.2",
        "pubmedbert": "microsoft/BiomedNLP-PubMedBERT-base-uncased-abstract-fulltext",
    }
    
    def __init__(self, model_type: str = "specter2"):
        try:
            from sentence_transformers import SentenceTransformer
            model_name = self.MODELS.get(model_type, model_type)
            self.model = SentenceTransformer(model_name)
            self._dimension = self.model.get_sentence_embedding_dimension()
        except ImportError:
            raise ImportError(
                "sentence-transformers is required. "
                "Install with: pip install sentence-transformers"
            )
    
    @property
    def dimension(self) -> int:
        return self._dimension
    
    def embed(self, texts: List[str]) -> List[List[float]]:
        """도메인 특화 임베딩 생성"""
        if not texts:
            return []
        
        embeddings = self.model.encode(texts, convert_to_numpy=True)
        return embeddings.tolist()


# Factory function
def get_embedding_model(
    model_type: str = "openai",
    model_name: Optional[str] = None
) -> EmbeddingModel:
    """
    임베딩 모델 팩토리
    
    Args:
        model_type: 모델 타입 (openai, local, scientific)
        model_name: 모델 이름
    
    Returns:
        EmbeddingModel 인스턴스
    """
    if model_type == "openai":
        return OpenAIEmbedding(model=model_name or "text-embedding-3-small")
    elif model_type == "local":
        return LocalEmbedding(model_name=model_name or "all-MiniLM-L6-v2")
    elif model_type == "scientific":
        return ScientificEmbedding(model_type=model_name or "specter2")
    else:
        raise ValueError(f"Unknown model type: {model_type}")
