"""
Hypothesis Generator

LLM 기반 가설 생성
"""

from dataclasses import dataclass, field
from typing import List, Dict, Any, Optional
import json
from datetime import datetime

import sys
import os
sys.path.append(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
from core.llm import LLMClient, get_llm_client


@dataclass
class Hypothesis:
    """연구 가설"""
    
    # 기본 정보
    hypothesis_id: str = ""
    title: str = ""
    research_question: str = ""
    
    # 방법론
    methodology: str = ""
    experiment_plan: List[str] = field(default_factory=list)
    expected_results: str = ""
    
    # 평가 점수
    novelty_score: float = 0.0
    feasibility_score: float = 0.0
    impact_score: float = 0.0
    
    # 관련 정보
    related_papers: List[str] = field(default_factory=list)
    keywords: List[str] = field(default_factory=list)
    domain: str = ""
    
    # 메타데이터
    created_at: Optional[datetime] = None
    source: str = ""  # user, lbd, recursive
    status: str = "proposed"  # proposed, approved, in_progress, completed, rejected
    
    def __post_init__(self):
        if not self.created_at:
            self.created_at = datetime.now()
        if not self.hypothesis_id:
            import hashlib
            content = f"{self.title}{self.research_question}"
            self.hypothesis_id = hashlib.md5(content.encode()).hexdigest()[:12]
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "hypothesis_id": self.hypothesis_id,
            "title": self.title,
            "research_question": self.research_question,
            "methodology": self.methodology,
            "experiment_plan": self.experiment_plan,
            "expected_results": self.expected_results,
            "novelty_score": self.novelty_score,
            "feasibility_score": self.feasibility_score,
            "impact_score": self.impact_score,
            "related_papers": self.related_papers,
            "keywords": self.keywords,
            "domain": self.domain,
            "created_at": self.created_at.isoformat() if self.created_at else None,
            "source": self.source,
            "status": self.status
        }
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "Hypothesis":
        created_at = None
        if data.get("created_at"):
            try:
                created_at = datetime.fromisoformat(data["created_at"])
            except ValueError:
                pass
        
        return cls(
            hypothesis_id=data.get("hypothesis_id", ""),
            title=data.get("title", ""),
            research_question=data.get("research_question", ""),
            methodology=data.get("methodology", ""),
            experiment_plan=data.get("experiment_plan", []),
            expected_results=data.get("expected_results", ""),
            novelty_score=data.get("novelty_score", 0.0),
            feasibility_score=data.get("feasibility_score", 0.0),
            impact_score=data.get("impact_score", 0.0),
            related_papers=data.get("related_papers", []),
            keywords=data.get("keywords", []),
            domain=data.get("domain", ""),
            created_at=created_at,
            source=data.get("source", ""),
            status=data.get("status", "proposed")
        )
    
    def overall_score(self) -> float:
        """종합 점수"""
        return (self.novelty_score + self.feasibility_score + self.impact_score) / 3


