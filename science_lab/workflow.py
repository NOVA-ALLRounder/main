"""
LangGraph 기반 워크플로우 정의
에이전트 노드 연결 및 라우팅 로직
"""
from typing import Dict, Any, Literal
import json

try:
    from langgraph.graph import StateGraph, END
    from langgraph.checkpoint.memory import MemorySaver
    LANGGRAPH_AVAILABLE = True
except ImportError:
    LANGGRAPH_AVAILABLE = False

from state import ScientificState, create_initial_state
from agents.router import router_node
from agents.librarian import librarian_node
from agents.pi import pi_novelty_node, pi_methods_node, pi_feasibility_node
from agents.engineer import engineer_node
from agents.critic import critic_node
from agents.author import author_draft_node, author_refine_node
from agents.fact_checker import fact_checker_node
from config import NOVELTY_THRESHOLD


def should_continue_after_router(state: ScientificState) -> Literal["librarian", "end"]:
    """라우터 후 다음 단계 결정"""
    if state.get("intent") == "unclear" or state.get("intent_confidence", 0) < 0.3:
        # 의도가 불분명하면 종료 (사용자 명확화 필요)
        return "end"
    return "librarian"


def route_after_librarian(state: ScientificState) -> Literal["novelty_check", "feasibility_check"]:
    """문헌 검색 후 경로 결정"""
    if state.get("intent") == "hypothesis":
        return "novelty_check"
    else:
        return "feasibility_check"


def route_after_novelty(state: ScientificState) -> Literal["propose_methods", "end"]:
    """독창성 평가 후 경로 결정"""
    score = state.get("novelty_score", 0)
    if score >= NOVELTY_THRESHOLD:
        # 독창적 가설 -> 방법론 제안
        return "propose_methods"
    else:
        # 기존 연구 존재 -> 리포트 생성 후 종료
        return "end"


def should_run_experiment(state: ScientificState) -> Literal["experiment", "wait_selection"]:
    """실험 실행 여부 결정"""
    selected = state.get("selected_method_index", -1)
    if selected >= 0:
        return "experiment"
    else:
        return "wait_selection"


def build_graph():
    """
    과학 연구 워크플로우 그래프 구축
    
    흐름:
    1. Router: 의도 분류
    2. Librarian: 문헌 검색
    3. PI: 독창성 평가 (가설) 또는 실현 가능성 평가 (질문)
    4. PI: 방법론 제안 (독창적 가설인 경우)
    5. [User Selection] 방법론 선택
    6. Engineer: 실험 실행
    7. Author: 초안 작성
    8. Critic: 검토
    9. Author: 최종 보고서 수정
    """
    
    if not LANGGRAPH_AVAILABLE:
        return None
    
    # 그래프 생성
    graph = StateGraph(ScientificState)
    
    # 노드 추가
    graph.add_node("router", router_node)
    graph.add_node("librarian", librarian_node)
    graph.add_node("novelty_check", pi_novelty_node)
    graph.add_node("feasibility_check", pi_feasibility_node)
    graph.add_node("propose_methods", pi_methods_node)
    graph.add_node("experiment", engineer_node)
    graph.add_node("draft_report", author_draft_node)
    graph.add_node("fact_check", fact_checker_node)
    graph.add_node("critic_review", critic_node)
    graph.add_node("finalize_report", author_refine_node)
    
    # 엣지 정의
    graph.set_entry_point("router")
    
    graph.add_conditional_edges(
        "router",
        should_continue_after_router,
        {
            "librarian": "librarian",
            "end": END
        }
    )
    
    graph.add_conditional_edges(
        "librarian",
        route_after_librarian,
        {
            "novelty_check": "novelty_check",
            "feasibility_check": "feasibility_check"
        }
    )
    
    graph.add_conditional_edges(
        "novelty_check",
        route_after_novelty,
        {
            "propose_methods": "propose_methods",
            "end": END
        }
    )
    
    # 실현 가능성 평가 후 종료
    graph.add_edge("feasibility_check", END)
    
    # 방법론 제안 후 일시 정지 (Human-in-the-Loop)
    # 실제로는 여기서 interrupt하고 사용자 선택을 기다림
    graph.add_edge("propose_methods", END)  # 임시 - API에서 처리
    
    # 실험 -> 초안 -> 팩트체크 -> 검토 -> 최종
    graph.add_edge("experiment", "draft_report")
    graph.add_edge("draft_report", "fact_check")
    graph.add_edge("fact_check", "critic_review")
    graph.add_edge("critic_review", "finalize_report")
    graph.add_edge("finalize_report", END)
    
    # 체크포인터 설정 (메모리 기반)
    memory = MemorySaver()
    
    return graph.compile(checkpointer=memory)


