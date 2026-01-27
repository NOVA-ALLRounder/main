"""
Bibliography Manager

BibTeX 참고문헌 관리
"""

from typing import List, Dict, Any, Optional
from dataclasses import dataclass
import re


@dataclass
class BibEntry:
    """BibTeX 엔트리"""
    key: str
    entry_type: str  # article, inproceedings, book, etc.
    title: str
    authors: List[str]
    year: int
    
    # Optional fields
    journal: Optional[str] = None
    booktitle: Optional[str] = None
    volume: Optional[str] = None
    pages: Optional[str] = None
    doi: Optional[str] = None
    url: Optional[str] = None
    abstract: Optional[str] = None
    
    def to_bibtex(self) -> str:
        """BibTeX 형식으로 변환"""
        lines = [f"@{self.entry_type}{{{self.key},"]
        lines.append(f'  title = {{{self.title}}},')
        lines.append(f'  author = {{{" and ".join(self.authors)}}},')
        lines.append(f'  year = {{{self.year}}},')
        
        if self.journal:
            lines.append(f'  journal = {{{self.journal}}},')
        if self.booktitle:
            lines.append(f'  booktitle = {{{self.booktitle}}},')
        if self.volume:
            lines.append(f'  volume = {{{self.volume}}},')
        if self.pages:
            lines.append(f'  pages = {{{self.pages}}},')
        if self.doi:
            lines.append(f'  doi = {{{self.doi}}},')
        if self.url:
            lines.append(f'  url = {{{self.url}}},')
        
        lines.append('}')
        return '\n'.join(lines)


