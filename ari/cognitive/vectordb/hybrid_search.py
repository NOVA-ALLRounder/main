"""
Hybrid Search

Dense + Sparse (BM25) 하이브리드 검색
"""

from typing import List, Dict, Any, Optional, Tuple
from dataclasses import dataclass
import numpy as np
from rank_bm25 import BM25Okapi

from .chroma_store import ChromaStore
from .embeddings import EmbeddingModel


@dataclass
class SearchResult:
    """검색 결과"""
    doc_id: str
    document: str
    metadata: Dict[str, Any]
    score: float
    dense_score: float = 0.0
    sparse_score: float = 0.0


class HybridSearcher:
    """
    하이브리드 검색기
    
    Dense (벡터 유사도) + Sparse (BM25) 검색 결합
    """
    
    def __init__(
        self,
        chroma_store: ChromaStore,
        dense_weight: float = 0.7,
        sparse_weight: float = 0.3
    ):
        """
        하이브리드 검색기 초기화
        
        Args:
            chroma_store: ChromaDB 저장소
            dense_weight: Dense 검색 가중치
            sparse_weight: Sparse (BM25) 검색 가중치
        """
        self.store = chroma_store
        self.dense_weight = dense_weight
        self.sparse_weight = sparse_weight
        
        # BM25 index (lazy initialization)
        self._bm25: Optional[BM25Okapi] = None
        self._corpus: List[str] = []
        self._doc_ids: List[str] = []
        self._doc_metadatas: List[Dict[str, Any]] = []
    
    def _build_bm25_index(self):
        """BM25 인덱스 구축"""
        # Get all documents from ChromaDB
        all_ids = self.store.get_all_ids()
        if not all_ids:
            return
        
        result = self.store.get(all_ids)
        
        self._doc_ids = result.get("ids", [])
        self._corpus = result.get("documents", [])
        self._doc_metadatas = result.get("metadatas", [])
        
        # Tokenize documents for BM25
        tokenized_corpus = [self._tokenize(doc) for doc in self._corpus]
        self._bm25 = BM25Okapi(tokenized_corpus)
    
    def _tokenize(self, text: str) -> List[str]:
        """텍스트 토큰화"""
        import re
        # Simple whitespace tokenization with lowercase
        tokens = re.findall(r'\b\w+\b', text.lower())
        return tokens
    
    def _normalize_scores(self, scores: List[float]) -> List[float]:
        """점수 정규화 (0-1 범위)"""
        if not scores:
            return []
        
        min_score = min(scores)
        max_score = max(scores)
        
        if max_score == min_score:
            return [1.0] * len(scores)
        
        return [(s - min_score) / (max_score - min_score) for s in scores]
    
    def search(
        self,
        query: str,
        n_results: int = 10,
        where: Optional[Dict[str, Any]] = None,
        rerank: bool = True
    ) -> List[SearchResult]:
        """
        하이브리드 검색 수행
        
        Args:
            query: 검색 쿼리
            n_results: 반환할 결과 수
            where: 메타데이터 필터
            rerank: 결과 재정렬 여부
        
        Returns:
            검색 결과 리스트
        """
        # Ensure BM25 index is built
        if self._bm25 is None:
            self._build_bm25_index()
        
        if not self._corpus:
            return []
        
        # Dense search (ChromaDB)
        dense_results = self.store.search(
            query=query,
            n_results=min(n_results * 2, len(self._corpus)),
            where=where
        )
        
        # Create score maps
        dense_scores = {}
        for i, doc_id in enumerate(dense_results["ids"]):
            # Convert distance to similarity (ChromaDB returns cosine distance)
            distance = dense_results["distances"][i]
            similarity = 1 - distance  # Cosine similarity
            dense_scores[doc_id] = similarity
        
        # Sparse search (BM25)
        query_tokens = self._tokenize(query)
        bm25_scores = self._bm25.get_scores(query_tokens)
        
        sparse_scores = {}
        for i, doc_id in enumerate(self._doc_ids):
            sparse_scores[doc_id] = bm25_scores[i]
        
        # Normalize scores
        dense_vals = list(dense_scores.values())
        sparse_vals = list(sparse_scores.values())
        
        if dense_vals:
            dense_normalized = self._normalize_scores(dense_vals)
            for i, doc_id in enumerate(dense_scores.keys()):
                dense_scores[doc_id] = dense_normalized[i]
        
        if sparse_vals:
            sparse_normalized = self._normalize_scores(sparse_vals)
            for i, doc_id in enumerate(sparse_scores.keys()):
                sparse_scores[doc_id] = sparse_normalized[i]
        
        # Combine scores
        all_doc_ids = set(dense_scores.keys()) | set(sparse_scores.keys())
        combined_scores = {}
        
        for doc_id in all_doc_ids:
            dense = dense_scores.get(doc_id, 0.0)
            sparse = sparse_scores.get(doc_id, 0.0)
            combined = self.dense_weight * dense + self.sparse_weight * sparse
            combined_scores[doc_id] = (combined, dense, sparse)
        
        # Sort by combined score
        sorted_ids = sorted(
            combined_scores.keys(),
            key=lambda x: combined_scores[x][0],
            reverse=True
        )[:n_results]
        
        # Build results
        results = []
        for doc_id in sorted_ids:
            idx = self._doc_ids.index(doc_id) if doc_id in self._doc_ids else -1
            if idx >= 0:
                combined, dense, sparse = combined_scores[doc_id]
                results.append(SearchResult(
                    doc_id=doc_id,
                    document=self._corpus[idx],
                    metadata=self._doc_metadatas[idx],
                    score=combined,
                    dense_score=dense,
                    sparse_score=sparse
                ))
        
        return results
    
    def search_dense_only(
        self,
        query: str,
        n_results: int = 10,
        where: Optional[Dict[str, Any]] = None
    ) -> List[SearchResult]:
        """Dense 검색만 수행"""
        results = self.store.search(query, n_results, where)
        
        search_results = []
        for i, doc_id in enumerate(results["ids"]):
            search_results.append(SearchResult(
                doc_id=doc_id,
                document=results["documents"][i],
                metadata=results["metadatas"][i],
                score=1 - results["distances"][i],
                dense_score=1 - results["distances"][i],
                sparse_score=0.0
            ))
        
        return search_results
    
    def search_sparse_only(
        self,
        query: str,
        n_results: int = 10
    ) -> List[SearchResult]:
        """Sparse (BM25) 검색만 수행"""
        if self._bm25 is None:
            self._build_bm25_index()
        
        if not self._corpus:
            return []
        
        query_tokens = self._tokenize(query)
        scores = self._bm25.get_scores(query_tokens)
        
        # Get top results
        top_indices = np.argsort(scores)[::-1][:n_results]
        
        results = []
        for idx in top_indices:
            if scores[idx] > 0:
                results.append(SearchResult(
                    doc_id=self._doc_ids[idx],
                    document=self._corpus[idx],
                    metadata=self._doc_metadatas[idx],
                    score=scores[idx],
                    dense_score=0.0,
                    sparse_score=scores[idx]
                ))
        
        return results
    
    def rebuild_index(self):
        """인덱스 재구축"""
        self._bm25 = None
        self._corpus = []
        self._doc_ids = []
        self._doc_metadatas = []
        self._build_bm25_index()