class HypothesisGenerator:
    """LLM 기반 가설 생성기"""
    
    GENERATION_PROMPT = """You are a research scientist generating novel research hypotheses.

Based on the following research context, generate a detailed research hypothesis.

Research Context:
{context}

Domain: {domain}

Requirements:
1. The hypothesis should be novel and not simply restate existing work
2. It should be feasible to test with computational experiments
3. It should have potential for significant impact

Generate a hypothesis in the following JSON format:
{{
    "title": "Clear, descriptive title for the research",
    "research_question": "The central question this research aims to answer",
    "methodology": "Detailed description of the proposed approach",
    "experiment_plan": [
        "Step 1: ...",
        "Step 2: ...",
        "Step 3: ..."
    ],
    "expected_results": "What you expect to find and why it matters",
    "novelty_score": 0.0-10.0,
    "feasibility_score": 0.0-10.0,
    "impact_score": 0.0-10.0,
    "keywords": ["keyword1", "keyword2", ...]
}}
"""
    
    def __init__(self, llm_client: Optional[LLMClient] = None):
        self.llm = llm_client or get_llm_client()
    
    def generate(
        self,
        context: str,
        domain: str = "Machine Learning",
        num_hypotheses: int = 1
    ) -> List[Hypothesis]:
        """
        가설 생성
        
        Args:
            context: 연구 문맥 (문헌 요약, 기존 연구 등)
            domain: 연구 분야
            num_hypotheses: 생성할 가설 수
        
        Returns:
            생성된 가설 리스트
        """
        hypotheses = []
        
        for _ in range(num_hypotheses):
            prompt = self.GENERATION_PROMPT.format(
                context=context,
                domain=domain
            )
            
            try:
                response = self.llm.generate_json(
                    prompt=prompt,
                    system_prompt="You are an expert research scientist with deep knowledge in multiple domains.",
                    temperature=0.7
                )
                
                hypothesis = Hypothesis(
                    title=response.get("title", ""),
                    research_question=response.get("research_question", ""),
                    methodology=response.get("methodology", ""),
                    experiment_plan=response.get("experiment_plan", []),
                    expected_results=response.get("expected_results", ""),
                    novelty_score=float(response.get("novelty_score", 5.0)),
                    feasibility_score=float(response.get("feasibility_score", 5.0)),
                    impact_score=float(response.get("impact_score", 5.0)),
                    keywords=response.get("keywords", []),
                    domain=domain,
                    source="generated"
                )
                
                if hypothesis.title and hypothesis.research_question:
                    hypotheses.append(hypothesis)
            
            except Exception as e:
                print(f"Hypothesis generation error: {e}")
        
        return hypotheses
    
    def generate_from_gap(
        self,
        existing_work: str,
        limitations: str,
        domain: str = "Machine Learning"
    ) -> List[Hypothesis]:
        """
        기존 연구의 한계점에서 가설 생성
        
        Args:
            existing_work: 기존 연구 요약
            limitations: 한계점
            domain: 연구 분야
        
        Returns:
            생성된 가설 리스트
        """
        context = f"""
Existing Work:
{existing_work}

Known Limitations:
{limitations}

Generate a hypothesis that addresses these limitations and advances the field.
"""
        return self.generate(context, domain)
    
    def generate_from_papers(
        self,
        paper_abstracts: List[str],
        domain: str = "Machine Learning"
    ) -> List[Hypothesis]:
        """
        논문 초록들에서 가설 생성
        
        Args:
            paper_abstracts: 논문 초록 리스트
            domain: 연구 분야
        
        Returns:
            생성된 가설 리스트
        """
        context = "Recent related work:\n\n"
        for i, abstract in enumerate(paper_abstracts[:5], 1):
            context += f"Paper {i}:\n{abstract[:500]}...\n\n"
        
        context += "Based on these papers, identify a research gap and propose a novel hypothesis."
        
        return self.generate(context, domain)
    
    def refine_hypothesis(
        self,
        hypothesis: Hypothesis,
        feedback: str
    ) -> Hypothesis:
        """
        피드백을 바탕으로 가설 개선
        
        Args:
            hypothesis: 원본 가설
            feedback: 개선 피드백
        
        Returns:
            개선된 가설
        """
        prompt = f"""
Original Hypothesis:
Title: {hypothesis.title}
Research Question: {hypothesis.research_question}
Methodology: {hypothesis.methodology}

Feedback:
{feedback}

Refine the hypothesis based on the feedback. Return in JSON format with the same structure.
"""
        
        try:
            response = self.llm.generate_json(
                prompt=prompt,
                system_prompt="You are refining a research hypothesis based on constructive feedback.",
                temperature=0.5
            )
            
            refined = Hypothesis(
                title=response.get("title", hypothesis.title),
                research_question=response.get("research_question", hypothesis.research_question),
                methodology=response.get("methodology", hypothesis.methodology),
                experiment_plan=response.get("experiment_plan", hypothesis.experiment_plan),
                expected_results=response.get("expected_results", hypothesis.expected_results),
                novelty_score=float(response.get("novelty_score", hypothesis.novelty_score)),
                feasibility_score=float(response.get("feasibility_score", hypothesis.feasibility_score)),
                impact_score=float(response.get("impact_score", hypothesis.impact_score)),
                keywords=response.get("keywords", hypothesis.keywords),
                domain=hypothesis.domain,
                source="refined"
            )
            
            return refined
        
        except Exception:
            return hypothesis
