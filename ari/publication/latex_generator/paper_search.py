"""
Paper Search Module

Semantic Scholar API를 사용한 실제 논문 검색
가짜 인용 대신 100% 실제 논문만 사용
"""

import os
import re
import time
import logging
from typing import List, Dict, Any, Optional
from dataclasses import dataclass

# HTTP 요청용
try:
    import requests
    HAS_REQUESTS = True
except ImportError:
    HAS_REQUESTS = False

logger = logging.getLogger(__name__)


@dataclass
class PaperResult:
    """검색된 논문 결과"""
    paper_id: str
    title: str
    authors: List[str]
    year: int
    abstract: Optional[str] = None
    venue: Optional[str] = None
    citation_count: int = 0
    doi: Optional[str] = None
    url: Optional[str] = None
    
    def to_bibtex_dict(self) -> Dict[str, Any]:
        """BibTeX 딕셔너리로 변환"""
        # 첫 저자 성으로 key 생성
        if self.authors:
            first_author = self.authors[0]
            last_name = first_author.split()[-1].lower()
            last_name = re.sub(r'[^a-z]', '', last_name)
        else:
            last_name = "unknown"
        
        key = f"{last_name}{self.year}"
        
        return {
            "key": key,
            "entry_type": "article" if self.venue and "journal" in self.venue.lower() else "inproceedings",
            "title": self.title,
            "authors": self.authors,
            "year": self.year,
            "journal": self.venue if self.venue else None,
            "doi": self.doi,
            "url": self.url or f"https://www.semanticscholar.org/paper/{self.paper_id}",
            "abstract": self.abstract
        }


class SemanticScholarClient:
    """
    Semantic Scholar API 클라이언트
    
    무료 API 사용 (API 키 없이도 동작, rate limit 있음)
    https://api.semanticscholar.org/api-docs/
    """
    
    BASE_URL = "https://api.semanticscholar.org/graph/v1"
    
    def __init__(self, api_key: Optional[str] = None):
        """
        Args:
            api_key: Semantic Scholar API 키 (선택사항, 있으면 rate limit 완화)
        """
        self.api_key = api_key or os.getenv("SEMANTIC_SCHOLAR_API_KEY")
        self.session = requests.Session() if HAS_REQUESTS else None
        
        if self.session and self.api_key:
            self.session.headers["x-api-key"] = self.api_key
    
    def search_papers(
        self,
        query: str,
        limit: int = 10,
        fields: List[str] = None,
        year_range: Optional[tuple] = None
    ) -> List[PaperResult]:
        """
        논문 검색
        
        Args:
            query: 검색 쿼리 (키워드, 제목 등)
            limit: 최대 결과 수
            fields: 가져올 필드 목록
            year_range: (시작년, 끝년) 튜플
            
        Returns:
            PaperResult 리스트
        """
        if not self.session:
            logger.warning("requests 라이브러리가 설치되지 않음. 빈 결과 반환.")
            return []
        
        # 기본 필드
        if fields is None:
            fields = ["paperId", "title", "authors", "year", "abstract", 
                     "venue", "citationCount", "externalIds", "url"]
        
        params = {
            "query": query,
            "limit": min(limit, 100),  # API 최대 100개
            "fields": ",".join(fields)
        }
        
        # 연도 필터
        if year_range:
            params["year"] = f"{year_range[0]}-{year_range[1]}"
        
        try:
            response = self.session.get(
                f"{self.BASE_URL}/paper/search",
                params=params,
                timeout=30
            )
            response.raise_for_status()
            data = response.json()
            
            results = []
            for paper in data.get("data", []):
                result = self._parse_paper(paper)
                if result:
                    results.append(result)
            
            return results
            
        except requests.RequestException as e:
            logger.error(f"Semantic Scholar API 오류: {e}")
            return []
    
    def get_paper_details(self, paper_id: str) -> Optional[PaperResult]:
        """논문 상세 정보 조회"""
        if not self.session:
            return None
        
        fields = ["paperId", "title", "authors", "year", "abstract",
                 "venue", "citationCount", "externalIds", "url"]
        
        try:
            response = self.session.get(
                f"{self.BASE_URL}/paper/{paper_id}",
                params={"fields": ",".join(fields)},
                timeout=30
            )
            response.raise_for_status()
            return self._parse_paper(response.json())
            
        except requests.RequestException as e:
            logger.error(f"논문 상세 조회 실패: {e}")
            return None
    
    def _parse_paper(self, data: Dict[str, Any]) -> Optional[PaperResult]:
        """API 응답을 PaperResult로 변환"""
        try:
            # 저자 이름 추출
            authors = []
            for author in data.get("authors", []):
                name = author.get("name", "")
                if name:
                    authors.append(name)
            
            # DOI 추출
            external_ids = data.get("externalIds", {}) or {}
            doi = external_ids.get("DOI")
            
            # 연도가 없으면 스킵
            year = data.get("year")
            if not year:
                return None
            
            return PaperResult(
                paper_id=data.get("paperId", ""),
                title=data.get("title", ""),
                authors=authors,
                year=year,
                abstract=data.get("abstract"),
                venue=data.get("venue"),
                citation_count=data.get("citationCount", 0),
                doi=doi,
                url=data.get("url")
            )
        except Exception as e:
            logger.warning(f"논문 파싱 실패: {e}")
            return None