class BibliographyManager:
    """참고문헌 관리자"""
    
    def __init__(self):
        self.entries: Dict[str, BibEntry] = {}
    
    def add_entry(self, entry: BibEntry):
        """엔트리 추가"""
        self.entries[entry.key] = entry
    
    def add_from_dict(self, data: Dict[str, Any]) -> BibEntry:
        """딕셔너리에서 엔트리 생성 및 추가"""
        entry = BibEntry(
            key=data.get("key", self._generate_key(data)),
            entry_type=data.get("entry_type", "article"),
            title=data.get("title", ""),
            authors=data.get("authors", []),
            year=data.get("year", 2024),
            journal=data.get("journal"),
            booktitle=data.get("booktitle"),
            volume=data.get("volume"),
            pages=data.get("pages"),
            doi=data.get("doi"),
            url=data.get("url"),
            abstract=data.get("abstract")
        )
        self.add_entry(entry)
        return entry
    
    def _generate_key(self, data: Dict[str, Any]) -> str:
        """BibTeX 키 생성"""
        authors = data.get("authors", ["unknown"])
        year = data.get("year", 2024)
        
        if authors:
            first_author = authors[0]
            last_name = first_author.split()[-1].lower()
            last_name = re.sub(r'[^a-z]', '', last_name)
        else:
            last_name = "unknown"
        
        return f"{last_name}{year}"
    
    def get_entry(self, key: str) -> Optional[BibEntry]:
        """키로 엔트리 조회"""
        return self.entries.get(key)
    
    def remove_entry(self, key: str):
        """엔트리 제거"""
        if key in self.entries:
            del self.entries[key]
    
    def to_bibtex(self) -> str:
        """전체 BibTeX 파일 생성"""
        return '\n\n'.join(entry.to_bibtex() for entry in self.entries.values())
    
    def save(self, filepath: str):
        """BibTeX 파일 저장"""
        with open(filepath, 'w', encoding='utf-8') as f:
            f.write(self.to_bibtex())
    
    def load(self, filepath: str):
        """BibTeX 파일 로드 (간단한 파싱)"""
        with open(filepath, 'r', encoding='utf-8') as f:
            content = f.read()
        
        # 간단한 BibTeX 파싱
        pattern = r'@(\w+)\{([^,]+),\s*(.*?)\n\}'
        matches = re.findall(pattern, content, re.DOTALL)
        
        for entry_type, key, fields_str in matches:
            fields = {}
            for match in re.finditer(r'(\w+)\s*=\s*\{([^}]*)\}', fields_str):
                field_name, field_value = match.groups()
                fields[field_name.lower()] = field_value.strip()
            
            # 저자 파싱
            authors = []
            if 'author' in fields:
                authors = [a.strip() for a in fields['author'].split(' and ')]
            
            try:
                entry = BibEntry(
                    key=key.strip(),
                    entry_type=entry_type.lower(),
                    title=fields.get('title', ''),
                    authors=authors,
                    year=int(fields.get('year', 2024)),
                    journal=fields.get('journal'),
                    booktitle=fields.get('booktitle'),
                    volume=fields.get('volume'),
                    pages=fields.get('pages'),
                    doi=fields.get('doi'),
                    url=fields.get('url')
                )
                self.add_entry(entry)
            except (ValueError, KeyError):
                pass
    
    def filter_by_year(self, min_year: int, max_year: int = None) -> List[BibEntry]:
        """연도로 필터링"""
        max_year = max_year or 9999
        return [e for e in self.entries.values() if min_year <= e.year <= max_year]
    
    def search(self, query: str) -> List[BibEntry]:
        """제목/저자 검색"""
        query = query.lower()
        results = []
        
        for entry in self.entries.values():
            if query in entry.title.lower():
                results.append(entry)
            elif any(query in author.lower() for author in entry.authors):
                results.append(entry)
        
        return results
    
    def get_all_keys(self) -> List[str]:
        """모든 키 반환"""
        return list(self.entries.keys())
    
    def count(self) -> int:
        """엔트리 수"""
        return len(self.entries)
    
    def generate_synthetic_references(
        self,
        domain: str,
        keywords: List[str] = None,
        num_refs: int = 8
    ) -> List[BibEntry]:
        """
        도메인에 맞는 합성 참고문헌 생성
        
        Args:
            domain: 연구 도메인
            keywords: 관련 키워드들
            num_refs: 생성할 참고문헌 수
        
        Returns:
            생성된 BibEntry 리스트
        """
        import random
        
        # 도메인별 저널/학회 및 저자 풀
        VENUES = {
            "Machine Learning": [
                ("IEEE Transactions on Neural Networks and Learning Systems", "article"),
                ("Advances in Neural Information Processing Systems (NeurIPS)", "inproceedings"),
                ("International Conference on Machine Learning (ICML)", "inproceedings"),
                ("Journal of Machine Learning Research", "article"),
            ],
            "통신": [
                ("IEEE Transactions on Wireless Communications", "article"),
                ("IEEE Communications Letters", "article"),
                ("IEEE International Conference on Communications (ICC)", "inproceedings"),
                ("IEEE Transactions on Signal Processing", "article"),
            ],
            "Computer Vision": [
                ("IEEE Conference on Computer Vision and Pattern Recognition (CVPR)", "inproceedings"),
                ("International Conference on Computer Vision (ICCV)", "inproceedings"),
                ("IEEE Transactions on Image Processing", "article"),
            ],
            "NLP": [
                ("Association for Computational Linguistics (ACL)", "inproceedings"),
                ("Conference on Empirical Methods in NLP (EMNLP)", "inproceedings"),
                ("Transactions of the ACL", "article"),
            ],
            "의료": [
                ("Nature Medicine", "article"),
                ("IEEE Transactions on Medical Imaging", "article"),
                ("Medical Image Computing and Computer Assisted Intervention (MICCAI)", "inproceedings"),
            ],
        }
        
        AUTHOR_POOL = [
            "Zhang, Wei", "Wang, Xiaoming", "Li, Jun", "Chen, Yang", "Liu, Feng",
            "Kim, Seung-Hwan", "Park, Jiyoung", "Lee, Dongwon",
            "Smith, John", "Johnson, Emily", "Brown, Michael", "Davis, Sarah",
            "Mueller, Hans", "Schmidt, Thomas", "Weber, Anna",
            "Tanaka, Yuki", "Suzuki, Hiroshi", "Yamamoto, Kenji",
        ]
        
        TITLE_TEMPLATES = {
            "Machine Learning": [
                "Deep Learning Approaches for {keyword} Recognition",
                "Efficient Neural Network Architecture for {keyword} Tasks",
                "Self-Supervised Learning for {keyword} Understanding",
                "Transformer-based {keyword} Models: A Comprehensive Study",
            ],
            "통신": [
                "Performance Analysis of {keyword} in Wireless Systems",
                "Optimization of {keyword} for 5G/6G Networks",
                "Resource Allocation Strategies for {keyword} Communications",
                "Channel Estimation for {keyword} MIMO Systems",
            ],
            "Computer Vision": [
                "Visual {keyword} Detection Using Convolutional Networks",
                "Real-time {keyword} Segmentation with Deep Learning",
                "Multi-scale Feature Extraction for {keyword} Recognition",
            ],
            "NLP": [
                "Language Models for {keyword} Generation",
                "Semantic Analysis of {keyword} Text Corpora",
                "Cross-lingual {keyword} Understanding with Transformers",
            ],
            "의료": [
                "AI-Assisted {keyword} Diagnosis from Medical Images",
                "Deep Learning for {keyword} Detection in Clinical Settings",
                "Computer-Aided {keyword} Screening System",
            ],
        }
        
        venues = VENUES.get(domain, VENUES["Machine Learning"])
        templates = TITLE_TEMPLATES.get(domain, TITLE_TEMPLATES["Machine Learning"])
        keywords = keywords or ["signal processing", "optimization", "deep learning"]
        
        entries = []
        used_keys = set(self.entries.keys())
        
        for i in range(num_refs):
            # 랜덤 저자 선택 (2-4명)
            num_authors = random.randint(2, 4)
            authors = random.sample(AUTHOR_POOL, num_authors)
            
            # 랜덤 연도 (최근 5년)
            year = random.randint(2020, 2025)
            
            # 랜덤 학회/저널
            venue_name, entry_type = random.choice(venues)
            
            # 제목 생성
            template = random.choice(templates)
            keyword = random.choice(keywords)
            title = template.format(keyword=keyword.title())
            
            # 고유 키 생성
            base_key = f"{authors[0].split(',')[0].lower()}{year}"
            key = base_key
            suffix = 0
            while key in used_keys:
                suffix += 1
                key = f"{base_key}{chr(ord('a') + suffix)}"
            used_keys.add(key)
            
            entry = BibEntry(
                key=key,
                entry_type=entry_type,
                title=title,
                authors=authors,
                year=year,
                journal=venue_name if entry_type == "article" else None,
                booktitle=venue_name if entry_type == "inproceedings" else None,
                volume=str(random.randint(20, 50)) if entry_type == "article" else None,
                pages=f"{random.randint(1, 500)}-{random.randint(501, 1000)}"
            )
            
            self.add_entry(entry)
            entries.append(entry)
        
        return entries
    
    def search_real_references(
        self,
        hypothesis_title: str,
        domain: str = "",
        keywords: List[str] = None,
        num_refs: int = 8,
        fallback_to_synthetic: bool = True
    ) -> List[BibEntry]:
        """
        실제 논문 검색하여 참고문헌 추가 (Semantic Scholar API 사용)
        
        Args:
            hypothesis_title: 가설/논문 제목
            domain: 연구 도메인
            keywords: 관련 키워드
            num_refs: 필요한 참고문헌 수
            fallback_to_synthetic: API 실패시 합성 참고문헌으로 대체
            
        Returns:
            추가된 BibEntry 리스트
        """
        try:
            from .paper_search import RealPaperSearcher
            
            searcher = RealPaperSearcher()
            papers = searcher.search_for_hypothesis(
                hypothesis_title=hypothesis_title,
                keywords=keywords,
                domain=domain,
                num_papers=num_refs
            )
            
            if papers:
                # 실제 논문을 BibEntry로 변환
                entries = []
                for paper in papers:
                    entry_dict = paper.to_bibtex_dict()
                    entry = self.add_from_dict(entry_dict)
                    entries.append(entry)
                
                return entries
            
        except ImportError as e:
            import logging
            logging.warning(f"paper_search 모듈 로드 실패: {e}")
        except Exception as e:
            import logging
            logging.warning(f"실제 논문 검색 실패: {e}")
        
        # 폴백: 합성 참고문헌 생성
        if fallback_to_synthetic:
            import logging
            logging.info("실제 논문 검색 실패, 합성 참고문헌으로 대체")
            return self.generate_synthetic_references(
                domain=domain,
                keywords=keywords,
                num_refs=num_refs
            )
        
        return []

