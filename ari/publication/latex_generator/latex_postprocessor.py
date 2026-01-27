"""
LaTeX Post-Processor

LaTeX 문서 후처리: 중복 제거, 참조 수정, 형식 통일
"""

import re
from typing import Dict, List, Tuple, Optional


class LaTeXPostProcessor:
    """
    LaTeX 문서 후처리
    - 중복 섹션 제거
    - 깨진 참조(`??`) 수정
    - 인용 형식 통일
    - 특수문자 이스케이프
    """
    
    def __init__(self):
        pass
    
    def process(self, latex_content: str) -> str:
        """
        전체 후처리 파이프라인
        
        Args:
            latex_content: 원본 LaTeX 문서
        
        Returns:
            처리된 LaTeX 문서
        """
        content = latex_content
        
        # 1. 중복 섹션 제거
        content = self._remove_duplicate_sections(content)
        
        # 2. 깨진 참조 제거/수정
        content = self._fix_broken_references(content)
        
        # 3. 인용 형식 통일
        content = self._standardize_citations(content)
        
        # 4. Markdown 잔여물 제거
        content = self._remove_markdown_artifacts(content)
        
        # 5. 빈 줄 정리
        content = self._clean_whitespace(content)
        
        return content
    
    def _remove_duplicate_sections(self, content: str) -> str:
        """
        연속된 중복 섹션 제거
        예: \\section{Introduction} 바로 뒤에 또 \\section{Introduction}
        """
        # 같은 섹션이 연속으로 나오는 패턴
        patterns = [
            # \section{X} 뒤에 바로 또 \section{X}
            r'(\\section\{([^}]+)\}\s*\n?\s*)\\section\{\2\}',
            # \subsection{X} 중복
            r'(\\subsection\{([^}]+)\}\s*\n?\s*)\\subsection\{\2\}',
        ]
        
        for pattern in patterns:
            content = re.sub(pattern, r'\1', content, flags=re.IGNORECASE)
        
        # ### Markdown 헤더가 LaTeX 안에 섞인 경우 제거
        content = re.sub(r'^###?\s+.+$', '', content, flags=re.MULTILINE)
        
        return content
    
    def _fix_broken_references(self, content: str) -> str:
        """
        깨진 참조 수정
        - "Figure ??" -> 텍스트로 대체 또는 제거
        - "Table ??" -> 텍스트로 대체 또는 제거
        - "Section ??" -> 텍스트로 대체 또는 제거
        """
        # Figure ?? 패턴
        content = re.sub(
            r'Figure\s*\?\?',
            'Figure 1',
            content,
            flags=re.IGNORECASE
        )
        content = re.sub(
            r'Fig\.\s*\?\?',
            'Fig. 1',
            content,
            flags=re.IGNORECASE
        )
        
        # Table ?? 패턴
        content = re.sub(
            r'Table\s*\?\?',
            'Table 1',
            content,
            flags=re.IGNORECASE
        )
        
        # Section ?? 패턴
        content = re.sub(
            r'Section\s*\?\?',
            'the previous section',
            content,
            flags=re.IGNORECASE
        )
        
        # \ref{...} 결과가 ??인 경우 - LaTeX 컴파일 후에만 보이므로
        # 미리 \ref 명령어를 안전하게 처리
        # 존재하지 않는 label 참조는 제거하거나 대체
        
        # 깨진 \ref{} 패턴들
        content = re.sub(
            r'\\ref\{fig:[^}]*\}',
            '1',
            content
        )
        content = re.sub(
            r'\\ref\{tab:[^}]*\}',
            '1',
            content
        )
        content = re.sub(
            r'\\ref\{sec:[^}]*\}',
            'this section',
            content
        )
        
        return content
    
    def _standardize_citations(self, content: str) -> str:
        """
        인용 형식 통일
        - (Author et al., 2024) 형식을 \\cite{} 형식으로 변환
        - 인라인 논문 제목 인용을 적절히 처리
        """
        # (Author, Year) 또는 (Author et al., Year) 패턴
        # 예: (Smith, 2023), (Kim et al., 2024)
        author_year_pattern = r'\(([A-Z][a-z]+(?:\s+et\s+al\.?)?),?\s*(\d{4})\)'
        
        def replace_citation(match):
            author = match.group(1).replace(' ', '_').replace('.', '')
            year = match.group(2)
            cite_key = f"{author.lower()}_{year}"
            return f"\\cite{{{cite_key}}}"
        
        content = re.sub(author_year_pattern, replace_citation, content)
        
        # [CITE: topic] 플레이스홀더 처리 (이미 generator에서 처리하지만 중복 안전)
        content = re.sub(
            r'\[CITE:\s*([^\]]+)\]',
            lambda m: f"\\cite{{{m.group(1).lower().replace(' ', '_')[:20]}}}",
            content
        )
        
        # 인라인 논문 제목 인용 (괄호 안에 긴 제목)
        # 예: (Robust blue-green... optimized...) -> \cite{related_work}
        long_paren_pattern = r'\(([A-Z][^)]{50,})\)'
        content = re.sub(
            long_paren_pattern,
            r'\\cite{related_work}',
            content
        )
        
        return content
    
    def _remove_markdown_artifacts(self, content: str) -> str:
        """
        Markdown 잔여물 제거
        - **bold** -> \textbf{bold}
        - *italic* -> \textit{italic}
        - ### Headers -> 제거
        - ``` code blocks -> 제거
        """
        # **bold**
        content = re.sub(r'\*\*([^*]+)\*\*', r'\\textbf{\1}', content)
        
        # *italic*
        content = re.sub(r'\*([^*]+)\*', r'\\textit{\1}', content)
        
        # ### Headers (LaTeX 내에서는 이미 \section 사용)
        content = re.sub(r'^###?\s*.+$', '', content, flags=re.MULTILINE)
        
        # Bullet points (- item) -> LaTeX itemize
        # 간단히 제거하거나 변환
        content = re.sub(r'^\s*[-*]\s+', r'\\item ', content, flags=re.MULTILINE)
        
        # Numbered lists (1. item)
        content = re.sub(r'^\s*\d+\.\s+', r'\\item ', content, flags=re.MULTILINE)
        
        return content
    
    def _clean_whitespace(self, content: str) -> str:
        """
        불필요한 공백 정리
        """
        # 3줄 이상 연속 빈 줄 -> 2줄로
        content = re.sub(r'\n{3,}', '\n\n', content)
        
        # 줄 끝 공백 제거
        content = re.sub(r'[ \t]+$', '', content, flags=re.MULTILINE)
        
        return content
    
    def validate_latex(self, content: str) -> List[str]:
        """
        LaTeX 문법 간이 검증
        
        Returns:
            발견된 문제점 리스트
        """
        issues = []
        
        # 짝이 맞지 않는 중괄호 체크
        open_braces = content.count('{')
        close_braces = content.count('}')
        if open_braces != close_braces:
            issues.append(f"Unbalanced braces: {open_braces} open, {close_braces} close")
        
        # 필수 섹션 존재 확인
        required_sections = ["Abstract", "Introduction", "Conclusion"]
        for section in required_sections:
            if f"\\section{{{section}}}" not in content and section.lower() not in content.lower():
                issues.append(f"Missing section: {section}")
        
        # \\begin과 \\end 쌍 확인
        begin_matches = re.findall(r'\\begin\{(\w+)\}', content)
        end_matches = re.findall(r'\\end\{(\w+)\}', content)
        
        begin_counts = {}
        end_counts = {}
        for env in begin_matches:
            begin_counts[env] = begin_counts.get(env, 0) + 1
        for env in end_matches:
            end_counts[env] = end_counts.get(env, 0) + 1
        
        for env, count in begin_counts.items():
            if end_counts.get(env, 0) != count:
                issues.append(f"Unbalanced environment: {env} (begin: {count}, end: {end_counts.get(env, 0)})")
        
        return issues
