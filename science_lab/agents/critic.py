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
from config import OPENAI_API_KEY


CRITIC_PROMPT = """당신은 엄격한 학술 논문 심사자(Reviewer 2)입니다.

다음 연구 결과와 보고서 초안을 비판적으로 검토하세요:

## 입력 정보
- 원래 가설: {hypothesis}
- 도메인: {domain}
- 실험 방법: {methodology}
- 실험 결과 (JSON): {results}
- 실험 결과 데이터: {experiment_data}
- 보고서 초안: {draft}

## 검토 기준 (반드시 모두 확인)

### 1. 데이터-결론 일관성 (CRITICAL)
- 보고서의 결론이 실험 결과 데이터와 정확히 일치하는가?
- p-value가 0.05 이상인데 "유의한 차이"를 주장하는가? → 심각한 오류
- p-value가 0.05 미만인데 "유의하지 않다"고 하는가? → 심각한 오류
- 수치 데이터가 결론과 모순되는가?

### 2. 기술적 정확성 (CRITICAL)
- 도메인 상식에 어긋나는 주장이 있는가?
  - 예: "Bluetooth가 WiFi보다 빠르다" (틀림)
  - 예: "절대영도 이하로 냉각" (불가능)
- 물리적/과학적으로 불가능한 결과를 주장하는가?

### 3. 표 형식 검증
- 결과 테이블에 실제 수치가 있는가?
- "|  |  |" 같은 빈 셀이 있는가?
- "N/A", "없음", "-" 같은 placeholder만 있는가?

### 4. 참고문헌 검증
- 불완전한 문장이 있는가? (예: "추가 연구를 통해...", "...등")
- 참고문헌 번호가 연속적인가?
- 잘린 텍스트가 있는가? (예: "-텍스트", "Sar...")

### 5. 논리적 일관성
- 가설 → 방법 → 결과 → 결론의 논리 흐름이 자연스러운가?
- 과장된 주장이 있는가?

## 반환 형식 (JSON)
{{
    "overall_score": 1-10,
    "strengths": ["강점 1", "강점 2"],
    "weaknesses": ["약점 1", "약점 2"],
    "major_issues": ["심각한 문제 - 반드시 수정 필요"],
    "minor_issues": ["사소한 문제"],
    "data_conclusion_match": true/false,
    "technical_accuracy": true/false,
    "table_quality": "good" | "incomplete" | "missing",
    "reference_quality": "good" | "issues" | "missing",
    "suggestions": ["구체적인 수정 제안"],
    "hallucination_detected": true/false,
    "recommendation": "accept" | "minor_revision" | "major_revision" | "reject"
}}
"""


