# =============================================================================
# T_lab - State Definitions
# Unified state for workflow management (from science_lab)
# =============================================================================

from typing import TypedDict, List, Dict, Any, Optional, Literal
from dataclasses import dataclass, field
from datetime import datetime
import uuid


class ScientificState(TypedDict, total=False):
    """Main state object for the research workflow."""
    
    # Session
    session_id: str
    created_at: str
    
    # User Input
    user_input: str
    domain: str
    
    # Intent Classification (Router)
    intent: Literal["hypothesis", "question", "unknown"]
    intent_confidence: float
    
    # Simulatability Check (Router)
    simulatable: bool
    simulation_reason: str
    alternative_method: Optional[str]
    
    # Literature Search (Librarian)
    literature_context: List[Dict[str, Any]]
    search_queries: List[str]
    
    # Novelty Assessment (PI)
    novelty_score: float
    novelty_reasoning: str
    feasibility_grade: str
    
    # Method Proposal (PI)
    proposed_methods: List[Dict[str, Any]]
    selected_method: Dict[str, Any]
    selected_method_index: int
    
    # Experiment (Engineer)
    experiment_code: str
    experiment_results: Dict[str, Any]
    execution_logs: List[str]
    
    # Simulation (ExperimentRunner)
    simulation_params: Dict[str, Any]
    simulation_results: Dict[str, Any]
    
    # Report (Author)
    draft_report: str
    final_report: str
    report_path: str
    
    # Review (Critic)
    critique: str
    improvements: List[str]
    quality_score: float
    
    # Fact Check
    verified_citations: List[Dict[str, Any]]
    unverified_citations: List[str]
    
    # Workflow Control
    status: Literal["pending", "running", "paused", "completed", "failed"]
    current_step: str
    error: Optional[str]
    logic_chain: List[Dict[str, Any]]
    activity_log: List[Dict[str, str]]


def create_initial_state(user_input: str, domain: str = "") -> ScientificState:
    """Create a new state with initial values."""
    return ScientificState(
        session_id=str(uuid.uuid4()),
        created_at=datetime.now().isoformat(),
        user_input=user_input,
        domain=domain,
        intent="unknown",
        intent_confidence=0.0,
        simulatable=True,
        simulation_reason="",
        alternative_method=None,
        literature_context=[],
        search_queries=[],
        novelty_score=0.0,
        novelty_reasoning="",
        feasibility_grade="",
        proposed_methods=[],
        selected_method={},
        selected_method_index=-1,
        experiment_code="",
        experiment_results={},
        execution_logs=[],
        simulation_params={},
        simulation_results={},
        draft_report="",
        final_report="",
        report_path="",
        critique="",
        improvements=[],
        quality_score=0.0,
        verified_citations=[],
        unverified_citations=[],
        status="pending",
        current_step="init",
        error=None,
        logic_chain=[],
        activity_log=[]
    )
