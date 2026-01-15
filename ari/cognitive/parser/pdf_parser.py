"""
PDF Parser

PyMuPDF를 사용한 PDF 논문 파싱
"""

import re
from pathlib import Path
from typing import List, Dict, Any, Optional, Tuple
from dataclasses import dataclass, field
import fitz  # PyMuPDF


@dataclass
class ParsedSection:
    """파싱된 섹션"""
    title: str
    content: str
    level: int = 1  # Heading level (1, 2, 3...)
    page_start: int = 0
    page_end: int = 0


@dataclass
class ParsedPaper:
    """파싱된 논문"""
    title: str = ""
    abstract: str = ""
    sections: List[ParsedSection] = field(default_factory=list)
    references: List[str] = field(default_factory=list)
    full_text: str = ""
    page_count: int = 0
    metadata: Dict[str, Any] = field(default_factory=dict)
    
    def get_section(self, name: str) -> Optional[ParsedSection]:
        """섹션 이름으로 조회"""
        name_lower = name.lower()
        for section in self.sections:
            if name_lower in section.title.lower():
                return section
        return None
    
    def get_introduction(self) -> str:
        """서론 섹션 반환"""
        section = self.get_section("introduction")
        return section.content if section else ""
    
    def get_methodology(self) -> str:
        """방법론 섹션 반환"""
        for name in ["method", "approach", "methodology"]:
            section = self.get_section(name)
            if section:
                return section.content
        return ""
    
    def get_results(self) -> str:
        """결과 섹션 반환"""
        for name in ["result", "experiment", "evaluation"]:
            section = self.get_section(name)
            if section:
                return section.content
        return ""
    
    def get_conclusion(self) -> str:
        """결론 섹션 반환"""
        for name in ["conclusion", "discussion", "future"]:
            section = self.get_section(name)
            if section:
                return section.content
        return ""


