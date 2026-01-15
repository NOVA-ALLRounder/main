"""
Novelty Checker

Semantic Scholar API를 이용한 가설 신규성 검증
"""

from dataclasses import dataclass
from typing import List, Dict, Any, Optional
import asyncio

import sys
import os
sys.path.append(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
from core.llm import LLMClient, get_llm_client
from cognitive.crawler.semantic_scholar import SemanticScholarClient


@dataclass
class NoveltyResult:
    """신규성 검증 결과"""
    hypothesis: str
    is_novel: bool
    novelty_score: float  # 0-10
    
    similar_papers: List[Dict[str, Any]]
    most_similar_paper: Optional[Dict[str, Any]]
    
    differentiation_points: List[str]
    overlap_points: List[str]
    
    recommendation: str  # accept, modify, reject
    feedback: str
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "hypothesis": self.hypothesis,
            "is_novel": self.is_novel,
            "novelty_score": self.novelty_score,
            "similar_papers": self.similar_papers,
            "most_similar_paper": self.most_similar_paper,
            "differentiation_points": self.differentiation_points,
            "overlap_points": self.overlap_points,
            "recommendation": self.recommendation,
            "feedback": self.feedback
        }


class NoveltyChecker:
    """가설 신규성 검증기"""
    
    EVALUATION_PROMPT = """You are evaluating the novelty of a research hypothesis by comparing it to existing literature.

Hypothesis:
{hypothesis}

Similar Existing Papers:
{papers}

Evaluate the novelty of this hypothesis:
1. What are the key differentiation points from existing work?
2. What aspects overlap with existing work?
3. Is this hypothesis novel enough to pursue?

Return your evaluation in JSON format:
{{
    "novelty_score": 0.0-10.0,
    "differentiation_points": ["point1", "point2", ...],
    "overlap_points": ["point1", "point2", ...],
    "recommendation": "accept" | "modify" | "reject",
    "feedback": "Detailed feedback and suggestions"
}}
"""
    
    def __init__(
        self,
        semantic_scholar: Optional[SemanticScholarClient] = None,
        llm_client: Optional[LLMClient] = None,
        novelty_threshold: float = 6.0
    ):
        self.ss_client = semantic_scholar or SemanticScholarClient()
        self.llm = llm_client or get_llm_client()
        self.novelty_threshold = novelty_threshold
    
    async def check_novelty(
        self,
        hypothesis: str,
        max_papers: int = 20
    ) -> NoveltyResult:
        """
        가설의 신규성 검증
        
        Args:
            hypothesis: 검증할 가설 문자열
            max_papers: 비교할 최대 논문 수
        
        Returns:
            NoveltyResult
        """
        # Search for similar papers
        similar_papers = await self.ss_client.search(
            query=hypothesis,
            max_results=max_papers
        )
        
        if not similar_papers:
            # No similar papers found - likely novel
            return NoveltyResult(
                hypothesis=hypothesis,
                is_novel=True,
                novelty_score=9.0,
                similar_papers=[],
                most_similar_paper=None,
                differentiation_points=["No closely related work found"],
                overlap_points=[],
                recommendation="accept",
                feedback="The hypothesis appears to be highly novel. No closely related work was found in the literature."
            )
        
        # Format papers for LLM evaluation
        papers_text = ""
        paper_dicts = []
        for i, paper in enumerate(similar_papers[:10], 1):
            papers_text += f"\n{i}. {paper.title}"
            if paper.abstract:
                papers_text += f"\n   Abstract: {paper.abstract[:300]}..."
            papers_text += f"\n   Citations: {paper.citation_count}\n"
            
            paper_dicts.append({
                "title": paper.title,
                "abstract": paper.abstract[:500] if paper.abstract else "",
                "citations": paper.citation_count,
                "year": paper.published_date.year if paper.published_date else None
            })
        
        # LLM evaluation
        prompt = self.EVALUATION_PROMPT.format(
            hypothesis=hypothesis,
            papers=papers_text
        )
        
        try:
            response = self.llm.generate_json(
                prompt=prompt,
                system_prompt="You are a research novelty evaluator with expertise in identifying original contributions.",
                temperature=0.3
            )
            
            novelty_score = float(response.get("novelty_score", 5.0))
            
            return NoveltyResult(
                hypothesis=hypothesis,
                is_novel=novelty_score >= self.novelty_threshold,
                novelty_score=novelty_score,
                similar_papers=paper_dicts,
                most_similar_paper=paper_dicts[0] if paper_dicts else None,
                differentiation_points=response.get("differentiation_points", []),
                overlap_points=response.get("overlap_points", []),
                recommendation=response.get("recommendation", "modify"),
                feedback=response.get("feedback", "")
            )
        
        except Exception as e:
            # Fallback evaluation based on number of similar papers
            novelty_score = max(3.0, 10.0 - len(similar_papers) * 0.3)
            
            return NoveltyResult(
                hypothesis=hypothesis,
                is_novel=novelty_score >= self.novelty_threshold,
                novelty_score=novelty_score,
                similar_papers=paper_dicts,
                most_similar_paper=paper_dicts[0] if paper_dicts else None,
                differentiation_points=[],
                overlap_points=[],
                recommendation="modify",
                feedback=f"Found {len(similar_papers)} similar papers. Manual review recommended. Error: {str(e)}"
            )
    
    def check_novelty_sync(
        self,
        hypothesis: str,
        max_papers: int = 20
    ) -> NoveltyResult:
        """동기 버전의 신규성 검증"""
        return asyncio.run(self.check_novelty(hypothesis, max_papers))
    
    async def batch_check(
        self,
        hypotheses: List[str],
        max_papers_per: int = 10
    ) -> List[NoveltyResult]:
        """여러 가설 일괄 검증"""
        results = []
        for hypothesis in hypotheses:
            result = await self.check_novelty(hypothesis, max_papers_per)
            results.append(result)
            await asyncio.sleep(1)  # Rate limiting
        return results
    
    def filter_novel(
        self,
        hypotheses: List[str],
        threshold: float = None
    ) -> List[str]:
        """
        신규성이 높은 가설만 필터링 (Rejection Sampling)
        
        Args:
            hypotheses: 가설 리스트
            threshold: 신규성 임계값 (None이면 기본값 사용)
        
        Returns:
            신규성이 높은 가설 리스트
        """
        threshold = threshold or self.novelty_threshold
        
        novel_hypotheses = []
        for hypothesis in hypotheses:
            result = self.check_novelty_sync(hypothesis)
            if result.novelty_score >= threshold:
                novel_hypotheses.append(hypothesis)
        
        return novel_hypotheses
