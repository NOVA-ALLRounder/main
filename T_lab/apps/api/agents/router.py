# =============================================================================
# T_lab Agent - Router (Intent Classification)
# Adapted from science_lab/agents/router.py
# =============================================================================

from typing import Dict, Any
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
logger = get_logger("agents.router")


ROUTER_SYSTEM_PROMPT = """당신은 과학적 입력 분류 전문가입니다.

사용자의 입력과 도메인을 분석하여 다음 중 하나로 결정하세요:
1. 'hypothesis' - 검증이 필요한 새로운 과학술적 가설
2. 'question' - 기존 지식에 기반한 사실 확인 또는 실현 가능성에 대한 질문

가설의 특징:
- "~할 것이다", "~라고 생각한다" 등의 예측성 표현
- 인과 관계나 메커니즘에 대한 주장
- 새로운 이론이나 현상에 대한 제안

질문의 특징:
- "~인가요?", "~할 수 있나요?", "~은 무엇인가요?" 등 의문형
- 기존 사실에 대한 확인 요청
- 현재 상태나 실현 가능성에 대한 단순 문의

## 시뮬레이션 적합성 판단 (가설인 경우)
가설이라면, 수학적 시뮬레이션이 가능한지 판단하세요:

**시뮬레이션 가능 (simulatable: true)**:
- 정량적 비교가 가능한 가설 (예: "A가 B보다 크다")
- 수학적 모델이 존재하는 분야 (예: 물리, 화학, 통신)
- 통계적 검증이 가능한 가설

**시뮬레이션 불가능 (simulatable: false)**:
- 질적 연구가 필요한 가설 (예: "사용자들이 X를 선호한다")
- 실제 데이터가 필수적인 가설 (예: "약물 A가 실제로 효과 있다")
- 탐색적 연구가 필요한 주제 (예: "왜 X 현상이 발생하는가?")

반드시 한국어로 다음 JSON 형식을 반환하세요:
{
    "intent": "hypothesis" 또는 "question",
    "confidence": 0.0-1.0,
    "reasoning": "분류 근거 설명",
    "key_concepts": ["개념1", "개념2"],
    "suggested_domain": "가장 적절한 과학/공학 도메인",
    "simulatable": true 또는 false,
    "simulation_reason": "시뮬레이션 가능/불가능 이유",
    "alternative_method": "시뮬레이션 불가능 시 대안적 연구 방법 (예: '문헌 리뷰', '설문 조사', '임상시험')"
}
"""


class RouterAgent:
    """Router Agent - Intent Classification"""
    
    def __init__(self):
        self.llm = None
        if LANGCHAIN_AVAILABLE and settings.openai_api_key:
            self.llm = ChatOpenAI(
                model="gpt-4o-mini",
                temperature=0.3,
                api_key=settings.openai_api_key
            )
    
    def classify(self, state: ScientificState) -> ScientificState:
        """Classify input intent."""
        user_input = state.get('user_input', '')
        domain = state.get('domain', '')
        
        logger.info("Classifying intent", input=user_input[:50], source="router")
        
        if self.llm:
            result = self._classify_with_llm(user_input, domain)
        else:
            result = self._classify_mock(user_input, domain)
            
        logger.info(f"Router Result: {result.get('suggested_domain')}", session_id=state.get("session_id"))
        
        state['intent'] = result.get('intent', 'question')
        state['intent_confidence'] = result.get('confidence', 0.5)
        state['current_step'] = 'intent_classified'
        
        # Simulatable check (for hypothesis)
        state['simulatable'] = result.get('simulatable', True)
        state['simulation_reason'] = result.get('simulation_reason', '')
        state['alternative_method'] = result.get('alternative_method')
        
        # Auto-set domain
        if not domain and result.get('suggested_domain'):
            state['domain'] = result['suggested_domain']
        
        # Add to logic chain
        state['logic_chain'] = state.get('logic_chain', [])
        state['logic_chain'].append({
            "step": "router",
            "intent": state['intent'],
            "confidence": state['intent_confidence'],
            "simulatable": state['simulatable']
        })
        
        return state
    
    def _classify_with_llm(self, user_input: str, domain: str) -> Dict[str, Any]:
        """LLM-based classification."""
        prompt = f"Domain: {domain or 'unspecified'}\n\nUser Input:\n{user_input}"
        
        messages = [
            SystemMessage(content=ROUTER_SYSTEM_PROMPT),
            HumanMessage(content=prompt)
        ]
        
        try:
            response = self.llm.invoke(messages)
            content = response.content
            # Clean markdown code blocks
            if '```json' in content:
                content = content.split('```json')[1].split('```')[0]
            elif '```' in content:
                content = content.split('```')[1].split('```')[0]
            return json.loads(content.strip())
        except Exception as e:
            logger.error(f"Router LLM Error: {e}")
            return self._classify_mock(user_input, domain)

    def _classify_mock(self, user_input: str, domain: str) -> Dict[str, Any]:
        """Mock classification (no API key)."""
        hypothesis_patterns = ['will', 'would', 'should', 'causes', 'increases', 'decreases', 'affects', 'hypothesis']
        question_patterns = ['?', 'can', 'is it', 'how', 'what', 'why', 'possible']
        
        h_score = sum(1 for p in hypothesis_patterns if p.lower() in user_input.lower())
        q_score = sum(1 for p in question_patterns if p.lower() in user_input.lower())
        
        # Simple domain inference
        suggested = "General Science"
        if any(w in user_input.lower() for w in ['communication', 'wifi', 'mimo', 'network', 'signal']):
            suggested = "Communication Engineering"
        elif any(w in user_input.lower() for w in ['bio', 'cell', 'gene', 'dna']):
            suggested = "Biology"
        elif any(w in user_input.lower() for w in ['physic', 'quantum', 'force', 'energy']):
            suggested = "Physics"
            
        result = {
            "suggested_domain": suggested,
            "confidence": 0.7
        }
        
        if h_score > q_score:
            result.update({"intent": "hypothesis", "reasoning": f"Hypothesis patterns: {h_score}"})
        else:
            result.update({"intent": "question", "reasoning": f"Question patterns: {q_score}"})
            
        return result


def classify_intent(state: ScientificState) -> ScientificState:
    """Entry point for router agent."""
    agent = RouterAgent()
    return agent.classify(state)
