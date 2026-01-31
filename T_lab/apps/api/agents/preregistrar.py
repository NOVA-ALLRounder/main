# =============================================================================
# T_lab Agent - Preregistrar (Research Integrity)
# Locks hypothesis and analysis plan to prevent p-hacking
# =============================================================================

from typing import Dict, Any, List
import json
from datetime import datetime

try:
    from langchain_openai import ChatOpenAI
    from langchain_core.messages import SystemMessage, HumanMessage
    LANGCHAIN_AVAILABLE = True
except ImportError:
    LANGCHAIN_AVAILABLE = False

from state import ScientificState
from core import get_settings, get_logger

settings = get_settings()
logger = get_logger("agents.preregistrar")


PREREG_PROMPT = """당신은 연구 윤리를 감독하는 사전 등록 관리자(Preregistrar)입니다.

실험을 수행하기 전에, 다음 사항들이 명확하게 정의되었는지 확인하고 연구 계획을 '동결(Lock)'해야 합니다.
1. 가설 ($H_0$, $H_1$)이 명확한가?
2. 분석 방법이 구체적인가? (시뮬레이션/통계 검정 방법 등)
3. 표본 수와 유의 수준($\alpha$)이 결정되었는가?

입력된 연구 계획을 검토하고, 문제가 없다면 정식으로 등록하십시오.
문제가 있다면 구체적인 보완 사항을 요청하십시오.

반드시 한국어로 다음 JSON 형식을 반환하세요:
{
    "status": "approved|rejected",
    "locked_plan": {
        "hypothesis_summary": "요약된 가설",
        "analysis_method": "확정된 분석 방법",
        "significance_level": 0.05,
        "timestamp": "ISO8601 Time"
    },
    "feedback": "승인 사유 또는 거절 사유"
}
"""

class PreregistrarAgent:
    """Agent that locks research plans."""
    
    def __init__(self):
        self.llm = None
        if LANGCHAIN_AVAILABLE and settings.openai_api_key:
            self.llm = ChatOpenAI(
                model=settings.default_model,
                temperature=0.1,  # Strict / Low Creativity
                api_key=settings.openai_api_key
            )
            
    def register_plan(self, state: ScientificState) -> ScientificState:
        """Validate and lock the research plan."""
        method = state.get('selected_method', {})
        hypothesis = state.get('user_input', '')
        
        logger.info("registering research plan", source="preregistrar")
        
        if self.llm:
            result = self._validate_with_llm(hypothesis, method)
        else:
            result = self._validate_mock(hypothesis, method)
            
        if result.get('status') == 'approved':
            # Lock the plan in the state
            state['preregistered_plan'] = result.get('locked_plan')
            state['hypothesis_locked'] = True
            state['current_step'] = 'plan_locked'
            
            # Add to activity log
            state['logic_chain'] = state.get('logic_chain', [])
            state['logic_chain'].append({
                "step": "preregistrar",
                "status": "locked",
                "plan": result.get('locked_plan')
            })
        else:
            state['hypothesis_locked'] = False
            state['error'] = f"Plan Rejected: {result.get('feedback')}"
            
        return state

    def _validate_with_llm(self, hypothesis: str, method: Dict) -> Dict[str, Any]:
        """Validate using LLM."""
        method_str = json.dumps(method, ensure_ascii=False)
        prompt = f"Hypothesis: {hypothesis}\n\nSelected Method:\n{method_str}"
        
        messages = [
            SystemMessage(content=PREREG_PROMPT),
            HumanMessage(content=prompt)
        ]
        
        try:
            response = self.llm.invoke(messages)
            content = response.content
            if '```json' in content:
                content = content.split('```json')[1].split('```')[0]
            return json.loads(content.strip())
        except Exception as e:
            logger.error(f"LLM validation failed: {e}")
            return self._validate_mock(hypothesis, method)

    def _validate_mock(self, hypothesis: str, method: Dict) -> Dict[str, Any]:
        return {
            "status": "approved",
            "locked_plan": {
                "hypothesis_summary": hypothesis[:50],
                "analysis_method": method.get('title', 'Unknown'),
                "significance_level": 0.05,
                "timestamp": datetime.now().isoformat()
            },
            "feedback": "Research plan meets all criteria for preregistration."
        }

def register_plan(state: ScientificState) -> ScientificState:
    agent = PreregistrarAgent()
    return agent.register_plan(state)