# 전역 그래프 인스턴스
_workflow_graph = None


def get_workflow_graph():
    """워크플로우 그래프 인스턴스 반환"""
    global _workflow_graph
    if _workflow_graph is None:
        _workflow_graph = build_graph()
    return _workflow_graph


def run_workflow(
    user_input: str,
    domain: str = "",
    session_id: str = None
) -> ScientificState:
    """
    워크플로우 실행
    
    Args:
        user_input: 사용자 입력 (가설 또는 질문)
        domain: 연구 도메인
        session_id: 세션 ID (재개용)
    
    Returns:
        최종 상태
    """
    graph = get_workflow_graph()
    
    if graph is None:
        # LangGraph 없으면 직접 실행
        return run_workflow_simple(user_input, domain)
    
    # 초기 상태 생성
    initial_state = create_initial_state(user_input, domain)
    
    # 그래프 실행
    config = {"configurable": {"thread_id": session_id or initial_state["session_id"]}}
    
    final_state = graph.invoke(initial_state, config)
    
    return final_state


def run_workflow_simple(user_input: str, domain: str = "") -> ScientificState:
    """LangGraph 없이 간단한 워크플로우 실행"""
    from agents.router import RouterAgent
    from agents.librarian import LibrarianAgent
    from agents.pi import PIAgent
    from agents.engineer import EngineerAgent
    from agents.author import AuthorAgent
    from agents.critic import CriticAgent
    
    state = create_initial_state(user_input, domain)
    
    # 1. 의도 분류
    router = RouterAgent()
    state = router.classify(state)
    
    # 2. 문헌 검색
    librarian = LibrarianAgent()
    state = librarian.search(state)
    
    pi = PIAgent()
    
    # 3. 의도에 따른 분기
    if state.get("intent") == "hypothesis":
        # 독창성 평가
        state = pi.evaluate_novelty(state)
        
        if state.get("novelty_score", 0) >= NOVELTY_THRESHOLD:
            # 방법론 제안
            state = pi.propose_methods(state)
            # 여기서 사용자 선택 대기
    else:
        # 실현 가능성 평가
        state = pi.evaluate_feasibility(state)
    
    return state


def continue_workflow(
    state: ScientificState,
    selected_method_index: int
) -> ScientificState:
    """
    방법론 선택 후 워크플로우 계속
    
    Args:
        state: 현재 상태
        selected_method_index: 선택된 방법론 인덱스
    
    Returns:
        최종 상태
    """
    from agents.engineer import EngineerAgent
    from agents.author import AuthorAgent
    from agents.critic import CriticAgent
    from agents.fact_checker import FactCheckerAgent
    
    state["selected_method_index"] = selected_method_index
    state["status"] = "processing"
    
    # 4. 실험 실행
    print("\n[Workflow] 🧪 가상 실험 시작 (Engineer Agent)...")
    engineer = EngineerAgent()
    state = engineer.run_experiment(state)
    
    # 5. 초안 작성
    print("\n[Workflow] 📝 보고서 초안 작성 중 (Author Agent)...")
    author = AuthorAgent()
    state = author.write_draft(state)
    
    # 6. 사실 검증 (팩트체크)
    print("\n[Workflow] 🔍 사실 검증 중 (FactChecker Agent)...")
    fact_checker = FactCheckerAgent()
    state = fact_checker.verify(state)
    
    # 팩트체크 결과 확인
    fact_result = state.get('fact_check_result', {})
    if fact_result.get('requires_revision'):
        print(f"\n[Workflow] ⚠️ 사실 검증 문제 발견: {fact_result.get('summary')}")
        state['fact_check_issues'] = fact_result.get('factual_errors', [])
    
    # 7. 비평 검토
    print("\n[Workflow] 🧐 동료 심사 및 검증 중 (Critic Agent)...")
    critic = CriticAgent()
    state = critic.review(state)
    
    # 8. 최종 보고서
    print("\n[Workflow] 📑 최종 보고서 생성 중 (Author Agent)...")
    state = author.refine_report(state)
    
    return state
