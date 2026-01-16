from langgraph.graph import StateGraph, END
from .state import AgentState
from .nodes import DAACSNodes

def create_workflow():
    # 1. Initialize Graph
    workflow = StateGraph(AgentState)

    # 2. Add Nodes
    workflow.add_node("planner", DAACSNodes.planner_node)
    workflow.add_node("supervisor", DAACSNodes.supervisor_node)
    workflow.add_node("writer", DAACSNodes.writer_node)
    workflow.add_node("reviewer", DAACSNodes.reviewer_node)
    workflow.add_node("publisher", DAACSNodes.publisher_node)

    # 3. Add Edges
    # Start -> Planner
    workflow.set_entry_point("planner")
    workflow.add_edge("planner", "supervisor")

    # Supervisor Conditional Logic
    workflow.add_conditional_edges(
        "supervisor",
        lambda state: state["next_step"],
        {
            "writer": "writer",
            "reviewer": "reviewer",
            "publisher": "publisher",
            "STOP": END
        }
    )

    # Worker -> Supervisor (Loop back)
    workflow.add_edge("writer", "supervisor")
    workflow.add_edge("reviewer", "supervisor")
    workflow.add_edge("publisher", END)

    # 4. Compile
    return workflow.compile()
