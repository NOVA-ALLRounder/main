"""
Critic Agent - 비평가 에이전트
결과 검토, 논리적 결함 및 환각 탐지
"""
from typing import Dict, Any
import json
import os

try:
    from langchain_openai import ChatOpenAI
    from langchain_core.messages import SystemMessage, HumanMessage
    LANGCHAIN_AVAILABLE = True
except ImportError:
    LANGCHAIN_AVAILABLE = False

from state import ScientificState


CRITIC_PROMPT = """당신은 엄격한 학술 논문 심사자(Reviewer 2)입니다.

다음 연구 결과와 보고서 초안을 비판적으로 검토하세요:

원래 가설: {hypothesis}
실험 방법: {methodology}
실험 결과: {results}
보고서 초안: {draft}

다음 관점에서 검토하세요:
1. 논리적 일관성: 가설, 방법, 결론이 일관되는가?
2. 과장된 주장: 데이터가 뒷받침하지 않는 주장이 있는가?
3. 환각(Hallucination): 실제 결과와 다른 내용이 있는가?
4. 방법론적 한계: 실험 설계의 문제점은?
5. 인용 적절성: 문헌 참조가 적절한가?

JSON 형식으로 반환하세요:
{{
    "overall_score": 1-10 (10이 최고),
    "strengths": ["강점 1", "강점 2"],
    "weaknesses": ["약점 1", "약점 2"],
    "major_issues": ["주요 문제 1"],
    "minor_issues": ["사소한 문제 1"],
    "suggestions": ["수정 제안 1", "제안 2"],
    "hallucination_detected": true/false,
    "recommendation": "accept" | "minor_revision" | "major_revision" | "reject"
}}
"""


class CriticAgent:
    """비평가 에이전트 - 동료 심사 시뮬레이션"""
    
    def __init__(self, api_key: str = None):
        self.api_key = api_key or os.getenv("OPENAI_API_KEY", "")
        self.llm = None
        if LANGCHAIN_AVAILABLE and self.api_key:
            self.llm = ChatOpenAI(
                model="gpt-4o-mini",
                temperature=0.3,
                api_key=self.api_key
            )
    
    def review(self, state: ScientificState) -> ScientificState:
        """보고서 검토"""
        hypothesis = state.get('user_input', '')
        methods = state.get('proposed_methods', [])
        selected_idx = state.get('selected_method_index', 0)
        logs = state.get('execution_logs', '')
        draft = state.get('draft_report', '')
        
        methodology = methods[selected_idx] if methods and selected_idx < len(methods) else {}
        
        if self.llm:
            feedback = self._review_llm(hypothesis, methodology, logs, draft)
        else:
            feedback = self._review_mock(hypothesis, logs, draft)
        
        state['critic_feedback'] = json.dumps(feedback, ensure_ascii=False, indent=2)
        state['current_step'] = 'review_completed'
        
        return state
    
    def _review_llm(self, hypothesis: str, methodology: Dict, results: str, draft: str) -> Dict:
        """LLM 리뷰"""
        prompt = CRITIC_PROMPT.format(
            hypothesis=hypothesis,
            methodology=json.dumps(methodology, ensure_ascii=False) if methodology else "방법론 정보 없음",
            results=results[:1000] if results else "결과 정보 없음",
            draft=draft[:2000] if draft else "초안 정보 없음"
        )
        response = self.llm.invoke([HumanMessage(content=prompt)])
        
        try:
            content = response.content
            if '{' in content:
                start = content.index('{')
                end = content.rindex('}') + 1
                return json.loads(content[start:end])
        except:
            pass
        
        return {
            "overall_score": 7,
            "strengths": ["체계적인 접근"],
            "weaknesses": ["추가 검증 필요"],
            "suggestions": ["더 많은 데이터로 재검증 권장"],
            "recommendation": "minor_revision"
        }
    
    def _review_mock(self, hypothesis: str, results: str, draft: str) -> Dict:
        """Mock 리뷰"""
        # 간단한 휴리스틱 평가
        has_results = bool(results)
        has_draft = bool(draft)
        
        score = 5
        if has_results:
            score += 2
        if has_draft:
            score += 2
        if len(hypothesis) > 50:
            score += 1
        
        return {
            "overall_score": min(10, score),
            "strengths": [
                "가설이 명확하게 정의됨",
                "실험 방법론이 체계적임"
            ],
            "weaknesses": [
                "샘플 크기가 제한적일 수 있음",
                "외부 검증 필요"
            ],
            "major_issues": [],
            "minor_issues": [
                "일부 용어 정의 보완 필요"
            ],
            "suggestions": [
                "실제 데이터로 검증 권장",
                "신뢰구간 추가 보고 권장"
            ],
            "hallucination_detected": False,
            "recommendation": "minor_revision" if score >= 7 else "major_revision"
        }


def critic_node(state: ScientificState) -> ScientificState:
    """LangGraph 노드 함수"""
    agent = CriticAgent()
    return agent.review(state)
