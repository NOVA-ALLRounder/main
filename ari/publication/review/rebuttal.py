"""
Rebuttal Manager

반박 및 수정 루프 관리
"""

from dataclasses import dataclass, field
from typing import List, Dict, Any, Optional, Tuple
from enum import Enum

import sys
import os
sys.path.append(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
from core.llm import LLMClient, get_llm_client
from core.logger import get_logger

from .reviewer import ReviewResult

logger = get_logger("rebuttal")


class ResponseType(Enum):
    """응답 유형"""
    CLARIFICATION = "clarification"      # 설명 보완
    ADDITIONAL_EXPERIMENT = "experiment"  # 추가 실험
    TEXT_REVISION = "revision"           # 텍스트 수정
    REBUTTAL = "rebuttal"               # 반박


@dataclass
class RebuttalResponse:
    """반박/응답 항목"""
    weakness_addressed: str
    response_type: ResponseType
    response_text: str
    
    # 추가 실험이 필요한 경우
    experiment_needed: bool = False
    experiment_description: str = ""
    
    # 관련 수정
    sections_to_modify: List[str] = field(default_factory=list)
    modifications: str = ""


@dataclass
class RebuttalLetter:
    """반박 레터"""
    responses: List[RebuttalResponse]
    summary: str = ""
    
    # 수정 사항
    text_modifications: Dict[str, str] = field(default_factory=dict)
    additional_experiments: List[str] = field(default_factory=list)
    
    def to_text(self) -> str:
        """텍스트 형식으로 변환"""
        lines = ["# Response to Reviewers\n"]
        lines.append(f"{self.summary}\n")
        
        for i, response in enumerate(self.responses, 1):
            lines.append(f"## Response {i}")
            lines.append(f"**Concern:** {response.weakness_addressed}")
            lines.append(f"**Response Type:** {response.response_type.value}")
            lines.append(f"**Response:** {response.response_text}\n")
        
        if self.additional_experiments:
            lines.append("## Additional Experiments Conducted")
            for exp in self.additional_experiments:
                lines.append(f"- {exp}")
        
        return "\n".join(lines)


class RebuttalManager:
    """반박/수정 관리자"""
    
    REBUTTAL_PROMPT = """You are the author of a scientific paper responding to reviewer feedback.

Reviewer's concern:
{concern}

Your paper's relevant section:
{relevant_section}

Determine the best response strategy:
1. CLARIFICATION - If there's a misunderstanding, clarify
2. EXPERIMENT - If valid criticism, propose/conduct additional experiment
3. REVISION - If the writing is unclear, revise the text
4. REBUTTAL - If you disagree with the criticism, explain why

Provide your response in JSON format:
{{
    "response_type": "clarification" | "experiment" | "revision" | "rebuttal",
    "response_text": "Your detailed response to the reviewer",
    "experiment_needed": true/false,
    "experiment_description": "Description of needed experiment (if any)",
    "sections_to_modify": ["section1", "section2"],
    "modifications": "Specific text changes to make"
}}
"""
    
    REVISION_PROMPT = """Revise the following paper section based on reviewer feedback.

Original section:
{original}

Reviewer feedback:
{feedback}

Suggested changes:
{suggestions}

Write the revised section that addresses the feedback while maintaining academic quality.
"""
    
    def __init__(
        self,
        llm_client: Optional[LLMClient] = None,
        max_revision_rounds: int = 3
    ):
        self.llm = llm_client or get_llm_client()
        self.max_revision_rounds = max_revision_rounds
        self.revision_history: List[Dict[str, Any]] = []
    
    def analyze_reviews(
        self,
        reviews: List[ReviewResult]
    ) -> Dict[str, Any]:
        """
        리뷰 분석 및 대응 전략 수립
        
        Args:
            reviews: 리뷰 결과 리스트
        
        Returns:
            분석 결과 및 전략
        """
        all_weaknesses = []
        all_questions = []
        all_experiments = []
        
        for review in reviews:
            all_weaknesses.extend(review.weaknesses)
            all_questions.extend(review.questions)
            all_experiments.extend(review.additional_experiments)
        
        # 중복 제거
        unique_weaknesses = list(set(all_weaknesses))
        unique_questions = list(set(all_questions))
        unique_experiments = list(set(all_experiments))
        
        # 심각도 분류
        critical_issues = [w for w in unique_weaknesses if any(
            kw in w.lower() for kw in ["fatal", "serious", "major", "fundamental"]
        )]
        
        minor_issues = [w for w in unique_weaknesses if w not in critical_issues]
        
        return {
            "critical_issues": critical_issues,
            "minor_issues": minor_issues,
            "questions": unique_questions,
            "required_experiments": unique_experiments,
            "total_reviews": len(reviews),
            "needs_major_revision": len(critical_issues) > 0 or len(unique_experiments) > 2
        }
    
    def generate_rebuttal(
        self,
        reviews: List[ReviewResult],
        paper_sections: Dict[str, str]
    ) -> RebuttalLetter:
        """
        반박 레터 생성
        
        Args:
            reviews: 리뷰 결과 리스트
            paper_sections: 논문 섹션 딕셔너리
        
        Returns:
            RebuttalLetter 객체
        """
        analysis = self.analyze_reviews(reviews)
        
        responses = []
        all_modifications = {}
        all_experiments = []
        
        # 각 약점에 대응
        all_concerns = analysis["critical_issues"] + analysis["minor_issues"]
        
        for concern in all_concerns:
            response = self._generate_response(concern, paper_sections)
            responses.append(response)
            
            if response.experiment_needed:
                all_experiments.append(response.experiment_description)
            
            for section in response.sections_to_modify:
                if section in paper_sections:
                    if section not in all_modifications:
                        all_modifications[section] = []
                    all_modifications[section].append(response.modifications)
        
        # 질문에 대응
        for question in analysis["questions"][:5]:
            response = RebuttalResponse(
                weakness_addressed=question,
                response_type=ResponseType.CLARIFICATION,
                response_text=self._answer_question(question, paper_sections)
            )
            responses.append(response)
        
        # 요약 생성
        summary = self._generate_summary(analysis, responses)
        
        return RebuttalLetter(
            responses=responses,
            summary=summary,
            text_modifications=all_modifications,
            additional_experiments=all_experiments
        )
    
    def _generate_response(
        self,
        concern: str,
        paper_sections: Dict[str, str]
    ) -> RebuttalResponse:
        """개별 우려사항에 대한 응답 생성"""
        # 가장 관련된 섹션 찾기
        relevant_section = ""
        relevant_section_name = ""
        
        concern_lower = concern.lower()
        section_priorities = {
            "methodology": ["method", "approach", "algorithm"],
            "experiments": ["experiment", "baseline", "comparison", "result"],
            "introduction": ["motivation", "contribution", "novel"],
            "conclusion": ["limitation", "future"]
        }
        
        for section_name, keywords in section_priorities.items():
            if any(kw in concern_lower for kw in keywords):
                if section_name in paper_sections:
                    relevant_section = paper_sections[section_name][:1000]
                    relevant_section_name = section_name
                    break
        
        if not relevant_section:
            # 첫 번째 섹션 사용
            for name, content in paper_sections.items():
                relevant_section = content[:1000]
                relevant_section_name = name
                break
        
        prompt = self.REBUTTAL_PROMPT.format(
            concern=concern,
            relevant_section=relevant_section
        )
        
        try:
            response = self.llm.generate_json(
                prompt=prompt,
                system_prompt="You are a paper author responding diplomatically and constructively to criticism.",
                temperature=0.4
            )
            
            return RebuttalResponse(
                weakness_addressed=concern,
                response_type=ResponseType(response.get("response_type", "clarification")),
                response_text=response.get("response_text", ""),
                experiment_needed=response.get("experiment_needed", False),
                experiment_description=response.get("experiment_description", ""),
                sections_to_modify=response.get("sections_to_modify", []),
                modifications=response.get("modifications", "")
            )
        
        except Exception as e:
            return RebuttalResponse(
                weakness_addressed=concern,
                response_type=ResponseType.CLARIFICATION,
                response_text=f"We acknowledge this concern and will address it. [Error: {e}]"
            )
    
    def _answer_question(
        self,
        question: str,
        paper_sections: Dict[str, str]
    ) -> str:
        """질문에 답변"""
        context = "\n".join([
            f"{name}: {content[:500]}" 
            for name, content in list(paper_sections.items())[:3]
        ])
        
        prompt = f"""Answer this reviewer question about your paper:

Question: {question}

Paper context:
{context}

Provide a clear, concise answer.
"""
        
        try:
            return self.llm.complete(
                prompt=prompt,
                system_prompt="You are answering reviewer questions about your paper.",
                temperature=0.3
            )
        except Exception:
            return "We appreciate this question and will clarify in the revision."
    
    def _generate_summary(
        self,
        analysis: Dict[str, Any],
        responses: List[RebuttalResponse]
    ) -> str:
        """반박 레터 요약 생성"""
        num_experiments = sum(1 for r in responses if r.experiment_needed)
        
        summary = f"""We thank the reviewers for their valuable feedback.

Key points addressed:
- {len(analysis['critical_issues'])} critical issues addressed
- {len(analysis['minor_issues'])} minor issues clarified
- {len(analysis['questions'])} questions answered
- {num_experiments} additional experiments {'conducted' if num_experiments > 0 else 'N/A'}

We have revised the paper accordingly and believe it now addresses all concerns.
"""
        return summary
    
    def apply_revisions(
        self,
        original_sections: Dict[str, str],
        rebuttal: RebuttalLetter
    ) -> Dict[str, str]:
        """
        수정 사항 적용
        
        Args:
            original_sections: 원본 섹션
            rebuttal: 반박 레터
        
        Returns:
            수정된 섹션
        """
        revised_sections = dict(original_sections)
        
        for section_name, modifications in rebuttal.text_modifications.items():
            if section_name in revised_sections:
                original = revised_sections[section_name]
                feedback = "\n".join(modifications)
                
                prompt = self.REVISION_PROMPT.format(
                    original=original,
                    feedback=feedback,
                    suggestions=feedback
                )
                
                try:
                    revised = self.llm.complete(
                        prompt=prompt,
                        system_prompt="You are revising academic text based on feedback.",
                        temperature=0.3
                    )
                    revised_sections[section_name] = revised
                except Exception:
                    pass
        
        # 히스토리 기록
        self.revision_history.append({
            "round": len(self.revision_history) + 1,
            "modifications": list(rebuttal.text_modifications.keys()),
            "experiments_added": rebuttal.additional_experiments
        })
        
        return revised_sections
