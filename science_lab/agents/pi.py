"""
PI Agent (Principal Investigator) - 연구 책임자 에이전트
독창성 평가 및 3가지 방법론 제안
"""
from typing import Dict, Any, List
import json
import os

try:
    from langchain_openai import ChatOpenAI
    from langchain_core.messages import SystemMessage, HumanMessage
    LANGCHAIN_AVAILABLE = True
except ImportError:
    LANGCHAIN_AVAILABLE = False

from state import ScientificState, MethodologyType
from config import NOVELTY_THRESHOLD


PI_NOVELTY_PROMPT = """당신은 과학 연구의 독창성을 평가하는 전문가입니다.

사용자의 가설과 기존 문헌을 비교하여 독창성을 평가하세요.

기존 문헌 요약:
{literature_summary}

사용자 가설:
{hypothesis}

다음 JSON을 반환하세요:
{{
    "novelty_score": 0.0~1.0 (1.0이 가장 독창적),
    "analysis": "독창성 분석 설명",
    "similar_research": ["유사한 기존 연구 제목들"],
    "novel_aspects": ["독창적인 측면들"],
    "is_novel": true/false
}}
"""


PI_METHODOLOGY_PROMPT = """당신은 과학 실험 설계 전문가입니다.

다음 독창적 가설을 검증하기 위한 3가지 서로 다른 접근 방식을 제안하세요.

가설: {hypothesis}
도메인: {domain}
관련 문헌: {literature_summary}

3가지 방법론:
1. **이론/분석적 접근 (Analytical)**: 기존 데이터나 수학적 모델 분석
2. **시뮬레이션 접근 (Simulation)**: 컴퓨터 시뮬레이션/수치 해석
3. **데이터 기반 접근 (Data-Driven)**: 머신러닝/통계 분석

각 방법론에 대해 JSON 배열로 반환하세요:
[
    {{
        "type": "analytical",
        "title": "방법론 제목",
        "description": "상세 설명",
        "steps": ["단계1", "단계2"],
        "advantages": ["장점1", "장점2"],
        "disadvantages": ["단점1", "단점2"],
        "estimated_time": "예상 소요 시간",
        "required_libraries": ["numpy", "scipy"]
    }},
    ...
]
"""


PI_FEASIBILITY_PROMPT = """당신은 기술 실현 가능성 분석 전문가입니다.

사용자의 질문과 관련 문헌을 바탕으로 실현 가능성을 평가하세요.

질문: {question}
도메인: {domain}
관련 문헌: {literature_summary}

다음 JSON을 반환하세요:
{{
    "feasibility_grade": "high" | "medium" | "low" | "uncertain",
    "analysis": "상세 분석",
    "current_status": "현재 기술/연구 현황",
    "challenges": ["주요 과제들"],
    "timeline_estimate": "실현 예상 시기",
    "key_references": ["핵심 참고 문헌"]
}}
"""


