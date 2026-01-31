# =============================================================================
# T_lab Agent - PI (Principal Investigator)
# Novelty evaluation + Method proposal
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
logger = get_logger("agents.pi")


NOVELTY_PROMPT = """당신은 연구 가설의 독창성을 평가하는 책임연구원(PI)입니다.

제공된 가설과 기존 문헌을 바탕으로 다음을 평가하세요:
1. 기존 연구와 비교했을 때 이 가설이 얼마나 독창적인가?
2. 독창성 점수 (0.0 - 1.0).

반드시 한국어로 다음 JSON 형식을 반환하세요:
{
    "novelty_score": 0.0-1.0,
    "reasoning": "독창성 평가에 대한 설명",
    "similar_works": ["유사한 기존 연구 목록"],
    "unique_aspects": ["이 가설만의 고유한 차별점"]
}
"""


METHODS_PROMPT = """당신은 연구 방법론을 제안하는 책임연구원(PI)입니다.

가설과 문헌 맥락을 바탕으로 3가지 다른 실험 접근 방식을 제안하세요:
1. Analytical/Theoretical (이론적 분석) - 수학적 또는 논리적 분석
2. Simulation/Computational (시뮬레이션) - 컴퓨터 시뮬레이션 또는 몬테카를로 방법
3. Data-Driven (데이터 기반) - 기존 데이터셋 또는 메타 분석 활용

4.3 Data-Driven (데이터 기반) - 기존 데이터셋 또는 메타 분석 활용
4. 각 방법론마다 통계적 가설을 명확히 설정하세요:
   - $H_0$ (귀무가설): 변수 간 효과가 없음
   - $H_1$ (대립가설): 변수 간 유의미한 효과가 있음

반드시 한국어로 다음 JSON 배열(총 3개)을 반환하세요:
[
    {
    {
        "title": "방법론 명칭",
        "type": "analytical|simulation|data_driven",
        "description": "간략한 설명",
        "methodology": "상세 방법론",
        "hypothesis": {
            "h0": "귀무가설",
            "h1": "대립가설"
        },
        "expected_outcome": "예상되는 결과",
        "pros": ["장점 1", "장점 2"],
        "cons": ["단점 1"]
    }
]
"""