class PDFParser:
    """PDF 논문 파서"""
    
    # Common section titles pattern
    SECTION_PATTERNS = [
        r'^(?:\d+\.?\s*)?(?:ABSTRACT|Abstract)',
        r'^(?:\d+\.?\s*)?(?:INTRODUCTION|Introduction)',
        r'^(?:\d+\.?\s*)?(?:RELATED\s*WORK|Related\s*Work|Background)',
        r'^(?:\d+\.?\s*)?(?:METHOD(?:OLOGY)?|Method(?:ology)?|Approach)',
        r'^(?:\d+\.?\s*)?(?:EXPERIMENT(?:S)?|Experiment(?:s)?|Evaluation)',
        r'^(?:\d+\.?\s*)?(?:RESULT(?:S)?|Result(?:s)?)',
        r'^(?:\d+\.?\s*)?(?:DISCUSSION|Discussion)',
        r'^(?:\d+\.?\s*)?(?:CONCLUSION(?:S)?|Conclusion(?:s)?)',
        r'^(?:\d+\.?\s*)?(?:REFERENCE(?:S)?|Reference(?:s)?|Bibliography)',
        r'^(?:\d+\.?\s*)?(?:APPENDIX|Appendix)',
    ]
    
    def __init__(self):
        self.section_regex = re.compile(
            '|'.join(self.SECTION_PATTERNS),
            re.MULTILINE
        )
    
    def parse(self, pdf_path: str) -> ParsedPaper:
        """
        PDF 파일 파싱
        
        Args:
            pdf_path: PDF 파일 경로
        
        Returns:
            ParsedPaper 객체
        """
        path = Path(pdf_path)
        if not path.exists():
            raise FileNotFoundError(f"PDF file not found: {pdf_path}")
        
        doc = fitz.open(pdf_path)
        
        try:
            # Extract text from all pages
            pages_text = []
            for page in doc:
                pages_text.append(page.get_text())
            
            full_text = "\n".join(pages_text)
            
            # Extract metadata
            metadata = doc.metadata or {}
            
            # Parse title (usually on first page)
            title = self._extract_title(pages_text[0] if pages_text else "")
            
            # Parse abstract
            abstract = self._extract_abstract(full_text)
            
            # Parse sections
            sections = self._extract_sections(full_text)
            
            # Parse references
            references = self._extract_references(full_text)
            
            return ParsedPaper(
                title=title or metadata.get("title", ""),
                abstract=abstract,
                sections=sections,
                references=references,
                full_text=full_text,
                page_count=len(doc),
                metadata=metadata
            )
        finally:
            doc.close()
    
    def _extract_title(self, first_page: str) -> str:
        """첫 페이지에서 제목 추출"""
        lines = first_page.strip().split('\n')
        
        # Title is usually the first non-empty line(s) with larger font
        # Simple heuristic: take first 1-3 lines that seem like a title
        title_lines = []
        for line in lines[:10]:
            line = line.strip()
            if not line:
                if title_lines:
                    break
                continue
            if len(line) > 200:  # Too long for a title
                break
            if re.match(r'^(?:Abstract|ABSTRACT)', line):
                break
            title_lines.append(line)
            if len(title_lines) >= 3:
                break
        
        return " ".join(title_lines)
    
    def _extract_abstract(self, text: str) -> str:
        """초록 추출"""
        # Look for Abstract section
        abstract_match = re.search(
            r'(?:ABSTRACT|Abstract)\s*\n(.*?)(?=\n\s*(?:\d+\.?\s*)?(?:INTRODUCTION|Introduction|1\s+Introduction))',
            text,
            re.DOTALL | re.IGNORECASE
        )
        
        if abstract_match:
            abstract = abstract_match.group(1).strip()
            # Clean up
            abstract = re.sub(r'\s+', ' ', abstract)
            return abstract
        
        return ""
    
    def _extract_sections(self, text: str) -> List[ParsedSection]:
        """섹션 추출"""
        sections = []
        
        # Find all section headers
        matches = list(re.finditer(
            r'^(?:(\d+)\.?\s+)?([A-Z][A-Za-z\s]+?)(?:\n|$)',
            text,
            re.MULTILINE
        ))
        
        for i, match in enumerate(matches):
            section_num = match.group(1) or ""
            section_title = match.group(2).strip()
            
            # Skip if too short or doesn't look like a section
            if len(section_title) < 3:
                continue
            
            # Get content until next section or end
            start = match.end()
            if i + 1 < len(matches):
                end = matches[i + 1].start()
            else:
                end = len(text)
            
            content = text[start:end].strip()
            
            # Determine level
            level = 1 if section_num and '.' not in section_num else 2
            
            sections.append(ParsedSection(
                title=section_title,
                content=content,
                level=level
            ))
        
        return sections
    
    def _extract_references(self, text: str) -> List[str]:
        """참고문헌 추출"""
        # Find references section
        ref_match = re.search(
            r'(?:REFERENCES?|References?|Bibliography)\s*\n(.*)$',
            text,
            re.DOTALL | re.IGNORECASE
        )
        
        if not ref_match:
            return []
        
        ref_text = ref_match.group(1)
        
        # Split by numbered references [1], [2], etc.
        refs = re.split(r'\n\s*\[\d+\]\s*', ref_text)
        
        # Or by numbered list 1., 2., etc.
        if len(refs) <= 1:
            refs = re.split(r'\n\s*\d+\.\s+', ref_text)
        
        # Clean up
        references = []
        for ref in refs:
            ref = ref.strip()
            ref = re.sub(r'\s+', ' ', ref)
            if len(ref) > 20:  # Minimum reference length
                references.append(ref)
        
        return references
    
    def extract_text_chunks(
        self,
        pdf_path: str,
        chunk_size: int = 1000,
        overlap: int = 100
    ) -> List[Dict[str, Any]]:
        """
        PDF를 청크 단위로 추출 (RAG용)
        
        Args:
            pdf_path: PDF 파일 경로
            chunk_size: 청크 크기 (문자 수)
            overlap: 청크 간 오버랩
        
        Returns:
            청크 리스트 [{"text": ..., "page": ..., "chunk_id": ...}]
        """
        doc = fitz.open(pdf_path)
        chunks = []
        chunk_id = 0
        
        try:
            for page_num, page in enumerate(doc):
                page_text = page.get_text()
                
                # Split page into chunks
                start = 0
                while start < len(page_text):
                    end = min(start + chunk_size, len(page_text))
                    chunk_text = page_text[start:end]
                    
                    if chunk_text.strip():
                        chunks.append({
                            "text": chunk_text,
                            "page": page_num + 1,
                            "chunk_id": chunk_id
                        })
                        chunk_id += 1
                    
                    if end >= len(page_text):
                        break
                    start = end - overlap
        finally:
            doc.close()
        
        return chunks
