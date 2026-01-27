"""
Base Crawler Classes

크롤러 베이스 클래스 및 공통 데이터 모델
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import List, Optional, Dict, Any
from datetime import datetime


@dataclass
class PaperSearchResult:
    """논문 검색 결과 데이터 클래스"""
    
    # 기본 정보
    paper_id: str
    title: str
    abstract: str
    authors: List[str]
    
    # 출판 정보
    published_date: Optional[datetime] = None
    journal: Optional[str] = None
    doi: Optional[str] = None
    
    # 링크
    url: Optional[str] = None
    pdf_url: Optional[str] = None
    
    # 분류
    categories: List[str] = field(default_factory=list)
    keywords: List[str] = field(default_factory=list)
    
    # 인용 정보
    citation_count: int = 0
    references: List[str] = field(default_factory=list)
    
    # 메타데이터
    source: str = ""  # arxiv, semantic_scholar, pubmed
    raw_data: Dict[str, Any] = field(default_factory=dict)
    
    def to_dict(self) -> Dict[str, Any]:
        """딕셔너리로 변환"""
        return {
            "paper_id": self.paper_id,
            "title": self.title,
            "abstract": self.abstract,
            "authors": self.authors,
            "published_date": self.published_date.isoformat() if self.published_date else None,
            "journal": self.journal,
            "doi": self.doi,
            "url": self.url,
            "pdf_url": self.pdf_url,
            "categories": self.categories,
            "keywords": self.keywords,
            "citation_count": self.citation_count,
            "references": self.references,
            "source": self.source,
        }
    
    def get_full_text(self) -> str:
        """검색/임베딩용 전체 텍스트 반환"""
        parts = [self.title]
        if self.abstract:
            parts.append(self.abstract)
        if self.keywords:
            parts.append(f"Keywords: {', '.join(self.keywords)}")
        return "\n\n".join(parts)


class BaseCrawler(ABC):
    """크롤러 베이스 클래스"""
    
    def __init__(
        self,
        rate_limit_delay: float = 1.0,
        max_retries: int = 3,
        timeout: int = 30
    ):
        self.rate_limit_delay = rate_limit_delay
        self.max_retries = max_retries
        self.timeout = timeout
    
    @abstractmethod
    async def search(
        self,
        query: str,
        max_results: int = 10,
        **kwargs
    ) -> List[PaperSearchResult]:
        """
        논문 검색
        
        Args:
            query: 검색 쿼리
            max_results: 최대 결과 수
            **kwargs: 추가 검색 옵션
        
        Returns:
            검색 결과 리스트
        """
        pass
    
    @abstractmethod
    async def get_paper(self, paper_id: str) -> Optional[PaperSearchResult]:
        """
        단일 논문 조회
        
        Args:
            paper_id: 논문 ID
        
        Returns:
            논문 정보 또는 None
        """
        pass
    
    async def download_pdf(
        self,
        paper: PaperSearchResult,
        save_path: str
    ) -> bool:
        """
        PDF 다운로드
        
        Args:
            paper: 논문 정보
            save_path: 저장 경로
        
        Returns:
            성공 여부
        """
        import aiohttp
        from pathlib import Path
        
        if not paper.pdf_url:
            return False
        
        try:
            async with aiohttp.ClientSession() as session:
                async with session.get(
                    paper.pdf_url,
                    timeout=aiohttp.ClientTimeout(total=self.timeout * 2)
                ) as response:
                    if response.status == 200:
                        Path(save_path).parent.mkdir(parents=True, exist_ok=True)
                        with open(save_path, 'wb') as f:
                            f.write(await response.read())
                        return True
        except Exception:
            pass
        
        return False
