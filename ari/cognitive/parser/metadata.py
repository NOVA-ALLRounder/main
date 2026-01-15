"""
Metadata Extractor

논문 메타데이터 추출 및 관리
"""

from dataclasses import dataclass, field
from typing import List, Optional, Dict, Any
from datetime import datetime
import re


@dataclass
class PaperMetadata:
    """논문 메타데이터"""
    
    # 식별자
    paper_id: str = ""
    doi: Optional[str] = None
    arxiv_id: Optional[str] = None
    
    # 기본 정보
    title: str = ""
    abstract: str = ""
    authors: List[str] = field(default_factory=list)
    
    # 출판 정보
    journal: Optional[str] = None
    conference: Optional[str] = None
    year: Optional[int] = None
    published_date: Optional[datetime] = None
    
    # 분류
    keywords: List[str] = field(default_factory=list)
    categories: List[str] = field(default_factory=list)
    
    # 인용 정보
    citation_count: int = 0
    reference_count: int = 0
    
    # 접근 정보
    url: Optional[str] = None
    pdf_url: Optional[str] = None
    
    # 소스
    source: str = ""  # arxiv, semantic_scholar, pubmed, etc.
    
    def to_dict(self) -> Dict[str, Any]:
        """딕셔너리로 변환"""
        return {
            "paper_id": self.paper_id,
            "doi": self.doi,
            "arxiv_id": self.arxiv_id,
            "title": self.title,
            "abstract": self.abstract,
            "authors": self.authors,
            "journal": self.journal,
            "conference": self.conference,
            "year": self.year,
            "published_date": self.published_date.isoformat() if self.published_date else None,
            "keywords": self.keywords,
            "categories": self.categories,
            "citation_count": self.citation_count,
            "reference_count": self.reference_count,
            "url": self.url,
            "pdf_url": self.pdf_url,
            "source": self.source,
        }
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "PaperMetadata":
        """딕셔너리에서 생성"""
        pub_date = None
        if data.get("published_date"):
            try:
                pub_date = datetime.fromisoformat(data["published_date"])
            except ValueError:
                pass
        
        return cls(
            paper_id=data.get("paper_id", ""),
            doi=data.get("doi"),
            arxiv_id=data.get("arxiv_id"),
            title=data.get("title", ""),
            abstract=data.get("abstract", ""),
            authors=data.get("authors", []),
            journal=data.get("journal"),
            conference=data.get("conference"),
            year=data.get("year"),
            published_date=pub_date,
            keywords=data.get("keywords", []),
            categories=data.get("categories", []),
            citation_count=data.get("citation_count", 0),
            reference_count=data.get("reference_count", 0),
            url=data.get("url"),
            pdf_url=data.get("pdf_url"),
            source=data.get("source", ""),
        )
    
    def get_citation_key(self) -> str:
        """BibTeX 인용 키 생성"""
        if not self.authors or not self.year:
            return self.paper_id or "unknown"
        
        # First author's last name + year
        first_author = self.authors[0]
        last_name = first_author.split()[-1].lower()
        last_name = re.sub(r'[^a-z]', '', last_name)
        
        return f"{last_name}{self.year}"
    
    def to_bibtex(self) -> str:
        """BibTeX 형식으로 변환"""
        key = self.get_citation_key()
        entry_type = "article" if self.journal else "inproceedings"
        
        lines = [f"@{entry_type}{{{key},"]
        lines.append(f'  title = {{{self.title}}},')
        
        if self.authors:
            authors_str = " and ".join(self.authors)
            lines.append(f'  author = {{{authors_str}}},')
        
        if self.year:
            lines.append(f'  year = {{{self.year}}},')
        
        if self.journal:
            lines.append(f'  journal = {{{self.journal}}},')
        elif self.conference:
            lines.append(f'  booktitle = {{{self.conference}}},')
        
        if self.doi:
            lines.append(f'  doi = {{{self.doi}}},')
        
        if self.url:
            lines.append(f'  url = {{{self.url}}},')
        
        if self.abstract:
            # Escape special characters in abstract
            abstract = self.abstract.replace('{', '\\{').replace('}', '\\}')
            lines.append(f'  abstract = {{{abstract}}},')
        
        lines.append('}')
        return '\n'.join(lines)


def extract_metadata(text: str, source: str = "") -> PaperMetadata:
    """
    텍스트에서 메타데이터 추출
    
    Args:
        text: 논문 텍스트
        source: 출처
    
    Returns:
        PaperMetadata 객체
    """
    metadata = PaperMetadata(source=source)
    
    # Extract title (first line or lines before abstract)
    lines = text.strip().split('\n')
    title_lines = []
    for line in lines[:10]:
        line = line.strip()
        if not line:
            if title_lines:
                break
            continue
        if re.match(r'(?:abstract|keywords|introduction)', line, re.I):
            break
        title_lines.append(line)
        if len(title_lines) >= 3:
            break
    metadata.title = ' '.join(title_lines)
    
    # Extract abstract
    abstract_match = re.search(
        r'(?:abstract|summary)[:\s]*\n?(.*?)(?=\n\s*(?:keywords|introduction|\d+\.|1\s))',
        text,
        re.I | re.S
    )
    if abstract_match:
        metadata.abstract = re.sub(r'\s+', ' ', abstract_match.group(1).strip())
    
    # Extract keywords
    keywords_match = re.search(
        r'keywords?[:\s]*([^\n]+)',
        text,
        re.I
    )
    if keywords_match:
        keywords_text = keywords_match.group(1)
        metadata.keywords = [k.strip() for k in re.split(r'[,;]', keywords_text) if k.strip()]
    
    # Extract year
    year_match = re.search(r'\b(19|20)\d{2}\b', text[:2000])
    if year_match:
        metadata.year = int(year_match.group())
    
    # Extract DOI
    doi_match = re.search(r'doi[:\s]*([^\s]+)', text, re.I)
    if doi_match:
        metadata.doi = doi_match.group(1).strip()
    
    # Extract arXiv ID
    arxiv_match = re.search(r'arxiv[:\s]*(\d+\.\d+)', text, re.I)
    if arxiv_match:
        metadata.arxiv_id = arxiv_match.group(1)
    
    return metadata
