"""
Virtual Science Lab - Semantic Scholar API Tool
학술 논문 검색을 위한 도구
"""

import urllib.request
import urllib.parse
import json
from typing import List, Dict, Optional, Any


class SemanticScholarTool:
    """Semantic Scholar Academic Graph API 클라이언트"""
    
    BASE_URL = "https://api.semanticscholar.org/graph/v1"
    
    def __init__(self, api_key: Optional[str] = None):
        self.api_key = api_key
        self.headers = {"Content-Type": "application/json"}
        if api_key:
            self.headers["x-api-key"] = api_key
    
    def search_papers(
        self,
        query: str,
        limit: int = 10,
        fields: Optional[List[str]] = None,
        year_range: Optional[tuple] = None
    ) -> List[Dict]:
        """
        키워드로 논문 검색
        
        Args:
            query: 검색 키워드
            limit: 최대 결과 수 (기본 10, 최대 100)
            fields: 반환할 필드 (기본: title, abstract, year, citationCount, authors)
            year_range: (시작년도, 끝년도) 튜플
        
        Returns:
            논문 목록
        """
        if fields is None:
            fields = ["paperId", "title", "abstract", "year", "citationCount", "authors", "url"]
        
        params = {
            "query": query,
            "limit": min(limit, 100),
            "fields": ",".join(fields)
        }
        
        if year_range:
            params["year"] = f"{year_range[0]}-{year_range[1]}"
        
        url = f"{self.BASE_URL}/paper/search?{urllib.parse.urlencode(params)}"
        
        try:
            req = urllib.request.Request(url, headers=self.headers)
            with urllib.request.urlopen(req, timeout=30) as response:
                data = json.loads(response.read().decode())
                return data.get("data", [])
        except Exception as e:
            print(f"[SemanticScholar] Search failed: {e}")
            return []
    
    def get_paper_details(self, paper_id: str) -> Optional[Dict]:
        """
        논문 ID로 상세 정보 조회
        """
        fields = ["paperId", "title", "abstract", "year", "citationCount", 
                  "authors", "references", "citations", "url", "venue"]
        
        url = f"{self.BASE_URL}/paper/{paper_id}?fields={','.join(fields)}"
        
        try:
            req = urllib.request.Request(url, headers=self.headers)
            with urllib.request.urlopen(req, timeout=30) as response:
                return json.loads(response.read().decode())
        except Exception as e:
            print(f"[SemanticScholar] Get paper failed: {e}")
            return None
    
    def get_citations(self, paper_id: str, limit: int = 10) -> List[Dict]:
        """
        특정 논문을 인용한 논문들 조회
        """
        fields = ["paperId", "title", "year", "citationCount"]
        url = f"{self.BASE_URL}/paper/{paper_id}/citations?fields={','.join(fields)}&limit={limit}"
        
        try:
            req = urllib.request.Request(url, headers=self.headers)
            with urllib.request.urlopen(req, timeout=30) as response:
                data = json.loads(response.read().decode())
                return [item["citingPaper"] for item in data.get("data", [])]
        except Exception as e:
            print(f"[SemanticScholar] Get citations failed: {e}")
            return []


def search_literature(
    query: str,
    domain: str = "general",
    max_results: int = 10,
    recent_years: int = 5
) -> List[Dict[str, Any]]:
    """
    도메인 인식 문헌 검색 함수 (간편 인터페이스)
    
    Args:
        query: 검색 쿼리
        domain: 연구 분야
        max_results: 최대 결과 수
        recent_years: 최근 N년 논문만
    
    Returns:
        정규화된 논문 목록
    """
    from datetime import datetime
    current_year = datetime.now().year
    
    tool = SemanticScholarTool()
    papers = tool.search_papers(
        query=query,
        limit=max_results,
        year_range=(current_year - recent_years, current_year)
    )
    
    # 정규화된 형식으로 변환
    results = []
    for paper in papers:
        authors = paper.get("authors", [])
        author_names = [a.get("name", "Unknown") for a in authors[:3]]
        
        results.append({
            "paper_id": paper.get("paperId", ""),
            "title": paper.get("title", "Untitled"),
            "abstract": paper.get("abstract", "")[:500] if paper.get("abstract") else "",
            "authors": author_names,
            "year": paper.get("year", 0),
            "citation_count": paper.get("citationCount", 0),
            "source": "semantic_scholar",
            "url": paper.get("url", "")
        })
    
    # 인용수 기준 정렬
    results.sort(key=lambda x: x["citation_count"], reverse=True)
    
    return results