class CriticAgent:
    """비평가 에이전트 - 동료 심사 시뮬레이션"""
    
    def __init__(self, api_key: str = None):
        self.api_key = api_key or OPENAI_API_KEY
        self.llm = None
        if LANGCHAIN_AVAILABLE and self.api_key:
            self.llm = ChatOpenAI(
                model="gpt-4o-mini",
                temperature=0.3,
                api_key=self.api_key
            )
        else:
            print(f"⚠️ [CriticAgent] API Key missing or LangChain not found. Running in MOCK mode.")
    
    def review(self, state: ScientificState) -> ScientificState:
        """보고서 검토"""
        hypothesis = state.get('user_input', '')
        domain = state.get('domain', '')
        methods = state.get('proposed_methods', [])
        selected_idx = state.get('selected_method_index', 0)
        logs = state.get('execution_logs', '')
        draft = state.get('draft_report', '')
        experiment_results = state.get('experiment_results', {})
        
        methodology = methods[selected_idx] if methods and selected_idx < len(methods) else {}
        
        if self.llm:
            feedback = self._review_llm(hypothesis, domain, methodology, logs, draft, experiment_results)
        else:
            feedback = self._review_mock(hypothesis, logs, draft, experiment_results)
        
        state['critic_feedback'] = json.dumps(feedback, ensure_ascii=False, indent=2)
        state['current_step'] = 'review_completed'
        
        return state
    
    def _review_llm(self, hypothesis: str, domain: str, methodology: Dict, 
                    results: str, draft: str, experiment_data: Dict) -> Dict:
        """LLM 리뷰"""
        prompt = CRITIC_PROMPT.format(
            hypothesis=hypothesis,
            domain=domain or "미지정",
            methodology=json.dumps(methodology, ensure_ascii=False) if methodology else "방법론 정보 없음",
            results=results[:2000] if results else "결과 정보 없음",
            experiment_data=json.dumps(experiment_data, ensure_ascii=False, indent=2) if experiment_data else "데이터 없음",
            draft=draft[:3000] if draft else "초안 정보 없음"
        )
        response = self.llm.invoke([HumanMessage(content=prompt)])
        
        content = response.content
        
        # 1. 마크다운 코드블록 제거
        content = content.replace("```json", "").replace("```", "").strip()
        
        # 2. JSON 파싱 시도
        try:
            return json.loads(content)
        except json.JSONDecodeError:
            pass
            
        # 3. 중괄호 찾기 시도
        try:
            if '{' in content:
                start = content.index('{')
                end = content.rindex('}') + 1
                return json.loads(content[start:end])
        except Exception as e:
            print(f"Critic JSON 파싱 실패: {e}")
            pass
        
        # 4. 실패 시 정형화된 에러 피드백 반환 (Mock 아님, 파싱 실패 알림)
        return {
            "overall_score": 5,
            "strengths": ["분석 시도되었으나 파싱 실패"],
            "weaknesses": ["시스템 내부 오류로 상세 리뷰 불가"],
            "suggestions": ["관리자에게 문의하세요"],
            "major_issues": ["비평 생성 실패"],
            "recommendation": "minor_revision"
        }
    
    def _review_mock(self, hypothesis: str, results: str, draft: str, experiment_data: Dict = None) -> Dict:
        """Mock 리뷰 (규칙 기반 검증 포함)"""
        import re
        
        has_results = bool(results)
        has_draft = bool(draft)
        issues = []
        
        # 기본 점수
        score = 5
        if has_results:
            score += 2
        if has_draft:
            score += 2
        if len(hypothesis) > 50:
            score += 1
        
        # 데이터-결론 일관성 검사
        data_conclusion_match = True
        if experiment_data:
            p_value = experiment_data.get('p_value')
            if p_value is not None and p_value >= 0.05:
                if draft and re.search(r'유의(미)?한?\s*(차이|효과)', draft):
                    data_conclusion_match = False
                    issues.append("p-value가 0.05 이상인데 '유의한 차이'를 주장함")
                    score -= 3
        
        # 표 형식 검사
        table_quality = "good"
        if draft:
            if re.search(r'\|\s*\|\s*\|', draft) or re.search(r'\|\s*(없음|N/A|-)\s*\|', draft):
                table_quality = "incomplete"
                issues.append("결과 테이블에 빈 셀 또는 placeholder 존재")
                score -= 1
        else:
            table_quality = "missing"
        
        # 참고문헌 검사
        reference_quality = "good"
        if draft and re.search(r'## 참고문헌', draft):
            if re.search(r'(추가\s*연구|\.\.\.|-텍스트)', draft):
                reference_quality = "issues"
                issues.append("참고문헌에 불완전한 텍스트 존재")
                score -= 1
        
        return {
            "overall_score": max(1, min(10, score)),
            "strengths": [
                "가설이 명확하게 정의됨",
                "실험 방법론이 체계적임"
            ],
            "weaknesses": [
                "샘플 크기가 제한적일 수 있음",
                "외부 검증 필요"
            ],
            "major_issues": issues if issues else [],
            "minor_issues": ["일부 용어 정의 보완 필요"] if not issues else [],
            "data_conclusion_match": data_conclusion_match,
            "technical_accuracy": True,  # Mock에서는 기본 True
            "table_quality": table_quality,
            "reference_quality": reference_quality,
            "suggestions": [
                "실제 데이터로 검증 권장",
                "신뢰구간 추가 보고 권장"
            ],
            "hallucination_detected": not data_conclusion_match,
            "recommendation": "accept" if score >= 8 else ("minor_revision" if score >= 6 else "major_revision")
        }


def critic_node(state: ScientificState) -> ScientificState:
    """LangGraph 노드 함수"""
    agent = CriticAgent()
    return agent.review(state)
