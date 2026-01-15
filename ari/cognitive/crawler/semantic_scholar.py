"""
Semantic Scholar API Client

Semantic Scholar API를 사용한 논문 검색, 인용 정보 조회, 신규성 검증
"""

import asyncio
import aiohttp
from typing import List, Optional, Dict, Any
from datetime import datetime

from .base import BaseCrawler, PaperSearchResult


class SemanticScholarClient(BaseCrawler):
    """Semantic Scholar API 클라이언트"""
    
    BASE_URL = "https://api.semanticscholar.org/graph/v1"
    PAPER_FIELDS = [
        "paperId", "title", "abstract", "authors", "year",
        "venue", "externalIds", "url", "publicationDate",
        "citationCount", "referenceCount", "fieldsOfStudy",
        "s2FieldsOfStudy", "openAccessPdf"
    ]
    
    def __init__(
        self,
        api_key: Optional[str] = None,
        rate_limit_delay: float = 1.0,
        max_retries: int = 3,
        timeout: int = 30
    ):
        super().__init__(rate_limit_delay, max_retries, timeout)
        self.api_key = api_key
        self.headers = {}
        if api_key:
            self.headers["x-api-key"] = api_key
    
    async def _request(
        self,
        endpoint: str,
        params: Optional[Dict[str, Any]] = None
    ) -> Optional[Dict[str, Any]]:
        """API 요청 헬퍼"""
        url = f"{self.BASE_URL}/{endpoint}"
        
        async with aiohttp.ClientSession() as session:
            for attempt in range(self.max_retries):
                try:
                    async with session.get(
                        url,
                        params=params,
                        headers=self.headers,
                        timeout=aiohttp.ClientTimeout(total=self.timeout)
                    ) as response:
                        if response.status == 200:
                            return await response.json()
                        elif response.status == 429:  # Rate limit
                            await asyncio.sleep(self.rate_limit_delay * (attempt + 1))
                        else:
                            return None
                except Exception:
                    await asyncio.sleep(self.rate_limit_delay)
        
        return None
    
    async def search(
        self,
        query: str,
        max_results: int = 50,
        year: Optional[str] = None,
        fields_of_study: Optional[List[str]] = None,
        open_access_only: bool = False
    ) -> List[PaperSearchResult]:
        """
        Semantic Scholar 논문 검색
        
        Args:
            query: 검색 쿼리
            max_results: 최대 결과 수 (최대 100)
            year: 출판 연도 필터 (예: "2020-2024", "2023")
            fields_of_study: 분야 필터 (예: ["Computer Science", "Medicine"])
            open_access_only: 오픈 액세스 논문만
        
        Returns:
            검색 결과 리스트
        """
        params = {
            "query": query,
            "limit": min(max_results, 100),
            "fields": ",".join(self.PAPER_FIELDS)
        }
        
        if year:
            params["year"] = year
        if fields_of_study:
            params["fieldsOfStudy"] = ",".join(fields_of_study)
        if open_access_only:
            params["openAccessPdf"] = ""
        
        data = await self._request("paper/search", params)
        
        if not data or "data" not in data:
            return []
        
        papers = []
        for item in data["data"]:
            paper = self._convert_result(item)
            if paper:
                papers.append(paper)
        
        return papers
    
    async def get_paper(self, paper_id: str) -> Optional[PaperSearchResult]:
        """
        논문 ID로 상세 정보 조회
        
        Args:
            paper_id: Semantic Scholar Paper ID, arXiv ID, DOI 등
        
        Returns:
            논문 정보 또는 None
        """
        params = {"fields": ",".join(self.PAPER_FIELDS)}
        data = await self._request(f"paper/{paper_id}", params)
        
        if data:
            return self._convert_result(data)
        return None
    
    async def get_citations(
        self,
        paper_id: str,
        max_results: int = 100
    ) -> List[PaperSearchResult]:
        """
        논문의 인용 문헌 조회
        
        Args:
            paper_id: 논문 ID
            max_results: 최대 결과 수
        
        Returns:
            인용 논문 리스트
        """
        params = {
            "fields": ",".join(self.PAPER_FIELDS),
            "limit": min(max_results, 1000)
        }
        data = await self._request(f"paper/{paper_id}/citations", params)
        
        if not data or "data" not in data:
            return []
        
        papers = []
        for item in data["data"]:
            if "citingPaper" in item:
                paper = self._convert_result(item["citingPaper"])
                if paper:
                    papers.append(paper)
        
        return papers
    
    async def get_references(
        self,
        paper_id: str,
        max_results: int = 100
    ) -> List[PaperSearchResult]:
        """
        논문의 참고 문헌 조회
        
        Args:
            paper_id: 논문 ID
            max_results: 최대 결과 수
        
        Returns:
            참고 논문 리스트
        """
        params = {
            "fields": ",".join(self.PAPER_FIELDS),
            "limit": min(max_results, 1000)
        }
        data = await self._request(f"paper/{paper_id}/references", params)
        
        if not data or "data" not in data:
            return []
        
        papers = []
        for item in data["data"]:
            if "citedPaper" in item:
                paper = self._convert_result(item["citedPaper"])
                if paper:
                    papers.append(paper)
        
        return papers
    
    async def check_novelty(
        self,
        hypothesis: str,
        max_similar: int = 20,
        similarity_threshold: float = 0.8
    ) -> Dict[str, Any]:
        """
        가설의 신규성 검증
        
        Args:
            hypothesis: 검증할 가설/아이디어
            max_similar: 비교할 최대 논문 수
            similarity_threshold: 유사도 임계값
        
        Returns:
            신규성 검증 결과
        """
        # Search for similar papers
        similar_papers = await self.search(hypothesis, max_results=max_similar)
        
        return {
            "hypothesis": hypothesis,
            "similar_papers": [p.to_dict() for p in similar_papers],
            "num_similar": len(similar_papers),
            "is_novel": len(similar_papers) < 5,  # Simple heuristic
            "most_similar": similar_papers[0].to_dict() if similar_papers else None
        }
    
    def _convert_result(self, data: Dict[str, Any]) -> Optional[PaperSearchResult]:
        """API 응답을 PaperSearchResult로 변환"""
        if not data or not data.get("title"):
            return None
        
        # Parse publication date
        pub_date = None
        if data.get("publicationDate"):
            try:
                pub_date = datetime.fromisoformat(data["publicationDate"])
            except ValueError:
                pass
        elif data.get("year"):
            pub_date = datetime(year=data["year"], month=1, day=1)
        
        # Extract PDF URL
        pdf_url = None
        if data.get("openAccessPdf"):
            pdf_url = data["openAccessPdf"].get("url")
        
        # Extract external IDs
        external_ids = data.get("externalIds", {})
        doi = external_ids.get("DOI")
        arxiv_id = external_ids.get("ArXiv")
        
        # Extract fields of study
        fields = []
        if data.get("fieldsOfStudy"):
            fields = data["fieldsOfStudy"]
        elif data.get("s2FieldsOfStudy"):
            fields = [f.get("category", "") for f in data["s2FieldsOfStudy"]]
        
        return PaperSearchResult(
            paper_id=data.get("paperId", ""),
            title=data.get("title", ""),
            abstract=data.get("abstract", ""),
            authors=[a.get("name", "") for a in data.get("authors", [])],
            published_date=pub_date,
            journal=data.get("venue"),
            doi=doi,
            url=data.get("url"),
            pdf_url=pdf_url,
            categories=fields,
            keywords=[],
            citation_count=data.get("citationCount", 0),
            references=[],
            source="semantic_scholar",
            raw_data=data
        )
