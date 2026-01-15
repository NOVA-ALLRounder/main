"""
Reviewer Agent

다중 페르소나 리뷰어 에이전트
"""

from dataclasses import dataclass, field
from typing import List, Dict, Any, Optional
from enum import Enum

import sys
import os
sys.path.append(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
from core.llm import LLMClient, get_llm_client
from core.logger import get_logger

logger = get_logger("reviewer")


class ReviewerPersona(Enum):
    """리뷰어 페르소나"""
    SKEPTIC = "skeptic"           # 방법론 비판가
    BENCHMARK_NAZI = "benchmark"  # 비교 실험 전문가
    DOMAIN_EXPERT = "domain"      # 분야 전문가
    METHODOLOGIST = "method"      # 방법론 전문가
    CLARITY_REVIEWER = "clarity"  # 명확성 리뷰어


@dataclass
class ReviewResult:
    """리뷰 결과"""
    reviewer_persona: ReviewerPersona
    
    # 점수 (1-10)
    overall_score: float = 5.0
    novelty_score: float = 5.0
    soundness_score: float = 5.0
    significance_score: float = 5.0
    clarity_score: float = 5.0
    
    # 상세 리뷰
    summary: str = ""
    strengths: List[str] = field(default_factory=list)
    weaknesses: List[str] = field(default_factory=list)
    questions: List[str] = field(default_factory=list)
    
    # 결정
    decision: str = "weak_reject"  # accept, weak_accept, weak_reject, reject
    confidence: float = 0.5
    
    # 추가 실험 요청
    additional_experiments: List[str] = field(default_factory=list)
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "reviewer_persona": self.reviewer_persona.value,
            "overall_score": self.overall_score,
            "novelty_score": self.novelty_score,
            "soundness_score": self.soundness_score,
            "significance_score": self.significance_score,
            "clarity_score": self.clarity_score,
            "summary": self.summary,
            "strengths": self.strengths,
            "weaknesses": self.weaknesses,
            "questions": self.questions,
            "decision": self.decision,
            "confidence": self.confidence,
            "additional_experiments": self.additional_experiments
        }


PERSONA_PROMPTS = {
    ReviewerPersona.SKEPTIC: """You are a SKEPTIC reviewer - highly critical, looking for flaws.
You focus on:
- Methodological weaknesses and potential flaws
- Overlooked assumptions
- Reproducibility concerns
- Statistical validity of claims
Be constructively critical but thorough.""",

    ReviewerPersona.BENCHMARK_NAZI: """You are a BENCHMARK reviewer - obsessed with fair comparisons.
You focus on:
- Are baselines appropriate and state-of-the-art?
- Are comparisons fair (same compute, same hyperparameter tuning)?
- Are there missing important baselines?
- Are ablation studies sufficient?
Be thorough about experimental rigor.""",

    ReviewerPersona.DOMAIN_EXPERT: """You are a DOMAIN EXPERT reviewer - deep knowledge of the field.
You focus on:
- Does this fit with the existing literature?
- Is the problem formulation correct?
- Are there relevant works that should be cited?
- Does this advance the state of the field?
Evaluate from a deep domain perspective.""",

    ReviewerPersona.METHODOLOGIST: """You are a METHODOLOGIST reviewer - focused on technical correctness.
You focus on:
- Is the math correct?
- Are the proofs valid (if any)?
- Is the algorithm description complete?
- Are the complexity claims accurate?
Be rigorous about technical details.""",

    ReviewerPersona.CLARITY_REVIEWER: """You are a CLARITY reviewer - focused on presentation quality.
You focus on:
- Is the paper well-written and organized?
- Are figures and tables clear and informative?
- Can the work be reproduced from the description?
- Is the contribution clearly stated?
Evaluate communication quality."""
}