class RealPaperSearcher:
    """
    실제 논문 검색기
    
    여러 소스를 통합하여 가설에 맞는 실제 논문을 검색
    """
    
    def __init__(self):
        self.semantic_scholar = SemanticScholarClient()
        self._cache: Dict[str, List[PaperResult]] = {}
    
    def search_for_hypothesis(
        self,
        hypothesis_title: str,
        keywords: List[str] = None,
        domain: str = "",
        num_papers: int = 8,
        min_citations: int = 5,
        year_range: tuple = (2018, 2025)
    ) -> List[PaperResult]:
        """
        가설에 맞는 실제 논문 검색
        
        Args:
            hypothesis_title: 가설/논문 제목
            keywords: 추가 키워드
            domain: 도메인 (ML, 통신 등)
            num_papers: 필요한 논문 수
            min_citations: 최소 인용수 필터
            year_range: 검색 연도 범위
            
        Returns:
            실제 논문 리스트
        """
        keywords = keywords or []
        all_papers = []
        seen_ids = set()
        
        # 검색 쿼리 생성
        queries = self._generate_search_queries(hypothesis_title, keywords, domain)
        
        for query in queries:
            if len(all_papers) >= num_papers:
                break
            
            # 캐시 확인
            cache_key = f"{query}_{year_range}"
            if cache_key in self._cache:
                papers = self._cache[cache_key]
            else:
                papers = self.semantic_scholar.search_papers(
                    query=query,
                    limit=20,
                    year_range=year_range
                )
                self._cache[cache_key] = papers
                time.sleep(0.5)  # Rate limit 방지
            
            # 중복 제거 및 품질 필터
            for paper in papers:
                if paper.paper_id in seen_ids:
                    continue
                if paper.citation_count < min_citations:
                    continue
                if not paper.title or not paper.authors:
                    continue
                
                seen_ids.add(paper.paper_id)
                all_papers.append(paper)
                
                if len(all_papers) >= num_papers:
                    break
        
        # 인용수로 정렬 (영향력 높은 논문 우선)
        all_papers.sort(key=lambda p: p.citation_count, reverse=True)
        
        return all_papers[:num_papers]
    
    def _generate_search_queries(
        self,
        title: str,
        keywords: List[str],
        domain: str
    ) -> List[str]:
        """검색 쿼리 생성"""
        queries = []
        
        # 1. 제목에서 핵심 용어 추출
        title_words = re.findall(r'\b[A-Za-z]{4,}\b', title)
        if title_words:
            # 상위 3-4개 단어로 쿼리
            queries.append(" ".join(title_words[:4]))
        
        # 2. 키워드 조합
        if keywords:
            queries.append(" ".join(keywords[:3]))
        
        # 3. 도메인 특화 쿼리
        domain_terms = {
            "통신": ["wireless communication", "MIMO", "5G"],
            "Machine Learning": ["deep learning", "neural network"],
            "Computer Vision": ["image recognition", "object detection"],
            "NLP": ["natural language processing", "transformer"],
            "의료": ["medical imaging", "clinical AI"],
        }
        
        if domain in domain_terms:
            for term in domain_terms[domain][:2]:
                if keywords:
                    queries.append(f"{term} {keywords[0]}")
                else:
                    queries.append(term)
        
        return queries[:5]  # 최대 5개 쿼리
    
    def to_bibliography_entries(self, papers: List[PaperResult]) -> List[Dict[str, Any]]:
        """PaperResult 리스트를 BibTeX 딕셔너리 리스트로 변환"""
        entries = []
        seen_keys = set()
        
        for paper in papers:
            entry = paper.to_bibtex_dict()
            
            # 키 중복 방지
            base_key = entry["key"]
            key = base_key
            counter = 1
            while key in seen_keys:
                key = f"{base_key}{chr(ord('a') + counter - 1)}"
                counter += 1
            
            entry["key"] = key
            seen_keys.add(key)
            entries.append(entry)
        
        return entries