class PIAgent:
    """연구 책임자 에이전트"""
    
    def __init__(self, api_key: str = None):
        self.api_key = api_key or os.getenv("OPENAI_API_KEY", "")
        self.llm = None
        if LANGCHAIN_AVAILABLE and self.api_key:
            self.llm = ChatOpenAI(
                model="gpt-4o-mini",
                temperature=0.5,
                api_key=self.api_key
            )
    
    def evaluate_novelty(self, state: ScientificState) -> ScientificState:
        """가설 독창성 평가"""
        hypothesis = state.get('user_input', '')
        literature = state.get('literature_context', [])
        
        literature_summary = self._summarize_literature(literature)
        
        if self.llm:
            result = self._evaluate_novelty_llm(hypothesis, literature_summary)
        else:
            result = self._evaluate_novelty_mock(hypothesis, literature)
        
        state['novelty_score'] = result.get('novelty_score', 0.5)
        state['novelty_analysis'] = result.get('analysis', '')
        state['existing_research'] = result.get('similar_research', [])
        state['current_step'] = 'novelty_evaluated'
        
        return state
    
    def propose_methods(self, state: ScientificState) -> ScientificState:
        """3가지 방법론 제안"""
        hypothesis = state.get('user_input', '')
        domain = state.get('domain', '')
        literature = state.get('literature_context', [])
        
        literature_summary = self._summarize_literature(literature)
        
        if self.llm:
            methods = self._propose_methods_llm(hypothesis, domain, literature_summary)
        else:
            methods = self._propose_methods_mock(hypothesis, domain)
        
        state['proposed_methods'] = methods
        state['status'] = 'waiting_input'
        state['current_step'] = 'methods_proposed'
        
        return state
    
    def evaluate_feasibility(self, state: ScientificState) -> ScientificState:
        """질문에 대한 실현 가능성 평가"""
        question = state.get('user_input', '')
        domain = state.get('domain', '')
        literature = state.get('literature_context', [])
        
        literature_summary = self._summarize_literature(literature)
        
        if self.llm:
            result = self._evaluate_feasibility_llm(question, domain, literature_summary)
        else:
            result = self._evaluate_feasibility_mock(question)
        
        state['feasibility_report'] = result.get('analysis', '')
        state['feasibility_grade'] = result.get('feasibility_grade', 'uncertain')
        state['current_step'] = 'feasibility_evaluated'
        
        return state
    
    def _summarize_literature(self, literature: List[Dict]) -> str:
        """문헌 목록 요약"""
        if not literature:
            return "관련 문헌을 찾지 못했습니다."
        
        summaries = []
        for i, item in enumerate(literature[:5], 1):
            title = item.get('title', 'Unknown')
            abstract = item.get('abstract', '')[:200]
            summaries.append(f"{i}. {title}\n   {abstract}...")
        
        return "\n\n".join(summaries)
    
    def _evaluate_novelty_llm(self, hypothesis: str, literature_summary: str) -> Dict:
        """LLM 독창성 평가"""
        prompt = PI_NOVELTY_PROMPT.format(
            hypothesis=hypothesis,
            literature_summary=literature_summary
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
        return {"novelty_score": 0.5, "analysis": response.content}
    
    def _evaluate_novelty_mock(self, hypothesis: str, literature: List) -> Dict:
        """Mock 독창성 평가"""
        # 문헌이 적으면 독창적일 가능성 높음
        score = 1.0 - (len(literature) * 0.1)
        score = max(0.3, min(1.0, score))
        
        return {
            "novelty_score": score,
            "analysis": f"관련 문헌 {len(literature)}개 발견. 독창성 점수: {score:.2f}",
            "similar_research": [l.get('title', '') for l in literature[:3]],
            "is_novel": score >= NOVELTY_THRESHOLD
        }
    
    def _propose_methods_llm(self, hypothesis: str, domain: str, literature_summary: str) -> List[Dict]:
        """LLM 방법론 제안"""
        prompt = PI_METHODOLOGY_PROMPT.format(
            hypothesis=hypothesis,
            domain=domain or "과학",
            literature_summary=literature_summary
        )
        response = self.llm.invoke([HumanMessage(content=prompt)])
        
        try:
            content = response.content
            if '[' in content:
                start = content.index('[')
                end = content.rindex(']') + 1
                return json.loads(content[start:end])
        except:
            pass
        return self._propose_methods_mock(hypothesis, domain)
    
    def _propose_methods_mock(self, hypothesis: str, domain: str) -> List[Dict]:
        """Mock 방법론 제안"""
        return [
            {
                "type": "analytical",
                "title": "메타 분석 및 통계적 검증",
                "description": "기존 데이터셋과 문헌을 활용한 메타 분석 수행",
                "steps": ["문헌 데이터 수집", "통계 분석", "결론 도출"],
                "advantages": ["비용 효율적", "빠른 결과"],
                "disadvantages": ["기존 데이터에 의존"],
                "estimated_time": "1-2시간",
                "required_libraries": ["pandas", "scipy", "statsmodels"]
            },
            {
                "type": "simulation",
                "title": "수치 시뮬레이션",
                "description": "수학적 모델을 기반으로 시뮬레이션 수행",
                "steps": ["모델 정의", "파라미터 설정", "시뮬레이션 실행", "결과 분석"],
                "advantages": ["메커니즘 검증 가능", "새로운 데이터 생성"],
                "disadvantages": ["모델 정확도에 의존"],
                "estimated_time": "2-4시간",
                "required_libraries": ["numpy", "scipy", "matplotlib"]
            },
            {
                "type": "data_driven",
                "title": "머신러닝 기반 예측",
                "description": "합성 데이터 생성 후 ML 모델 학습 및 예측",
                "steps": ["데이터 생성", "모델 학습", "예측 수행", "중요도 분석"],
                "advantages": ["복잡한 패턴 발견", "예측력 확보"],
                "disadvantages": ["블랙박스", "데이터 품질 의존"],
                "estimated_time": "3-5시간",
                "required_libraries": ["scikit-learn", "pandas", "numpy"]
            }
        ]
    
    def _evaluate_feasibility_llm(self, question: str, domain: str, literature_summary: str) -> Dict:
        """LLM 실현 가능성 평가"""
        prompt = PI_FEASIBILITY_PROMPT.format(
            question=question,
            domain=domain or "과학",
            literature_summary=literature_summary
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
        return {"feasibility_grade": "uncertain", "analysis": response.content}
    
    def _evaluate_feasibility_mock(self, question: str) -> Dict:
        """Mock 실현 가능성 평가"""
        return {
            "feasibility_grade": "medium",
            "analysis": "현재 기술 수준과 연구 동향을 고려할 때 중간 수준의 실현 가능성이 있습니다.",
            "current_status": "관련 연구가 활발히 진행 중입니다.",
            "challenges": ["기술적 한계", "비용 문제"],
            "timeline_estimate": "5-10년"
        }


def pi_novelty_node(state: ScientificState) -> ScientificState:
    """독창성 평가 노드"""
    agent = PIAgent()
    return agent.evaluate_novelty(state)


def pi_methods_node(state: ScientificState) -> ScientificState:
    """방법론 제안 노드"""
    agent = PIAgent()
    return agent.propose_methods(state)


def pi_feasibility_node(state: ScientificState) -> ScientificState:
    """실현 가능성 평가 노드"""
    agent = PIAgent()
    return agent.evaluate_feasibility(state)
