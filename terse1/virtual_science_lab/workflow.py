"""
Virtual Science Lab - LangGraph Workflow
과학 발견 에이전트 워크플로우 정의

Human-in-the-Loop: PI가 방법론을 제안하면 워크플로우가 일시 정지되고
사용자가 선택한 후 재개됨
"""

from langgraph.graph import StateGraph, END
from langgraph.checkpoint.memory import MemorySaver
from typing import Literal, Optional
import uuid

from .state import ScientificState
from .agents.vsl_agents import VSLAgents


def route_after_pi(state: ScientificState) -> str:
    """PI 후 분기"""
    next_step = state.get("next_step", "STOP")
    
    if next_step == "human_selection":
        return "wait_selection"
    elif next_step == "author":
        return "author"
    else:
        return "author"


def route_after_selection(state: ScientificState) -> str:
    """사용자 선택 후 분기"""
    if state.get("selected_method") is not None:
        return "engineer"
    else:
        return "author"


def wait_selection_node(state: ScientificState):
    """
    사용자 선택 대기 노드
    이 노드에서 interrupt가 발생하고, 사용자 입력 후 재개됨
    """
    # 방법론이 제안되었음을 표시
    methods = state.get("proposed_methods", [])
    if methods:
        print("\n" + "=" * 50)
        print("🔬 [Human-in-the-Loop] 방법론 선택이 필요합니다:")
        print("=" * 50)
        for m in methods:
            print(f"  [{m['method_id']}] {m['title']}")
            print(f"      유형: {m['approach_type']}")
            print(f"      설명: {m['description'][:100]}...")
            print()
    return {}


def create_vsl_workflow(with_checkpointer: bool = True):
    """
    Virtual Science Lab 워크플로우 생성
    
    Args:
        with_checkpointer: True면 메모리 체크포인터 사용 (interrupt 지원)
    """
    
    workflow = StateGraph(ScientificState)
    
    # 1. Add Nodes
    workflow.add_node("router", VSLAgents.router_agent)
    workflow.add_node("librarian", VSLAgents.librarian_agent)
    workflow.add_node("pi", VSLAgents.pi_agent)
    workflow.add_node("wait_selection", wait_selection_node)
    workflow.add_node("engineer", VSLAgents.engineer_agent)
    workflow.add_node("critic", VSLAgents.critic_agent)
    workflow.add_node("author", VSLAgents.author_agent)
    
    # 2. Set Entry Point
    workflow.set_entry_point("router")
    
    # 3. Add Edges
    workflow.add_edge("router", "librarian")
    workflow.add_edge("librarian", "pi")
    
    # PI 후 조건부 분기
    workflow.add_conditional_edges(
        "pi",
        route_after_pi,
        {"wait_selection": "wait_selection", "author": "author"}
    )
    
    # 사용자 선택 후 분기
    workflow.add_conditional_edges(
        "wait_selection",
        route_after_selection,
        {"engineer": "engineer", "author": "author"}
    )
    
    workflow.add_edge("engineer", "critic")
    workflow.add_edge("critic", "author")
    workflow.add_edge("author", END)
    
    # 4. Compile with interrupt
    if with_checkpointer:
        checkpointer = MemorySaver()
        # wait_selection 노드 직후에 interrupt
        return workflow.compile(
            checkpointer=checkpointer,
            interrupt_after=["wait_selection"]
        )
    else:
        return workflow.compile()


def create_initial_state(user_input: str, domain: str = "general") -> ScientificState:
    """초기 상태 생성"""
    return ScientificState(
        user_input=user_input,
        domain=domain,
        intent="hypothesis",
        intent_confidence=0.0,
        literature_context=[],
        novelty_score=0.0,
        existing_research_summary=None,
        feasibility_report=None,
        feasibility_rating=None,
        proposed_methods=[],
        selected_method=None,
        code_repository={},
        experiment_result=None,
        debug_attempts=0,
        figures=[],
        final_report_markdown=None,
        final_report_pdf=None,
        next_step="",
        error_message=None,
        session_id=str(uuid.uuid4())
    )


def run_vsl_interactive(user_input: str, domain: str = "general"):
    """
    대화형 워크플로우 실행 (Human-in-the-Loop 포함)
    
    사용자가 방법론을 선택할 때까지 대기하고, 선택 후 재개
    """
    workflow = create_vsl_workflow(with_checkpointer=True)
    initial_state = create_initial_state(user_input, domain)
    
    thread_id = str(uuid.uuid4())
    config = {"configurable": {"thread_id": thread_id}}
    
    print("\n🚀 [Phase 1] 시작: 의도 분류 → 문헌 검색 → 독창성 평가")
    print("=" * 60)
    
    # 첫 번째 실행: router → librarian → pi → wait_selection (interrupt)
    state = None
    for step in workflow.stream(initial_state, config):
        node_name = list(step.keys())[0]
        print(f"  ✓ {node_name} 완료")
        state = step[node_name] if step[node_name] else state
    
    # 현재 상태 확인
    current_state = workflow.get_state(config)
    
    # 방법론이 제안되었는지 확인
    proposed_methods = current_state.values.get("proposed_methods", [])
    
    if proposed_methods:
        # 사용자 입력 대기
        print("\n방법론 번호를 선택하세요 (1, 2, 3): ", end="")
        try:
            choice = int(input().strip()) - 1
            if 0 <= choice < len(proposed_methods):
                selected = choice
            else:
                print("잘못된 선택. 첫 번째 방법론을 사용합니다.")
                selected = 0
        except (ValueError, EOFError):
            print("입력 오류. 첫 번째 방법론을 사용합니다.")
            selected = 0
        
        # 상태 업데이트하여 재개
        print(f"\n🔧 [Phase 2] 선택된 방법론: {proposed_methods[selected]['title']}")
        print("=" * 60)
        
        workflow.update_state(config, {"selected_method": selected})
        
        # 재개: engineer → critic → author
        for step in workflow.stream(None, config):
            node_name = list(step.keys())[0]
            print(f"  ✓ {node_name} 완료")
    
    # 최종 상태 반환
    return workflow.get_state(config).values


def run_vsl_workflow(user_input: str, domain: str = "general") -> ScientificState:
    """
    단순 워크플로우 실행 (Human-in-the-Loop 없이)
    테스트용 또는 자동 선택이 필요한 경우 사용
    """
    workflow = create_vsl_workflow(with_checkpointer=False)
    initial_state = create_initial_state(user_input, domain)
    
    # 자동으로 첫 번째 방법론 선택
    initial_state["selected_method"] = 0
    
    return workflow.invoke(initial_state)

