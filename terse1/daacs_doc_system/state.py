from typing import List, Dict, TypedDict, Optional, Any, Literal
import operator

class ChapterPlan(TypedDict):
    """챕터 기획 정보"""
    chapter_id: str
    title: str
    key_points: List[str]
    estimated_words: int
    dependencies: List[str]  # 의존성 ID 목록
    status: Literal["pending", "writing", "reviewing", "complete"]

class CritiqueItem(TypedDict):
    """검수 피드백 항목"""
    round: int
    type: Literal["content", "style", "fact"]
    location: str
    comment: str
    fixed: bool

class AgentState(TypedDict):
    """
    DAACS v3 전역 상태 스키마
    LangGraph의 StateGraph에 사용됨
    """
    # 1. Input & Plan
    task: str                       # 사용자 요청
    plan: List[ChapterPlan]         # 기획된 목차
    
    # 2. Execution Context
    draft_refs: Dict[str, str]      # {chapter_id: file_path}
    reference_data: List[str]       # 수집된 자료 (URL or Content Snippet)
    
    # 3. Review & Feedback
    # operator.add를 사용하여 리스트를 덮어쓰지 않고 추가(append)함
    critique_history: List[CritiqueItem] 
    
    # 4. Control Flow
    revision_count: int             # 루프 반복 횟수
    next_step: str                  # Supervisor의 라우팅 결정
    final_artifact_path: Optional[str] # 최종 결과물 경로
    
    # 5. Failure Tracking (V6 Integration)
    failure_type: Optional[str]     # 감지된 실패 유형
    consecutive_failures: int       # 연속 실패 횟수 (Ping-Pong Guard)