class PIAgent:
    """Principal Investigator agent for novelty and methods."""
    
    def __init__(self):
        self.llm = None
        if LANGCHAIN_AVAILABLE and settings.openai_api_key:
            self.llm = ChatOpenAI(
                model=settings.default_model,
                temperature=0.5,
                api_key=settings.openai_api_key
            )
    
    def evaluate_novelty(self, state: ScientificState) -> ScientificState:
        """Evaluate hypothesis novelty."""
        hypothesis = state.get('user_input', '')
        literature = state.get('literature_context', [])
        
        logger.info("Evaluating novelty", source="pi")
        
        if self.llm:
            result = self._evaluate_with_llm(hypothesis, literature)
        else:
            result = self._evaluate_mock(hypothesis, literature)
        
        state['novelty_score'] = result.get('novelty_score', 0.5)
        state['novelty_reasoning'] = result.get('reasoning', '')
        state['current_step'] = 'novelty_evaluated'
        
        # Add to logic chain
        state['logic_chain'] = state.get('logic_chain', [])
        state['logic_chain'].append({
            "step": "pi_novelty",
            "novelty_score": state['novelty_score'],
            "reasoning": state['novelty_reasoning'][:100]
        })
        
        return state
    
    def propose_methods(self, state: ScientificState) -> ScientificState:
        """Propose research methodologies."""
        hypothesis = state.get('user_input', '')
        literature = state.get('literature_context', [])
        
        logger.info("Proposing methods", source="pi")
        
        if self.llm:
            methods = self._propose_with_llm(hypothesis, literature)
        else:
            methods = self._propose_mock(hypothesis)
        
        state['proposed_methods'] = methods
        state['current_step'] = 'methods_proposed'
        
        # Add to logic chain
        state['logic_chain'] = state.get('logic_chain', [])
        state['logic_chain'].append({
            "step": "pi_methods",
            "methods_count": len(methods),
            "method_types": [m.get('type', 'unknown') for m in methods]
        })
        
        return state
    
    def _evaluate_with_llm(self, hypothesis: str, literature: List[Dict]) -> Dict[str, Any]:
        """LLM-based novelty evaluation."""
        lit_text = "\n".join([f"- {p.get('title', '')}" for p in literature[:5]])
        prompt = f"Hypothesis: {hypothesis}\n\nExisting Literature:\n{lit_text}"
        
        messages = [
            SystemMessage(content=NOVELTY_PROMPT),
            HumanMessage(content=prompt)
        ]
        
        response = self.llm.invoke(messages)
        
        try:
            content = response.content
            if '```json' in content:
                content = content.split('```json')[1].split('```')[0]
            return json.loads(content.strip())
        except:
            return {"novelty_score": 0.5, "reasoning": "Evaluation completed"}
    
    def _propose_with_llm(self, hypothesis: str, literature: List[Dict]) -> List[Dict[str, Any]]:
        """LLM-based method proposal."""
        lit_text = "\n".join([f"- {p.get('title', '')}: {p.get('abstract', '')[:100]}" for p in literature[:3]])
        prompt = f"Hypothesis: {hypothesis}\n\nLiterature:\n{lit_text}"
        
        messages = [
            SystemMessage(content=METHODS_PROMPT),
            HumanMessage(content=prompt)
        ]
        
        response = self.llm.invoke(messages)
        
        try:
            content = response.content
            if '```json' in content:
                content = content.split('```json')[1].split('```')[0]
            return json.loads(content.strip())
        except:
            return self._propose_mock(hypothesis)
    
    def _evaluate_mock(self, hypothesis: str, literature: List[Dict]) -> Dict[str, Any]:
        """Mock novelty evaluation."""
        # Simple heuristic: fewer related papers = more novel
        score = max(0.3, 1.0 - (len(literature) * 0.1))
        return {
            "novelty_score": score,
            "reasoning": f"Based on {len(literature)} related papers found",
            "similar_works": [p.get('title', '') for p in literature[:3]],
            "unique_aspects": ["Novel approach needed"]
        }
    
    def _propose_mock(self, hypothesis: str) -> List[Dict[str, Any]]:
        """Mock method proposal."""
        return [
            {
                "title": "Theoretical Analysis",
                "type": "analytical",
                "description": "Mathematical modeling and theoretical derivation",
                "methodology": "Develop mathematical framework to analyze the hypothesis",
                "expected_outcome": "Theoretical predictions and constraints",
                "pros": ["Rigorous", "No data required"],
                "cons": ["May lack real-world validation"]
            },
            {
                "title": "Monte Carlo Simulation",
                "type": "simulation",
                "description": "Statistical simulation with randomized parameters",
                "methodology": "Run 1000+ iterations with parameter sampling",
                "expected_outcome": "Probability distributions and p-values",
                "pros": ["Handles uncertainty", "Reproducible"],
                "cons": ["Requires parameter assumptions"]
            },
            {
                "title": "Literature Meta-Analysis",
                "type": "data_driven",
                "description": "Aggregate data from existing studies",
                "methodology": "Extract and synthesize data from relevant papers",
                "expected_outcome": "Aggregated effect sizes and confidence intervals",
                "pros": ["Uses real data", "High external validity"],
                "cons": ["Limited to available data"]
            }
        ]


def evaluate_novelty(state: ScientificState) -> ScientificState:
    """Entry point for novelty evaluation."""
    agent = PIAgent()
    return agent.evaluate_novelty(state)


def propose_methods(state: ScientificState) -> ScientificState:
    """Entry point for method proposal."""
    agent = PIAgent()
    return agent.propose_methods(state)
