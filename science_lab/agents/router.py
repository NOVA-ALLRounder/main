"""
Router Agent - 제로샷 의도 분류
사용자 입력이 '가설 검증'인지 '단순 질문'인지 판단
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


ROUTER_SYSTEM_PROMPT = """당신은 과학적 입력 분류 전문가입니다.

다음 사용자의 입력과 도메인을 분석하여, 이것이:
1. 'hypothesis' - 검증이 필요한 새로운 과학적 가설
2. 'question' - 기존 지식에 기반한 사실 확인 또는 실현 가능성 질문

인지 분류하세요.

가설의 특징:
- "~할 것이다", "~라고 생각한다" 등의 추측/예측 표현
- 인과관계나 메커니즘에 대한 주장
- 새로운 이론이나 현상에 대한 제안

질문의 특징:
- "~인가?", "~할 수 있는가?" 등의 의문형
- 기존 사실에 대한 확인
- 실현 가능성이나 현재 상태에 대한 문의

JSON 형식으로 다음을 반환하세요:
{
    "intent": "hypothesis" 또는 "question",
    "confidence": 0.0~1.0 사이의 확신도,
    "reasoning": "분류 이유 설명",
    "key_concepts": ["핵심 개념 1", "핵심 개념 2"],
    "suggested_domain": "추천 도메인 (입력이 없는 경우)"
}
"""


class RouterAgent:
    """라우터 에이전트 - 의도 분류"""
    
    def __init__(self, api_key: str = None):
        self.api_key = api_key or os.getenv("OPENAI_API_KEY", "")
        self.llm = None
        if LANGCHAIN_AVAILABLE and self.api_key:
            self.llm = ChatOpenAI(
                model="gpt-4o-mini",
                temperature=0.3,
                api_key=self.api_key
            )
    
    def classify(self, state: ScientificState) -> ScientificState:
        """입력 의도 분류"""
        user_input = state.get('user_input', '')
        domain = state.get('domain', '')
        
        if self.llm:
            result = self._classify_with_llm(user_input, domain)
        else:
            result = self._classify_mock(user_input, domain)
        
        state['intent'] = result.get('intent', 'question')
        state['intent_confidence'] = result.get('confidence', 0.5)
        state['intent_reasoning'] = result.get('reasoning', '')
        state['current_step'] = 'intent_classified'
        
        # 도메인 자동 설정
        if not domain and result.get('suggested_domain'):
            state['domain'] = result['suggested_domain']
        
        return state
    
    def _classify_with_llm(self, user_input: str, domain: str) -> Dict[str, Any]:
        """LLM을 사용한 분류"""
        prompt = f"도메인: {domain or '미지정'}\n\n사용자 입력:\n{user_input}"
        
        messages = [
            SystemMessage(content=ROUTER_SYSTEM_PROMPT),
            HumanMessage(content=prompt)
        ]
        
        response = self.llm.invoke(messages)
        
        try:
            # JSON 파싱
            content = response.content
            if '```json' in content:
                content = content.split('```json')[1].split('```')[0]
            elif '```' in content:
                content = content.split('```')[1].split('```')[0]
            return json.loads(content.strip())
        except (json.JSONDecodeError, IndexError):
            return {
                "intent": "question",
                "confidence": 0.5,
                "reasoning": "JSON 파싱 실패, 기본값 사용"
            }
    
    def _classify_mock(self, user_input: str, domain: str) -> Dict[str, Any]:
        """Mock 분류 (API 키 없는 경우)"""
        # 가설 패턴 검사
        hypothesis_patterns = [
            '것이다', '할 것', '라고 생각', '예상', '가설',
            '하면', '일으킬', '영향을 미', '증가시킬', '감소시킬'
        ]
        
        question_patterns = [
            '?', '인가', '되는가', '할 수 있', '가능한가',
            '무엇', '어떻게', '왜'
        ]
        
        hypothesis_score = sum(1 for p in hypothesis_patterns if p in user_input)
        question_score = sum(1 for p in question_patterns if p in user_input)
        
        if hypothesis_score > question_score:
            return {
                "intent": "hypothesis",
                "confidence": 0.7,
                "reasoning": f"가설 패턴 {hypothesis_score}개 발견",
                "key_concepts": [],
                "suggested_domain": "과학"
            }
        else:
            return {
                "intent": "question",
                "confidence": 0.7,
                "reasoning": f"질문 패턴 {question_score}개 발견",
                "key_concepts": [],
                "suggested_domain": "과학"
            }


def router_node(state: ScientificState) -> ScientificState:
    """LangGraph 노드 함수"""
    agent = RouterAgent()
    return agent.classify(state)
