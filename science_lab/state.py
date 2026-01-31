"""
ScientificState - 연구 세션의 전역 상태 관리
LangGraph의 상태 스키마로 사용됨
"""
from typing import TypedDict, List, Dict, Optional, Any
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum


class Intent(str, Enum):
    """사용자 의도 분류"""
    HYPOTHESIS = "hypothesis"
    QUESTION = "question"
    UNCLEAR = "unclear"


class SessionStatus(str, Enum):
    """연구 세션 상태"""
    PROCESSING = "processing"
    WAITING_INPUT = "waiting_input"
    COMPLETED = "completed"
    FAILED = "failed"


class MethodologyType(str, Enum):
    """방법론 유형"""
    ANALYTICAL = "analytical"      # 이론/분석적 접근
    SIMULATION = "simulation"       # 시뮬레이션 접근
    DATA_DRIVEN = "data_driven"     # 데이터 기반/ML 접근


@dataclass
class Methodology:
    """제안된 실험 방법론"""
    type: MethodologyType
    title: str
    description: str
    advantages: List[str]
    disadvantages: List[str]
    estimated_time: str
    required_libraries: List[str]


@dataclass
class LiteratureItem:
    """검색된 학술 문헌"""
    title: str
    authors: List[str]
    abstract: str
    year: int
    source: str  # 'arxiv', 'semantic_scholar'
    url: str
    relevance_score: float = 0.0


@dataclass
class ExperimentResult:
    """실험 결과"""
    success: bool
    code: str
    stdout: str
    stderr: str
    figures: List[str] = field(default_factory=list)
    data_files: List[str] = field(default_factory=list)
    retry_count: int = 0


class ScientificState(TypedDict, total=False):
    """
    연구 세션의 전역 상태 - LangGraph 노드 간 공유
    
    모든 에이전트는 이 상태를 읽고 쓰며 작업을 수행
    """
    # 기본 정보
    session_id: str
    user_input: str
    domain: str
    created_at: str
    status: str
    
    # 의도 분류 결과
    intent: str  # 'hypothesis' | 'question'
    intent_confidence: float
    intent_reasoning: str
    
    # 문헌 검색 결과
    literature_context: List[Dict[str, Any]]
    search_queries: List[str]
    
    # 독창성 평가 (가설인 경우)
    novelty_score: float
    novelty_analysis: str
    existing_research: List[Dict[str, Any]]
    
    # 실현 가능성 평가 (질문인 경우)
    feasibility_report: str
    feasibility_grade: str  # 'high' | 'medium' | 'low' | 'uncertain'
    
    # 방법론 제안
    proposed_methods: List[Dict[str, Any]]
    selected_method_index: int
    
    # 실험 코드 및 결과
    code_repository: Dict[str, str]  # {filename: code_content}
    execution_logs: str
    experiment_success: bool
    retry_count: int
    
    # 생성된 산출물
    figures: List[str]  # 경로 목록
    data_outputs: List[str]
    experiment_results: Dict[str, Any]  # results.json 파싱 결과
    
    # 최종 보고서
    draft_report: str
    critic_feedback: str
    final_report: str
    report_pdf_path: str
    
    # 메시지 히스토리
    messages: List[Dict[str, str]]
    
    # 에러 추적
    errors: List[str]
    current_step: str


def create_initial_state(user_input: str, domain: str = "") -> ScientificState:
    """초기 상태 생성"""
    import uuid
    return ScientificState(
        session_id=str(uuid.uuid4()),
        user_input=user_input,
        domain=domain,
        created_at=datetime.now().isoformat(),
        status=SessionStatus.PROCESSING.value,
        intent="",
        intent_confidence=0.0,
        intent_reasoning="",
        literature_context=[],
        search_queries=[],
        novelty_score=0.0,
        novelty_analysis="",
        existing_research=[],
        feasibility_report="",
        feasibility_grade="",
        proposed_methods=[],
        selected_method_index=-1,
        code_repository={},
        execution_logs="",
        experiment_success=False,
        retry_count=0,
        figures=[],
        data_outputs=[],
        experiment_results={},
        draft_report="",
        critic_feedback="",
        final_report="",
        report_pdf_path="",
        messages=[],
        errors=[],
        current_step="initialized"
    )
