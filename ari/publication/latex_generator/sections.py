"""
Paper Sections

논문 섹션별 생성
"""

from dataclasses import dataclass, field
from typing import List, Dict, Any, Optional
from enum import Enum

import sys
import os
sys.path.append(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
from core.llm import LLMClient, get_llm_client


class SectionType(Enum):
    """논문 섹션 유형"""
    ABSTRACT = "abstract"
    INTRODUCTION = "introduction"
    RELATED_WORK = "related_work"
    METHODOLOGY = "methodology"
    EXPERIMENTS = "experiments"
    RESULTS = "results"
    DISCUSSION = "discussion"
    CONCLUSION = "conclusion"
    FUTURE_WORK = "future_work"


@dataclass
class PaperSection:
    """논문 섹션"""
    section_type: SectionType
    title: str
    content: str
    subsections: List["PaperSection"] = field(default_factory=list)
    citations: List[str] = field(default_factory=list)  # BibTeX keys
    figures: List[str] = field(default_factory=list)  # Figure references
    tables: List[str] = field(default_factory=list)  # Table references
    
    def to_latex(self) -> str:
        """LaTeX로 변환"""
        level = "section"
        
        latex = f"\\{level}{{{self.title}}}\n\n"
        latex += self.content + "\n"
        
        for subsection in self.subsections:
            latex += f"\n\\subsection{{{subsection.title}}}\n\n"
            latex += subsection.content + "\n"
        
        return latex


class SectionGenerator:
    """섹션 생성기"""
    
    PROMPTS = {
        SectionType.ABSTRACT: """Write a concise abstract for a scientific paper.

Title: {title}
Research Question: {research_question}
Methodology: {methodology}
Key Results: {results}

The abstract should:
1. State the problem (1-2 sentences)
2. Describe the approach (1-2 sentences)  
3. Summarize key findings (2-3 sentences)
4. Conclude with significance (1 sentence)

Return only the abstract text (no LaTeX commands), approximately 150-250 words.
""",
        
        SectionType.INTRODUCTION: """Write the Introduction section for a scientific paper.

Title: {title}
Research Question: {research_question}
Domain Context: {domain_context}
Related Work Summary: {related_work}
Available Citations: {citation_keys}

The introduction should:
1. Establish the research context and motivation
2. Identify the research gap or problem
3. State the research objectives and contributions
4. Outline the paper structure

CITATION RULES:
- Use \cite{{key}} format with the provided citation keys
- Include at least 3 citations from the Available Citations list
- DO NOT use placeholders like [Author et al.] or [?]

Return as LaTeX-compatible text (2-4 paragraphs).
""",
        
        SectionType.METHODOLOGY: """Write the Methodology section for a scientific paper.

Research Question: {research_question}
Proposed Approach: {methodology}
Experiment Plan: {experiment_plan}
Baseline Definitions: {baseline_definitions}

The methodology section should:
1. Describe the overall approach clearly
2. Explain the algorithm/model architecture with specific names
3. Detail implementation specifics (frameworks, hyperparameters)
4. Describe the experimental setup (datasets, evaluation metrics)
5. DEFINE ALL BASELINE METHODS using the provided Baseline Definitions

IMPORTANT:
- Include the Baseline Definitions section exactly as provided
- Clearly name all algorithms, models, and techniques
- These names MUST be used consistently in Experiments and Results sections
- Include specific framework names (e.g., PyTorch, TensorFlow)
- Define all acronyms on first use

Include mathematical notation where appropriate using LaTeX math mode.
Return as LaTeX-compatible text.
""",
        
        SectionType.RESULTS: """Write the Results section for a scientific paper.

Experiment Description: {experiments}
Metrics and Values: {metrics}
Key Findings: {findings}
Generated Data Summary: {generated_data}

The results section should:
1. Present quantitative results with SPECIFIC NUMBERS (e.g., "achieved 94.5% accuracy")
2. Include the provided table data exactly as given
3. Compare with baselines using actual percentage improvements
4. Reference Table 1 using \\ref{{tab:results}}

CRITICAL RULES:
- DO NOT use placeholder values like "X.XX", "$X \\pm Y$", "Metric 1", "Metric 2"
- DO NOT leave any "??" references
- USE the specific metric values provided in "Generated Data Summary"
- Every claim must have a concrete number

Return as LaTeX-compatible text with the provided table included.
""",
        
        SectionType.CONCLUSION: """Write the Conclusion section for a scientific paper.

Title: {title}
Key Contributions: {contributions}
Main Results: {results}
Limitations: {limitations}

The conclusion should:
1. Summarize the main contributions
2. Restate key findings
3. Discuss limitations honestly
4. Suggest future research directions

Return as LaTeX-compatible text (2-3 paragraphs).
"""
    }
    
    def __init__(self, llm_client: Optional[LLMClient] = None):
        self.llm = llm_client or get_llm_client()
    
    def generate_section(
        self,
        section_type: SectionType,
        context: Dict[str, Any]
    ) -> PaperSection:
        """
        섹션 생성
        
        Args:
            section_type: 섹션 유형
            context: 섹션 생성에 필요한 컨텍스트
        
        Returns:
            PaperSection 객체
        """
        prompt_template = self.PROMPTS.get(section_type)
        
        if not prompt_template:
            # Default prompt
            prompt = f"Write the {section_type.value} section for a scientific paper.\n\nContext: {context}"
        else:
            prompt = prompt_template.format(**context)
        
        content = self.llm.complete(
            prompt=prompt,
            system_prompt="You are an expert scientific writer. Write clear, precise, and well-structured academic text.",
            temperature=0.5
        )
        
        # 인용 추출
        citations = self._extract_citations(content)
        
        # 제목 생성
        title = self._get_section_title(section_type)
        
        return PaperSection(
            section_type=section_type,
            title=title,
            content=content,
            citations=citations
        )
    
    def _get_section_title(self, section_type: SectionType) -> str:
        """섹션 제목 반환"""
        titles = {
            SectionType.ABSTRACT: "Abstract",
            SectionType.INTRODUCTION: "Introduction",
            SectionType.RELATED_WORK: "Related Work",
            SectionType.METHODOLOGY: "Methodology",
            SectionType.EXPERIMENTS: "Experiments",
            SectionType.RESULTS: "Results",
            SectionType.DISCUSSION: "Discussion",
            SectionType.CONCLUSION: "Conclusion",
            SectionType.FUTURE_WORK: "Future Work",
        }
        return titles.get(section_type, section_type.value.replace("_", " ").title())
    
    def _extract_citations(self, content: str) -> List[str]:
        """컨텐츠에서 인용 추출"""
        import re
        
        citations = []
        
        # [CITE: topic] 패턴
        cite_placeholders = re.findall(r'\[CITE:\s*([^\]]+)\]', content)
        citations.extend(cite_placeholders)
        
        # \cite{key} 패턴
        cite_keys = re.findall(r'\\cite\{([^}]+)\}', content)
        citations.extend(cite_keys)
        
        return list(set(citations))
    
    def generate_all_sections(
        self,
        paper_context: Dict[str, Any]
    ) -> List[PaperSection]:
        """모든 섹션 생성"""
        sections = []
        
        section_order = [
            SectionType.ABSTRACT,
            SectionType.INTRODUCTION,
            SectionType.RELATED_WORK,
            SectionType.METHODOLOGY,
            SectionType.EXPERIMENTS,
            SectionType.RESULTS,
            SectionType.DISCUSSION,
            SectionType.CONCLUSION,
        ]
        
        for section_type in section_order:
            section = self.generate_section(section_type, paper_context)
            sections.append(section)
        
        return sections
