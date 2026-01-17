"""
ArXiv Search Tool - ArXiv API를 이용한 학술 논문 검색
"""
from typing import List, Dict, Any
import urllib.request
import urllib.parse
import xml.etree.ElementTree as ET


ARXIV_API_URL = "http://export.arxiv.org/api/query"


def search_arxiv(query: str, max_results: int = 10) -> List[Dict[str, Any]]:
    """
    ArXiv에서 논문 검색
    
    Args:
        query: 검색 쿼리
        max_results: 최대 결과 수
    
    Returns:
        논문 정보 목록
    """
    try:
        # URL 인코딩
        params = {
            'search_query': f'all:{query}',
            'start': 0,
            'max_results': max_results,
            'sortBy': 'relevance',
            'sortOrder': 'descending'
        }
        url = f"{ARXIV_API_URL}?{urllib.parse.urlencode(params)}"
        
        # API 호출
        with urllib.request.urlopen(url, timeout=10) as response:
            xml_data = response.read()
        
        # XML 파싱
        root = ET.fromstring(xml_data)
        ns = {'atom': 'http://www.w3.org/2005/Atom'}
        
        results = []
        for entry in root.findall('atom:entry', ns):
            # 제목
            title_elem = entry.find('atom:title', ns)
            title = title_elem.text.strip().replace('\n', ' ') if title_elem is not None else ''
            
            # 저자
            authors = []
            for author in entry.findall('atom:author', ns):
                name = author.find('atom:name', ns)
                if name is not None:
                    authors.append(name.text)
            
            # 초록
            summary_elem = entry.find('atom:summary', ns)
            abstract = summary_elem.text.strip() if summary_elem is not None else ''
            
            # 발행일
            published_elem = entry.find('atom:published', ns)
            published = published_elem.text[:4] if published_elem is not None else ''
            
            # URL
            id_elem = entry.find('atom:id', ns)
            url = id_elem.text if id_elem is not None else ''
            
            # 카테고리
            categories = []
            for cat in entry.findall('atom:category', ns):
                term = cat.get('term', '')
                if term:
                    categories.append(term)
            
            results.append({
                'title': title,
                'authors': authors,
                'abstract': abstract[:500],  # 길이 제한
                'year': int(published) if published.isdigit() else 0,
                'source': 'arxiv',
                'url': url,
                'categories': categories,
                'relevance_score': 0.8  # ArXiv는 관련도 반환 안 함
            })
        
        return results
        
    except Exception as e:
        # API 실패 시 빈 결과 반환
        print(f"ArXiv API 오류: {e}")
        return []


def get_arxiv_paper(arxiv_id: str) -> Dict[str, Any]:
    """
    특정 ArXiv 논문 상세 정보 조회
    
    Args:
        arxiv_id: ArXiv ID (예: '2301.04567')
    
    Returns:
        논문 상세 정보
    """
    try:
        url = f"{ARXIV_API_URL}?id_list={arxiv_id}"
        
        with urllib.request.urlopen(url, timeout=10) as response:
            xml_data = response.read()
        
        root = ET.fromstring(xml_data)
        ns = {'atom': 'http://www.w3.org/2005/Atom'}
        
        entry = root.find('atom:entry', ns)
        if entry is None:
            return {}
        
        title_elem = entry.find('atom:title', ns)
        summary_elem = entry.find('atom:summary', ns)
        
        return {
            'title': title_elem.text.strip() if title_elem is not None else '',
            'abstract': summary_elem.text.strip() if summary_elem is not None else '',
            'arxiv_id': arxiv_id
        }
        
    except Exception as e:
        print(f"ArXiv 조회 오류: {e}")
        return {}
