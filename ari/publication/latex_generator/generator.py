"""
LaTeX Generator

완전한 LaTeX 논문 생성
"""

from typing import List, Dict, Any, Optional
from pathlib import Path
from dataclasses import dataclass
import subprocess

import sys
import os
sys.path.append(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
from core.llm import LLMClient, get_llm_client
from core.logger import get_logger

from .sections import PaperSection, SectionGenerator, SectionType
from .bibliography import BibliographyManager, BibEntry
from .data_generator import SyntheticDataGenerator, ExperimentMetrics
from .latex_postprocessor import LaTeXPostProcessor
from .consistency_checker import ConsistencyChecker

logger = get_logger("latex_generator")


LATEX_TEMPLATE = r'''
\documentclass[11pt,a4paper]{{article}}
\usepackage[utf8]{{inputenc}}
\usepackage{{amsmath,amssymb}}
\usepackage{{graphicx}}
\usepackage{{hyperref}}
\usepackage{{natbib}}
\usepackage{{booktabs}}

\title{{{title}}}
\author{{{authors}}}
\date{{{date}}}

\begin{{document}}

\maketitle

\begin{{abstract}}
{abstract}
\end{{abstract}}

{body}

\bibliographystyle{{plainnat}}
\bibliography{{references}}

\end{{document}}
'''


@dataclass
class GeneratedPaper:
    """생성된 논문"""
    title: str
    authors: List[str]
    sections: List[PaperSection]
    bibliography: BibliographyManager
    
    latex_content: str = ""
    pdf_path: Optional[str] = None
    
    def save_latex(self, filepath: str):
        """LaTeX 파일 저장"""
        with open(filepath, 'w', encoding='utf-8') as f:
            f.write(self.latex_content)
    
    def save_all(self, output_dir: str) -> Dict[str, str]:
        """모든 파일 저장"""
        output_path = Path(output_dir)
        output_path.mkdir(parents=True, exist_ok=True)
        
        files = {}
        
        # LaTeX 저장
        tex_file = output_path / "paper.tex"
        self.save_latex(str(tex_file))
        files["tex"] = str(tex_file)
        
        # BibTeX 저장
        bib_file = output_path / "references.bib"
        self.bibliography.save(str(bib_file))
        files["bib"] = str(bib_file)
        
        return files


class LaTeXGenerator:
    """LaTeX 논문 생성기"""
    
    def __init__(
        self,
        llm_client: Optional[LLMClient] = None,
        template: str = None
    ):
        self.llm = llm_client or get_llm_client()
        self.section_generator = SectionGenerator(self.llm)
        self.bibliography = BibliographyManager()
        self.template = template or LATEX_TEMPLATE
        
        # 새로 추가된 모듈들
        self.data_generator = SyntheticDataGenerator()
        self.postprocessor = LaTeXPostProcessor()
        self.consistency_checker = ConsistencyChecker()
    
    def generate_paper(
        self,
        title: str,
        hypothesis: Dict[str, Any],
        experiment_results: Dict[str, Any],
        related_papers: List[Dict[str, Any]] = None,
        authors: List[str] = None
    ) -> GeneratedPaper:
        """
        논문 생성
        
        Args:
            title: 논문 제목
            hypothesis: 가설 정보
            experiment_results: 실험 결과
            related_papers: 관련 논문 정보
            authors: 저자 리스트
        
        Returns:
            GeneratedPaper 객체
        """
        authors = authors or ["ARI System"]
        
        # 1. 도메인에 맞는 합성 실험 데이터 생성
        domain = hypothesis.get("domain", "Machine Learning")
        keywords = hypothesis.get("keywords", [])
        
        metrics = self.data_generator.generate_metrics(
            domain=domain,
            hypothesis_keywords=keywords,
            num_metrics=4,
            hypothesis_title=title  # 문맥 인식 메트릭 선택을 위해 제목 전달
        )
        
        # 2. 실제 논문 검색하여 참고문헌 추가 (Semantic Scholar API)
        if related_papers:
            for paper in related_papers:
                self.bibliography.add_from_dict(paper)
        
        if self.bibliography.count() < 5:
            # 실제 논문 검색 (API 실패시 합성 참고문헌으로 폴백)
            self.bibliography.search_real_references(
                hypothesis_title=title,
                domain=domain,
                keywords=keywords,
                num_refs=8,
                fallback_to_synthetic=True
            )
        
        # 3. 테이블 및 결과 텍스트 생성 (새 API 사용)
        table_data = self.data_generator.generate_table_data(metrics)
        latex_table = self.data_generator.generate_latex_table(table_data)
        results_text = self.data_generator.format_results_text(metrics)
        baseline_definitions = self.data_generator.generate_baseline_definitions(metrics)
        
        # 4. 인과관계 설명 및 제안 알고리즘명 생성
        causal_explanation = self.data_generator.generate_causal_explanation(domain)
        proposed_algorithm = self.data_generator.get_proposed_algorithm(domain)
        methodology_context = self.data_generator.generate_methodology_context(domain)
        
        # 5. 인용 키 리스트 생성
        citation_keys = self.bibliography.get_all_keys()
        citation_text = ", ".join([f"\\cite{{{key}}}" for key in citation_keys[:5]])
        
        # 생성된 데이터 요약
        pm = metrics.primary_metric
        secondary_summary = ", ".join([f"{sm.name}={sm.value:.4f}" for sm in metrics.secondary_metrics])
        
        generated_data = f"""
Primary Metric: {pm.name} = {pm.value:.4f} (±{pm.std:.3f})
- Higher is better: {pm.higher_is_better}
Secondary Metrics: {secondary_summary}
Improvement: {metrics.improvement_text}

PROPOSED METHOD NAME: {proposed_algorithm}
(Use this specific name consistently throughout the paper)

TECHNICAL EXPLANATION (include in Methodology):
{causal_explanation}

Baseline Definitions (include in Methodology):
{baseline_definitions}

Comparison Table (include in Results):
{latex_table}

Results Narrative:
{results_text}

Use these citations in Related Work: {citation_text}
"""
        
        # 컨텍스트 구성
        context = {
            "title": title,
            "research_question": hypothesis.get("research_question", ""),
            "methodology": hypothesis.get("methodology", ""),
            "experiment_plan": hypothesis.get("experiment_plan", []),
            "domain_context": domain,
            "related_work": self._format_related_work(related_papers),
            "experiments": str(experiment_results.get("description", "")),
            "metrics": str(experiment_results.get("metrics", {})),
            "findings": experiment_results.get("findings", ""),
            "results": experiment_results.get("summary", ""),
            "contributions": hypothesis.get("expected_results", ""),
            "limitations": experiment_results.get("limitations", ""),
            "generated_data": generated_data,
            "baseline_definitions": baseline_definitions,
            "citation_keys": citation_keys,
        }
        
        # 섹션 생성
        sections = self.section_generator.generate_all_sections(context)
        
        # LaTeX 조립
        latex_content = self._assemble_latex(title, authors, sections)
        
        # 5. 후처리: 참조 수정, 중복 제거, 형식 통일
        latex_content = self.postprocessor.process(latex_content)
        
        paper = GeneratedPaper(
            title=title,
            authors=authors,
            sections=sections,
            bibliography=self.bibliography,
            latex_content=latex_content
        )
        
        return paper
    
    def _format_related_work(self, papers: List[Dict[str, Any]]) -> str:
        """관련 연구 포맷팅"""
        if not papers:
            return "No specific related work provided."
        
        formatted = []
        for paper in papers[:10]:
            title = paper.get("title", "")
            abstract = paper.get("abstract", "")[:200]
            formatted.append(f"- {title}: {abstract}")
        
        return "\n".join(formatted)
    
    def _assemble_latex(
        self,
        title: str,
        authors: List[str],
        sections: List[PaperSection]
    ) -> str:
        """LaTeX 조립"""
        # 초록 추출
        abstract = ""
        body_sections = []
        
        for section in sections:
            if section.section_type == SectionType.ABSTRACT:
                abstract = section.content
            else:
                body_sections.append(section)
        
        # 본문 조립
        body = ""
        for section in body_sections:
            body += section.to_latex() + "\n\n"
        
        # 인용 플레이스홀더 처리
        body = self._process_citations(body)
        
        # 템플릿 적용
        from datetime import datetime
        
        latex = self.template.format(
            title=self._escape_latex(title),
            authors=" \\and ".join(self._escape_latex(a) for a in authors),
            date=datetime.now().strftime("%B %Y"),
            abstract=abstract,
            body=body
        )
        
        return latex
    
    def _escape_latex(self, text: str) -> str:
        """LaTeX 특수 문자 이스케이프"""
        replacements = {
            '&': r'\&',
            '%': r'\%',
            '$': r'\$',
            '#': r'\#',
            '_': r'\_',
            '{': r'\{',
            '}': r'\}',
            '~': r'\textasciitilde{}',
            '^': r'\^{}',
        }
        
        for char, replacement in replacements.items():
            text = text.replace(char, replacement)
        
        return text
    
    def _process_citations(self, text: str) -> str:
        """인용 플레이스홀더 처리"""
        import re
        
        # [CITE: topic] -> \cite{generated_key}
        def replace_cite(match):
            topic = match.group(1).strip()
            # 토픽에서 키 생성
            key = topic.lower().replace(' ', '_')[:20]
            return f"\\cite{{{key}}}"
        
        text = re.sub(r'\[CITE:\s*([^\]]+)\]', replace_cite, text)
        
        return text
    
    def compile_to_pdf(
        self,
        paper: GeneratedPaper,
        output_dir: str
    ) -> Optional[str]:
        """
        PDF로 컴파일
        
        Args:
            paper: 생성된 논문
            output_dir: 출력 디렉토리
        
        Returns:
            PDF 파일 경로 또는 None
        """
        output_path = Path(output_dir).resolve()  # 절대 경로로 변환
        files = paper.save_all(str(output_path))
        
        tex_file = Path(files["tex"]).name  # 파일명만 사용 (paper.tex)
        
        try:
            # pdflatex 실행
            for _ in range(2):  # 참고문헌을 위해 두 번 실행
                result = subprocess.run(
                    ["pdflatex", "-interaction=nonstopmode", tex_file],
                    cwd=str(output_path),
                    capture_output=True,
                    timeout=120
                )
            
            # bibtex 실행
            subprocess.run(
                ["bibtex", "paper"],
                cwd=str(output_path),
                capture_output=True,
                timeout=30
            )
            
            # 다시 pdflatex
            subprocess.run(
                ["pdflatex", "-interaction=nonstopmode", tex_file],
                cwd=str(output_path),
                capture_output=True,
                timeout=120
            )
            
            # PDF 이름 변경 (paper.pdf -> sanitized_title.pdf)
            original_pdf = output_path / "paper.pdf"
            
            # 폴더명과 동일한 sanitized name 사용
            from core.utils import sanitize_filename
            sanitized_name = sanitize_filename(paper.title)[:80]
            new_pdf_name = f"{sanitized_name}.pdf"
            final_pdf_path = output_path / new_pdf_name
            
            if original_pdf.exists():
                original_pdf.rename(final_pdf_path)
                paper.pdf_path = str(final_pdf_path)
                return str(final_pdf_path)
            else:
                logger.warning(f"PDF not found at {original_pdf}")
        
        except subprocess.TimeoutExpired:
            logger.error("PDF compilation timed out")
        except FileNotFoundError:
            logger.error("pdflatex not found. Install TeX distribution.")
        except Exception as e:
            logger.error(f"PDF compilation failed: {e}")
        
        return None
    
    def add_reference(self, paper_info: Dict[str, Any]) -> str:
        """참고문헌 추가"""
        entry = self.bibliography.add_from_dict(paper_info)
        return entry.key
