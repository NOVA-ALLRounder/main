"""
arXiv Crawler

arXiv API를 사용한 논문 검색 및 다운로드
"""

import asyncio
import arxiv
from typing import List, Optional
from datetime import datetime

from .base import BaseCrawler, PaperSearchResult


class ArxivCrawler(BaseCrawler):
    """arXiv 논문 크롤러"""
    
    def __init__(
        self,
        rate_limit_delay: float = 3.0,  # arXiv recommends 3 seconds between requests
        max_retries: int = 3,
        timeout: int = 30
    ):
        super().__init__(rate_limit_delay, max_retries, timeout)
        self.client = arxiv.Client(
            page_size=100,
            delay_seconds=rate_limit_delay,
            num_retries=max_retries
        )
    
    async def search(
        self,
        query: str,
        max_results: int = 50,
        sort_by: str = "relevance",
        sort_order: str = "descending",
        categories: Optional[List[str]] = None
    ) -> List[PaperSearchResult]:
        """
        arXiv 논문 검색
        
        Args:
            query: 검색 쿼리
            max_results: 최대 결과 수
            sort_by: 정렬 기준 (relevance, lastUpdatedDate, submittedDate)
            sort_order: 정렬 순서 (ascending, descending)
            categories: 카테고리 필터 (예: ["cs.AI", "cs.LG"])
        
        Returns:
            검색 결과 리스트
        """
        # Build query with category filter
        if categories:
            category_query = " OR ".join([f"cat:{cat}" for cat in categories])
            query = f"({query}) AND ({category_query})"
        
        # Set sort criteria
        sort_criterion = {
            "relevance": arxiv.SortCriterion.Relevance,
            "lastUpdatedDate": arxiv.SortCriterion.LastUpdatedDate,
            "submittedDate": arxiv.SortCriterion.SubmittedDate
        }.get(sort_by, arxiv.SortCriterion.Relevance)
        
        sort_ord = {
            "ascending": arxiv.SortOrder.Ascending,
            "descending": arxiv.SortOrder.Descending
        }.get(sort_order, arxiv.SortOrder.Descending)
        
        # Create search object
        search = arxiv.Search(
            query=query,
            max_results=max_results,
            sort_by=sort_criterion,
            sort_order=sort_ord
        )
        
        # Execute search (run in thread pool since arxiv library is synchronous)
        results = await asyncio.get_event_loop().run_in_executor(
            None,
            lambda: list(self.client.results(search))
        )
        
        # Convert to PaperSearchResult
        papers = []
        for result in results:
            paper = self._convert_result(result)
            papers.append(paper)
        
        return papers
    
    async def get_paper(self, paper_id: str) -> Optional[PaperSearchResult]:
        """
        arXiv 논문 ID로 조회
        
        Args:
            paper_id: arXiv ID (예: "2301.07041")
        
        Returns:
            논문 정보 또는 None
        """
        # Clean paper ID
        paper_id = paper_id.replace("arxiv:", "").strip()
        
        search = arxiv.Search(id_list=[paper_id])
        
        try:
            results = await asyncio.get_event_loop().run_in_executor(
                None,
                lambda: list(self.client.results(search))
            )
            
            if results:
                return self._convert_result(results[0])
        except Exception:
            pass
        
        return None
    
    def _convert_result(self, result: arxiv.Result) -> PaperSearchResult:
        """arXiv Result를 PaperSearchResult로 변환"""
        return PaperSearchResult(
            paper_id=result.entry_id.split("/")[-1],  # Extract arxiv ID
            title=result.title,
            abstract=result.summary,
            authors=[author.name for author in result.authors],
            published_date=result.published,
            journal=result.journal_ref,
            doi=result.doi,
            url=result.entry_id,
            pdf_url=result.pdf_url,
            categories=[result.primary_category] + result.categories,
            keywords=[],  # arXiv doesn't provide keywords directly
            citation_count=0,  # Not available from arXiv API
            references=[],  # Not available from arXiv API
            source="arxiv",
            raw_data={
                "comment": result.comment,
                "updated": result.updated.isoformat() if result.updated else None,
            }
        )
    
    async def search_by_author(
        self,
        author_name: str,
        max_results: int = 50
    ) -> List[PaperSearchResult]:
        """저자로 논문 검색"""
        query = f'au:"{author_name}"'
        return await self.search(query, max_results)
    
    async def search_recent(
        self,
        categories: List[str],
        days: int = 7,
        max_results: int = 100
    ) -> List[PaperSearchResult]:
        """최근 N일 내 논문 검색"""
        from datetime import timedelta
        
        # arXiv search by date
        query = " OR ".join([f"cat:{cat}" for cat in categories])
        
        return await self.search(
            query=query,
            max_results=max_results,
            sort_by="submittedDate",
            sort_order="descending"
        )
