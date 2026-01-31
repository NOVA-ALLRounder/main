# =============================================================================
# T_lab Agent - Critic (Quality Review)
# Reviews and suggests improvements for reports
# =============================================================================

from typing import Dict, Any, List
import json

try:
    from langchain_openai import ChatOpenAI
    from langchain_core.messages import SystemMessage, HumanMessage
    LANGCHAIN_AVAILABLE = True
except ImportError:
    LANGCHAIN_AVAILABLE = False

from state import ScientificState
from core import get_settings, get_logger

settings = get_settings()
logger = get_logger("agents.critic")


CRITIC_PROMPT = """당신은 세계 최고 학술지의 까다로운 리뷰어(Peer Reviewer)입니다.

PI가 제안한 연구 설계(Methodology)를 비판적으로 검토해야 합니다.
다음 항목을 철저히 공격하십시오:
1. 표본 편향 (Sampling Bias) 가능성
2. 교란 변수 (Confounding Variables) 통제 여부
3. 통계적 검정력 (Statistical Power) 부족 여부
4. 가설($H_0, H_1$)의 논리적 타당성

당신의 승인(Approve)이 있어야만 실험을 진행할 수 있습니다.
미흡하다면 'Refine'을 요청하고 구체적인 수정 사항을 제시하십시오.

반드시 한국어로 다음 JSON 형식을 반환하세요:
{
    "quality_score": 0.0-1.0,
    "critique": "종합 비평",
    "issues": ["발견된 문제점 1", "문제점 2"],
    "required_actions": ["수정 요구사항 1", "수정 요구사항 2"],
    "decision": "approve|refine"
}
"""


class CriticAgent:
    """Critic agent for report quality review."""
    
    def __init__(self):
        self.llm = None
        if LANGCHAIN_AVAILABLE and settings.openai_api_key:
            self.llm = ChatOpenAI(
                model=settings.default_model,
                temperature=0.3,
                api_key=settings.openai_api_key
            )
    
    def review(self, state: ScientificState) -> ScientificState:
        """Review the research report."""
        report = state.get('draft_report', '')
        
        logger.info("Reviewing report", source="critic")
        
        if self.llm:
            review = self._review_with_llm(report)
        else:
            review = self._review_mock(report)
        
        state['critique'] = json.dumps(review, ensure_ascii=False)
        state['improvements'] = review.get('suggestions', [])
        state['quality_score'] = review.get('quality_score', 0.7)
        state['current_step'] = 'report_reviewed'
        
        # Add to logic chain
        state['logic_chain'] = state.get('logic_chain', [])
        state['logic_chain'].append({
            "step": "critic",
            "quality_score": state['quality_score'],
            "recommendation": review.get('recommendation', 'unknown')
        })
        
        return state
    
    def _review_with_llm(self, report: str) -> Dict[str, Any]:
        """Review report using LLM."""
        messages = [
            SystemMessage(content=CRITIC_PROMPT),
            HumanMessage(content=f"Please review this research report:\n\n{report[:5000]}")
        ]
        
        try:
            response = self.llm.invoke(messages)
            content = response.content
            if '```json' in content:
                content = content.split('```json')[1].split('```')[0]
            return json.loads(content.strip())
        except:
            return self._review_mock(report)
    
    def _review_mock(self, report: str) -> Dict[str, Any]:
        """Mock review."""
        # Simple heuristics
        has_stats = 'p-value' in report.lower() or 'p_value' in report.lower()
        has_refs = 'references' in report.lower() or 'doi' in report.lower()
        has_methods = 'method' in report.lower()
        
        score = 0.5
        if has_stats: score += 0.15
        if has_refs: score += 0.15
        if has_methods: score += 0.1
        if len(report) > 1000: score += 0.1
        
        return {
            "quality_score": min(score, 1.0),
            "strengths": [
                "Clear hypothesis statement" if len(report) > 500 else "Concise",
                "Statistical analysis included" if has_stats else "Structured format"
            ],
            "weaknesses": [
                "Could expand methodology section" if not has_methods else "Minor formatting issues",
                "Additional references recommended" if not has_refs else "Could add more discussion"
            ],
            "suggestions": [
                "Add more context in introduction",
                "Consider additional statistical tests",
                "Expand limitations section"
            ],
            "critical_issues": [],
            "recommendation": "minor_revision" if score > 0.6 else "major_revision"
        }


def review_report(state: ScientificState) -> ScientificState:
    """Entry point for critic agent."""
    agent = CriticAgent()
    return agent.review(state)
