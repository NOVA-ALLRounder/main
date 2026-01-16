"""
Virtual Science Lab (VSL) - State Definitions
자율 과학 발견 에이전트의 전역 상태 스키마
"""

from typing import List, Dict, TypedDict, Optional, Literal
from pathlib import Path


class LiteratureItem(TypedDict):
    """검색된 논문 정보"""
    paper_id: str
    title: str
    abstract: str
    authors: List[str]
    year: int
    citation_count: int
    source: Literal["semantic_scholar", "arxiv", "pubmed"]
    url: Optional[str]


class MethodologyProposal(TypedDict):
    """제안된 실험 방법론"""
    method_id: int
    approach_type: Literal["analytical", "simulation", "data_driven"]
    title: str
    description: str
    required_libraries: List[str]
    estimated_complexity: Literal["low", "medium", "high"]
    pros: List[str]
    cons: List[str]


class ExperimentResult(TypedDict):
    """실험 실행 결과"""
    success: bool
    stdout: str
    stderr: str
    generated_files: List[str]
    execution_time_seconds: float


class ScientificState(TypedDict):
    """
    Virtual Science Lab 전역 상태
    LangGraph StateGraph에서 사용
    """
    # 1. User Input
    user_input: str                 # 원본 입력 (가설 또는 질문)
    domain: str                     # 연구 분야 (physics, biology, cs, etc.)
    
    # 2. Intent Classification
    intent: Literal["hypothesis", "question"]
    intent_confidence: float        # 분류 신뢰도 (0.0 ~ 1.0)
    
    # 3. Literature Context
    literature_context: List[LiteratureItem]
    novelty_score: float            # 독창성 점수 (0.0 ~ 1.0)
    existing_research_summary: Optional[str]  # 기존 연구 요약 (이미 검증된 경우)
    
    # 4. Feasibility (for questions)
    feasibility_report: Optional[str]
    feasibility_rating: Optional[Literal["high", "medium", "low", "uncertain"]]
    
    # 5. Methodology (for hypotheses)
    proposed_methods: List[MethodologyProposal]
    selected_method: Optional[int]  # 사용자가 선택한 인덱스
    
    # 6. Virtual Experiment
    code_repository: Dict[str, str]  # {filename: code_content}
    experiment_result: Optional[ExperimentResult]
    debug_attempts: int              # Self-healing 시도 횟수
    
    # 7. Output
    figures: List[str]              # 생성된 이미지 경로
    final_report_markdown: Optional[str]
    final_report_pdf: Optional[str]
    
    # 8. Control Flow
    next_step: str
    error_message: Optional[str]
    session_id: str
