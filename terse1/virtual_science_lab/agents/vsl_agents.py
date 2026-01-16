"""
Virtual Science Lab - Agent Base & Implementations
DAACS v2 CLI Provider를 활용한 에이전트 구현
"""

import json
import sys
from typing import Dict, Any, Optional
from pathlib import Path

from ..state import ScientificState, LiteratureItem, MethodologyProposal
from ..prompts import VSLPrompts
from ..tools.semantic_scholar import search_literature
from ..llm_adapter import get_llm_for_agent, LLMSource


class VSLAgents:
    """Virtual Science Lab 에이전트 구현"""
    
    @staticmethod
    def router_agent(state: ScientificState, use_llm: bool = False) -> Dict[str, Any]:
        """
        의도 분류 에이전트
        - hypothesis vs question 분류
        - 도메인 추출
        """
        print("--- [Router] Classifying intent ---")
        
        user_input = state["user_input"]
        
        if use_llm:
            # LLM 사용 모드
            llm = get_llm_for_agent("router")
            result = llm.invoke_structured(
                f"{VSLPrompts.ROUTER}\n\nUSER INPUT:\n{user_input}"
            )
            if "error" not in result:
                return {
                    "intent": result.get("intent", "hypothesis"),
                    "intent_confidence": result.get("confidence", 0.7),
                    "domain": result.get("domain", "general"),
                    "next_step": "librarian"
                }
        
        # Fallback: 휴리스틱 분류
        hypothesis_keywords = ["할 것이다", "증가시킬", "감소시킬", "영향을 미친다", 
                               "will", "affect", "increase", "decrease", "cause"]
        question_keywords = ["가능한가", "있는가", "무엇인가", "어떻게", 
                            "possible", "what is", "how", "why", "can"]
        
        is_hypothesis = any(kw in user_input.lower() for kw in hypothesis_keywords)
        is_question = any(kw in user_input.lower() for kw in question_keywords)
        
        if is_hypothesis and not is_question:
            intent = "hypothesis"
            confidence = 0.85
        elif is_question:
            intent = "question"
            confidence = 0.80
        else:
            intent = "hypothesis"
            confidence = 0.60
        
        # 도메인 추출
        domain_keywords = {
            "physics": ["물리", "양자", "상대성", "physics", "quantum"],
            "biology": ["생물", "세포", "유전", "biology", "cell", "gene"],
            "chemistry": ["화학", "분자", "반응", "chemistry", "molecule"],
            "cs": ["알고리즘", "컴퓨터", "AI", "algorithm", "computer", "machine learning"],
            "medicine": ["의학", "치료", "약물", "medicine", "drug", "treatment"],
            "data_science": ["데이터", "통계", "분석", "data", "statistics", "analysis"]
        }
        
        detected_domain = "general"
        for domain, keywords in domain_keywords.items():
            if any(kw in user_input.lower() for kw in keywords):
                detected_domain = domain
                break
        
        return {
            "intent": intent,
            "intent_confidence": confidence,
            "domain": detected_domain,
            "next_step": "librarian"
        }
    
    @staticmethod
    def librarian_agent(state: ScientificState) -> Dict[str, Any]:
        """
        문헌 검색 에이전트
        - Semantic Scholar API 호출
        - 관련 논문 수집
        """
        print("--- [Librarian] Searching literature ---")
        
        query = state["user_input"]
        domain = state.get("domain", "general")
        
        # Semantic Scholar 검색
        papers = search_literature(
            query=query,
            domain=domain,
            max_results=10,
            recent_years=5
        )
        
        # LiteratureItem 형식으로 변환
        literature_items = []
        for paper in papers:
            literature_items.append(LiteratureItem(
                paper_id=paper["paper_id"],
                title=paper["title"],
                abstract=paper["abstract"],
                authors=paper["authors"],
                year=paper["year"],
                citation_count=paper["citation_count"],
                source="semantic_scholar",
                url=paper.get("url")
            ))
        
        return {
            "literature_context": literature_items,
            "next_step": "pi"
        }
    
    @staticmethod
    def pi_agent(state: ScientificState) -> Dict[str, Any]:
        """
        Principal Investigator 에이전트
        - 독창성 평가
        - 방법론 설계 (3가지)
        """
        print("--- [PI] Evaluating novelty and designing methods ---")
        
        intent = state.get("intent", "hypothesis")
        literature = state.get("literature_context", [])
        
        # 질문인 경우: 실현 가능성 리포트 생성
        if intent == "question":
            feasibility_report = f"총 {len(literature)}개의 관련 논문을 분석했습니다.\n"
            if literature:
                feasibility_report += f"가장 관련성 높은 논문: {literature[0]['title']}\n"
                feasibility_report += "현재 연구 동향을 고려할 때, 추가 분석이 필요합니다."
            
            return {
                "feasibility_report": feasibility_report,
                "feasibility_rating": "medium",
                "next_step": "author"  # 질문은 바로 보고서 작성으로
            }
        
        # 가설인 경우: 독창성 평가
        novelty_score = 0.7 if len(literature) < 3 else 0.4  # 논문 적으면 더 독창적
        
        if novelty_score < 0.3:
            # 이미 검증된 가설
            return {
                "novelty_score": novelty_score,
                "existing_research_summary": f"이 가설은 이미 검증된 것으로 보입니다. 관련 연구: {literature[0]['title'] if literature else 'N/A'}",
                "next_step": "author"
            }
        
        # 독창적 가설: 3가지 방법론 제안
        methods = [
            MethodologyProposal(
                method_id=1,
                approach_type="analytical",
                title="메타 분석 접근",
                description="기존 연구 데이터를 수집하여 통계적 메타 분석을 수행합니다.",
                required_libraries=["pandas", "scipy", "statsmodels"],
                estimated_complexity="low",
                pros=["빠른 결과", "비용 효율적"],
                cons=["새로운 데이터 생성 불가"]
            ),
            MethodologyProposal(
                method_id=2,
                approach_type="simulation",
                title="수치 시뮬레이션",
                description="가설의 메커니즘을 수학적 모델로 구현하고 시뮬레이션합니다.",
                required_libraries=["numpy", "scipy", "matplotlib"],
                estimated_complexity="medium",
                pros=["메커니즘 검증 가능", "시각화 용이"],
                cons=["모델 정확도에 의존"]
            ),
            MethodologyProposal(
                method_id=3,
                approach_type="data_driven",
                title="머신러닝 패턴 분석",
                description="합성 데이터를 생성하고 ML 모델로 패턴을 탐색합니다.",
                required_libraries=["pandas", "sklearn", "matplotlib"],
                estimated_complexity="high",
                pros=["복잡한 패턴 발견", "확장성"],
                cons=["블랙박스 해석 어려움"]
            )
        ]
        
        return {
            "novelty_score": novelty_score,
            "proposed_methods": methods,
            "next_step": "human_selection"  # 사용자 선택 대기
        }
    
    @staticmethod
    def engineer_agent(state: ScientificState) -> Dict[str, Any]:
        """
        엔지니어 에이전트
        - 선택된 방법론을 코드로 구현
        - Docker 샌드박스에서 실행
        """
        print("--- [Engineer] Generating experiment code ---")
        
        selected_idx = state.get("selected_method")
        methods = state.get("proposed_methods", [])
        
        if selected_idx is None or not methods:
            return {"error_message": "No method selected", "next_step": "STOP"}
        
        method = methods[selected_idx]
        
        # 간단한 템플릿 코드 생성 (실제로는 LLM이 생성)
        code_template = f'''"""
Virtual Science Lab - Experiment Script
Method: {method["title"]}
Approach: {method["approach_type"]}
"""

import numpy as np
import matplotlib.pyplot as plt
from pathlib import Path

# 결과 저장 디렉토리
RESULTS_DIR = Path("results")
RESULTS_DIR.mkdir(exist_ok=True)

def run_experiment():
    """실험 실행"""
    print("Starting experiment: {method["title"]}")
    
    # 샘플 데이터 생성
    x = np.linspace(0, 10, 100)
    y = np.sin(x) + np.random.normal(0, 0.1, 100)
    
    # 분석 수행
    mean_y = np.mean(y)
    std_y = np.std(y)
    
    print(f"Mean: {{mean_y:.4f}}")
    print(f"Std: {{std_y:.4f}}")
    
    # 그래프 저장
    plt.figure(figsize=(10, 6))
    plt.plot(x, y, 'b-', label='Data')
    plt.xlabel('X')
    plt.ylabel('Y')
    plt.title('{method["title"]}')
    plt.legend()
    plt.savefig(RESULTS_DIR / 'plot.png', dpi=150)
    plt.close()
    
    print(f"Results saved to {{RESULTS_DIR}}")
    return {{"mean": mean_y, "std": std_y}}

if __name__ == "__main__":
    results = run_experiment()
    print("Experiment completed successfully!")
'''
        
        return {
            "code_repository": {"experiment.py": code_template},
            "next_step": "execute"
        }
    
    @staticmethod
    def critic_agent(state: ScientificState) -> Dict[str, Any]:
        """
        비평가 에이전트
        - 논리 검증
        - 환각 감지
        """
        print("--- [Critic] Reviewing results ---")
        
        experiment_result = state.get("experiment_result")
        
        if not experiment_result or not experiment_result.get("success"):
            return {
                "error_message": "Experiment did not complete successfully",
                "next_step": "author"  # 실패해도 보고서는 작성
            }
        
        # 간단한 검증 (실제로는 LLM이 상세 검토)
        verdict = "minor_revision"
        issues = [
            {"severity": "minor", "location": "Methods", "comment": "파라미터 설정 근거 추가 필요"}
        ]
        
        return {
            "next_step": "author"
        }
    
    @staticmethod
    def author_agent(state: ScientificState) -> Dict[str, Any]:
        """
        저자 에이전트
        - IMRAD 형식 보고서 작성
        """
        print("--- [Author] Writing report ---")
        
        user_input = state["user_input"]
        intent = state.get("intent", "hypothesis")
        literature = state.get("literature_context", [])
        
        # Markdown 보고서 생성
        report = f"""# 연구 보고서

## Abstract
본 보고서는 "{user_input[:50]}..."에 대한 분석 결과를 담고 있습니다.

## 1. Introduction
사용자의 {'가설' if intent == 'hypothesis' else '질문'}을 바탕으로 관련 문헌을 조사하고 분석을 수행했습니다.

## 2. Methods
- 문헌 검색: Semantic Scholar API (최근 5년)
- 검색된 논문 수: {len(literature)}개

## 3. Results
### 3.1 문헌 분석
"""
        
        for i, paper in enumerate(literature[:5], 1):
            report += f"- [{i}] {paper.get('title', 'Untitled')} ({paper.get('year', 'N/A')})\n"
        
        if state.get("novelty_score"):
            report += f"\n### 3.2 독창성 점수\n독창성: {state['novelty_score']:.2f}\n"
        
        if state.get("feasibility_report"):
            report += f"\n### 3.2 실현 가능성 분석\n{state['feasibility_report']}\n"
        
        report += """
## 4. Discussion
본 연구는 제한된 시간 내에 자동화된 분석을 수행한 결과입니다.
더 정확한 결론을 위해서는 추가적인 수동 검증이 필요합니다.

## References
검색된 논문 목록은 위 Results 섹션을 참조하세요.

---
*이 보고서는 Virtual Science Lab에 의해 자동 생성되었습니다.*
"""
        
        return {
            "final_report_markdown": report,
            "next_step": "STOP"
        }