class ReviewerAgent:
    """리뷰어 에이전트"""
    
    REVIEW_PROMPT = """You are reviewing a scientific paper submission.

{persona_prompt}

Paper Content:
Title: {title}
Abstract: {abstract}

{sections}

Provide your review in the following JSON format:
{{
    "overall_score": 1-10,
    "novelty_score": 1-10,
    "soundness_score": 1-10,
    "significance_score": 1-10,
    "clarity_score": 1-10,
    "summary": "Brief summary of the paper and your assessment",
    "strengths": ["strength1", "strength2", ...],
    "weaknesses": ["weakness1", "weakness2", ...],
    "questions": ["question1", "question2", ...],
    "decision": "accept" | "weak_accept" | "weak_reject" | "reject",
    "confidence": 0.0-1.0,
    "additional_experiments": ["experiment1", "experiment2", ...]
}}
"""
    
    def __init__(
        self,
        persona: ReviewerPersona,
        llm_client: Optional[LLMClient] = None
    ):
        self.persona = persona
        self.llm = llm_client or get_llm_client()
        self.persona_prompt = PERSONA_PROMPTS.get(persona, "")
    
    def review(
        self,
        title: str,
        abstract: str,
        sections: Dict[str, str]
    ) -> ReviewResult:
        """
        논문 리뷰
        
        Args:
            title: 논문 제목
            abstract: 초록
            sections: 섹션 딕셔너리 {섹션명: 내용}
        
        Returns:
            ReviewResult 객체
        """
        # 섹션 포맷팅
        sections_text = ""
        for section_name, content in sections.items():
            # 너무 길면 잘라내기
            content_truncated = content[:2000] if len(content) > 2000 else content
            sections_text += f"\n\n### {section_name}\n{content_truncated}"
        
        prompt = self.REVIEW_PROMPT.format(
            persona_prompt=self.persona_prompt,
            title=title,
            abstract=abstract,
            sections=sections_text
        )
        
        try:
            response = self.llm.generate_json(
                prompt=prompt,
                system_prompt="You are an expert scientific peer reviewer providing constructive and thorough feedback.",
                temperature=0.5
            )
            
            return ReviewResult(
                reviewer_persona=self.persona,
                overall_score=float(response.get("overall_score", 5)),
                novelty_score=float(response.get("novelty_score", 5)),
                soundness_score=float(response.get("soundness_score", 5)),
                significance_score=float(response.get("significance_score", 5)),
                clarity_score=float(response.get("clarity_score", 5)),
                summary=response.get("summary", ""),
                strengths=response.get("strengths", []),
                weaknesses=response.get("weaknesses", []),
                questions=response.get("questions", []),
                decision=response.get("decision", "weak_reject"),
                confidence=float(response.get("confidence", 0.5)),
                additional_experiments=response.get("additional_experiments", [])
            )
        
        except Exception as e:
            logger.error(f"Review failed: {e}")
            return ReviewResult(
                reviewer_persona=self.persona,
                summary=f"Review failed: {str(e)}",
                weaknesses=[f"Unable to complete review: {str(e)}"]
            )


class MultiReviewerPanel:
    """다중 리뷰어 패널"""
    
    def __init__(
        self,
        personas: List[ReviewerPersona] = None,
        llm_client: Optional[LLMClient] = None
    ):
        self.llm = llm_client or get_llm_client()
        
        # 기본 페르소나
        if personas is None:
            personas = [
                ReviewerPersona.SKEPTIC,
                ReviewerPersona.BENCHMARK_NAZI,
                ReviewerPersona.DOMAIN_EXPERT
            ]
        
        self.reviewers = [
            ReviewerAgent(persona, self.llm) for persona in personas
        ]
    
    def review_paper(
        self,
        title: str,
        abstract: str,
        sections: Dict[str, str]
    ) -> List[ReviewResult]:
        """모든 리뷰어로 리뷰"""
        results = []
        
        for reviewer in self.reviewers:
            result = reviewer.review(title, abstract, sections)
            results.append(result)
        
        return results
    
    def get_aggregate_decision(self, reviews: List[ReviewResult]) -> Dict[str, Any]:
        """리뷰 종합"""
        if not reviews:
            return {"decision": "reject", "avg_score": 0}
        
        # 점수 평균
        avg_overall = sum(r.overall_score for r in reviews) / len(reviews)
        avg_novelty = sum(r.novelty_score for r in reviews) / len(reviews)
        avg_soundness = sum(r.soundness_score for r in reviews) / len(reviews)
        
        # 결정 집계
        decision_weights = {
            "accept": 3,
            "weak_accept": 2,
            "weak_reject": 1,
            "reject": 0
        }
        
        weighted_sum = sum(
            decision_weights.get(r.decision, 1) for r in reviews
        )
        avg_decision_weight = weighted_sum / len(reviews)
        
        if avg_decision_weight >= 2.5:
            final_decision = "accept"
        elif avg_decision_weight >= 1.5:
            final_decision = "weak_accept"
        elif avg_decision_weight >= 0.5:
            final_decision = "weak_reject"
        else:
            final_decision = "reject"
        
        # 모든 약점 수집
        all_weaknesses = []
        for r in reviews:
            all_weaknesses.extend(r.weaknesses)
        
        # 추가 실험 요청 수집
        all_experiments = []
        for r in reviews:
            all_experiments.extend(r.additional_experiments)
        
        return {
            "final_decision": final_decision,
            "avg_overall_score": avg_overall,
            "avg_novelty_score": avg_novelty,
            "avg_soundness_score": avg_soundness,
            "all_weaknesses": all_weaknesses,
            "required_experiments": list(set(all_experiments)),
            "num_reviewers": len(reviews)
        }
