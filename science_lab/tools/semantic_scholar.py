"""
Semantic Scholar Search Tool - Semantic Scholar API를 이용한 학술 논문 검색
"""
from typing import List, Dict, Any
import urllib.request
import urllib.parse
import urllib.error
import json
import os


SEMANTIC_SCHOLAR_API_URL = "https://api.semanticscholar.org/graph/v1"


def search_semantic_scholar(
    query: str, 
    limit: int = 10,
    fields: List[str] = None
) -> List[Dict[str, Any]]:
    """
    Semantic Scholar에서 논문 검색
    
    Args:
        query: 검색 쿼리
        limit: 최대 결과 수
        fields: 반환할 필드 목록
    
    Returns:
        논문 정보 목록
    """
    if fields is None:
        fields = ['title', 'abstract', 'authors', 'year', 'url', 'citationCount']
    
    # 재시도 로직 (Free Tier 고려하여 보수적으로 설정)
    max_retries = 1
    base_delay = 2
    
    for attempt in range(max_retries + 1):
        try:
            # URL 구성
            params = {
                'query': query,
                'limit': limit,
                'fields': ','.join(fields)
            }
            url = f"{SEMANTIC_SCHOLAR_API_URL}/paper/search?{urllib.parse.urlencode(params)}"
            
            # 헤더 설정
            headers = {
                'Accept': 'application/json',
                'User-Agent': 'ScienceLab/1.0'
            }
            
            # API 키가 있으면 추가
            api_key = os.getenv('SEMANTIC_SCHOLAR_API_KEY', '')
            if api_key:
                headers['x-api-key'] = api_key
            
            # 요청
            request = urllib.request.Request(url, headers=headers)
            
            with urllib.request.urlopen(request, timeout=10) as response:
                data = json.loads(response.read())
            
            results = []
            for paper in data.get('data', []):
                # 저자 추출
                authors = []
                for author in paper.get('authors', []):
                    name = author.get('name', '')
                    if name:
                        authors.append(name)
                
                results.append({
                    'title': paper.get('title', ''),
                    'authors': authors,
                    'abstract': paper.get('abstract', '')[:500] if paper.get('abstract') else '',
                    'year': paper.get('year', 0) or 0,
                    'source': 'semantic_scholar',
                    'url': paper.get('url', ''),
                    'citation_count': paper.get('citationCount', 0),
                    'relevance_score': 0.9  # Semantic Scholar는 관련도 순으로 반환
                })
            
            return results
            
        except urllib.error.HTTPError as e:
            if e.code == 429:
                print(f"Semantic Scholar API Rate Limit (429). Retrying in {base_delay * (attempt + 1)}s...")
                import time
                time.sleep(base_delay * (attempt + 1))
                continue
            else:
                print(f"Semantic Scholar API 오류: {e}")
                return []
        except Exception as e:
            print(f"Semantic Scholar 요청 오류: {e}")
            return []
    
    # 최대 재시도 횟수 초과 시, 오류 로그 대신 빈 리스트 반환 (사용자 혼란 방지)
    # print("Semantic Scholar API: 최대 재시도 횟수 초과")
    return []


def get_paper_details(paper_id: str) -> Dict[str, Any]:
    """
    특정 논문의 상세 정보 조회
    
    Args:
        paper_id: Semantic Scholar Paper ID
    
    Returns:
        논문 상세 정보
    """
    try:
        fields = ['title', 'abstract', 'authors', 'year', 'venue', 
                  'citationCount', 'referenceCount', 'fieldsOfStudy']
        
        url = f"{SEMANTIC_SCHOLAR_API_URL}/paper/{paper_id}?fields={','.join(fields)}"
        
        headers = {
            'Accept': 'application/json',
            'User-Agent': 'ScienceLab/1.0'
        }
        
        api_key = os.getenv('SEMANTIC_SCHOLAR_API_KEY', '')
        if api_key:
            headers['x-api-key'] = api_key
        
        request = urllib.request.Request(url, headers=headers)
        
        with urllib.request.urlopen(request, timeout=10) as response:
            return json.loads(response.read())
        
    except Exception as e:
        print(f"Semantic Scholar 조회 오류: {e}")
        return {}


def get_paper_citations(paper_id: str, limit: int = 20) -> List[Dict[str, Any]]:
    """
    특정 논문을 인용한 논문 목록 조회
    
    Args:
        paper_id: Semantic Scholar Paper ID
        limit: 최대 결과 수
    
    Returns:
        인용 논문 목록
    """
    try:
        url = f"{SEMANTIC_SCHOLAR_API_URL}/paper/{paper_id}/citations?limit={limit}&fields=title,authors,year"
        
        headers = {
            'Accept': 'application/json',
            'User-Agent': 'ScienceLab/1.0'
        }
        
        request = urllib.request.Request(url, headers=headers)
        
        with urllib.request.urlopen(request, timeout=10) as response:
            data = json.loads(response.read())
        
        return data.get('data', [])
        
    except Exception as e:
        print(f"인용 조회 오류: {e}")
        return []
