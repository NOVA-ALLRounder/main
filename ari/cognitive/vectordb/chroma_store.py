"""
ChromaDB Store

ChromaDB 기반 벡터 저장소
"""

from typing import List, Dict, Any, Optional
from pathlib import Path
import chromadb
from chromadb.config import Settings

from .embeddings import EmbeddingModel, get_embedding_model


class ChromaStore:
    """ChromaDB 벡터 저장소"""
    
    def __init__(
        self,
        collection_name: str = "ari_papers",
        persist_directory: Optional[str] = None,
        embedding_model: Optional[EmbeddingModel] = None
    ):
        """
        ChromaDB 저장소 초기화
        
        Args:
            collection_name: 컬렉션 이름
            persist_directory: 영구 저장 디렉토리 (None이면 인메모리)
            embedding_model: 임베딩 모델
        """
        self.collection_name = collection_name
        self.embedding_model = embedding_model or get_embedding_model("openai")
        
        # Initialize ChromaDB client
        if persist_directory:
            Path(persist_directory).mkdir(parents=True, exist_ok=True)
            self.client = chromadb.PersistentClient(
                path=persist_directory,
                settings=Settings(anonymized_telemetry=False)
            )
        else:
            self.client = chromadb.Client(
                settings=Settings(anonymized_telemetry=False)
            )
        
        # Get or create collection
        self.collection = self.client.get_or_create_collection(
            name=collection_name,
            metadata={"hnsw:space": "cosine"}
        )
    
    def add(
        self,
        documents: List[str],
        metadatas: Optional[List[Dict[str, Any]]] = None,
        ids: Optional[List[str]] = None
    ) -> List[str]:
        """
        문서 추가
        
        Args:
            documents: 문서 텍스트 리스트
            metadatas: 메타데이터 리스트
            ids: 문서 ID 리스트
        
        Returns:
            추가된 문서 ID 리스트
        """
        if not documents:
            return []
        
        # Generate IDs if not provided
        if ids is None:
            import hashlib
            ids = [
                hashlib.md5(doc.encode()).hexdigest()[:16]
                for doc in documents
            ]
        
        # Generate embeddings
        embeddings = self.embedding_model.embed(documents)
        
        # Prepare metadatas
        if metadatas is None:
            metadatas = [{}] * len(documents)
        
        # Add to collection
        self.collection.add(
            documents=documents,
            embeddings=embeddings,
            metadatas=metadatas,
            ids=ids
        )
        
        return ids
    
    def search(
        self,
        query: str,
        n_results: int = 10,
        where: Optional[Dict[str, Any]] = None,
        include_embeddings: bool = False
    ) -> Dict[str, Any]:
        """
        유사도 검색
        
        Args:
            query: 검색 쿼리
            n_results: 반환할 결과 수
            where: 메타데이터 필터
            include_embeddings: 임베딩 포함 여부
        
        Returns:
            검색 결과 {ids, documents, metadatas, distances}
        """
        # Generate query embedding
        query_embedding = self.embedding_model.embed_single(query)
        
        # Prepare include list
        include = ["documents", "metadatas", "distances"]
        if include_embeddings:
            include.append("embeddings")
        
        # Query collection
        results = self.collection.query(
            query_embeddings=[query_embedding],
            n_results=n_results,
            where=where,
            include=include
        )
        
        # Flatten results
        return {
            "ids": results["ids"][0] if results["ids"] else [],
            "documents": results["documents"][0] if results["documents"] else [],
            "metadatas": results["metadatas"][0] if results["metadatas"] else [],
            "distances": results["distances"][0] if results["distances"] else [],
        }
    
    def search_by_embedding(
        self,
        embedding: List[float],
        n_results: int = 10,
        where: Optional[Dict[str, Any]] = None
    ) -> Dict[str, Any]:
        """임베딩으로 직접 검색"""
        results = self.collection.query(
            query_embeddings=[embedding],
            n_results=n_results,
            where=where,
            include=["documents", "metadatas", "distances"]
        )
        
        return {
            "ids": results["ids"][0] if results["ids"] else [],
            "documents": results["documents"][0] if results["documents"] else [],
            "metadatas": results["metadatas"][0] if results["metadatas"] else [],
            "distances": results["distances"][0] if results["distances"] else [],
        }
    
    def get(
        self,
        ids: List[str],
        include_embeddings: bool = False
    ) -> Dict[str, Any]:
        """ID로 문서 조회"""
        include = ["documents", "metadatas"]
        if include_embeddings:
            include.append("embeddings")
        
        return self.collection.get(
            ids=ids,
            include=include
        )
    
    def update(
        self,
        ids: List[str],
        documents: Optional[List[str]] = None,
        metadatas: Optional[List[Dict[str, Any]]] = None
    ):
        """문서 업데이트"""
        update_kwargs = {"ids": ids}
        
        if documents:
            update_kwargs["documents"] = documents
            update_kwargs["embeddings"] = self.embedding_model.embed(documents)
        
        if metadatas:
            update_kwargs["metadatas"] = metadatas
        
        self.collection.update(**update_kwargs)
    
    def delete(self, ids: List[str]):
        """문서 삭제"""
        self.collection.delete(ids=ids)
    
    def count(self) -> int:
        """문서 수 반환"""
        return self.collection.count()
    
    def clear(self):
        """컬렉션 비우기"""
        # Delete and recreate collection
        self.client.delete_collection(self.collection_name)
        self.collection = self.client.create_collection(
            name=self.collection_name,
            metadata={"hnsw:space": "cosine"}
        )
    
    def get_all_ids(self) -> List[str]:
        """모든 문서 ID 반환"""
        result = self.collection.get(include=[])
        return result.get("ids", [])
