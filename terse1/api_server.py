"""
Virtual Science Lab - FastAPI Backend Server
Next.js 프론트엔드와 연동하기 위한 REST API 서버
"""

from fastapi import FastAPI, HTTPException, BackgroundTasks
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
from typing import Optional, List, Dict, Any
import uuid
import asyncio

# VSL 모듈 import
from virtual_science_lab.workflow import (
    create_vsl_workflow, 
    create_initial_state,
    run_vsl_workflow
)
from virtual_science_lab.state import ScientificState

app = FastAPI(
    title="Virtual Science Lab API",
    description="자율 과학 발견을 위한 멀티 에이전트 시스템",
    version="1.0.0"
)

# CORS 설정 (Next.js 연동)
app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:3000", "http://127.0.0.1:3000"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# 세션 저장소 (메모리, 실제로는 Redis/DB 사용)
sessions: Dict[str, Dict[str, Any]] = {}


# ===== Request/Response Models =====

class ResearchRequest(BaseModel):
    """연구 요청"""
    user_input: str
    domain: Optional[str] = "general"


class MethodSelectionRequest(BaseModel):
    """방법론 선택"""
    session_id: str
    selected_method: int  # 0, 1, 2


class SessionResponse(BaseModel):
    """세션 응답"""
    session_id: str
    status: str
    intent: Optional[str] = None
    domain: Optional[str] = None
    novelty_score: Optional[float] = None
    proposed_methods: Optional[List[Dict]] = None
    final_report: Optional[str] = None
    literature_count: Optional[int] = None


# ===== API Endpoints =====

@app.get("/")
async def root():
    """헬스 체크"""
    return {"status": "ok", "service": "Virtual Science Lab API"}


@app.post("/api/research/start", response_model=SessionResponse)
async def start_research(request: ResearchRequest):
    """
    새로운 연구 세션 시작
    Phase 1: Router → Librarian → PI → (방법론 제안)
    """
    session_id = str(uuid.uuid4())
    
    try:
        # 워크플로우 실행 (Phase 1만)
        workflow = create_vsl_workflow(with_checkpointer=True)
        initial_state = create_initial_state(request.user_input, request.domain)
        
        config = {"configurable": {"thread_id": session_id}}
        
        # 스트리밍 실행 (interrupt까지)
        for step in workflow.stream(initial_state, config):
            pass
        
        # 현재 상태 가져오기
        current_state = workflow.get_state(config)
        state_values = current_state.values
        
        # 세션 저장
        sessions[session_id] = {
            "workflow": workflow,
            "config": config,
            "state": state_values
        }
        
        # 방법론이 제안되었는지 확인
        proposed_methods = state_values.get("proposed_methods", [])
        
        return SessionResponse(
            session_id=session_id,
            status="waiting_selection" if proposed_methods else "completed",
            intent=state_values.get("intent"),
            domain=state_values.get("domain"),
            novelty_score=state_values.get("novelty_score"),
            proposed_methods=[
                {
                    "id": m["method_id"],
                    "title": m["title"],
                    "type": m["approach_type"],
                    "description": m["description"],
                    "pros": m.get("pros", []),
                    "cons": m.get("cons", [])
                }
                for m in proposed_methods
            ] if proposed_methods else None,
            literature_count=len(state_values.get("literature_context", [])),
            final_report=state_values.get("final_report_markdown")
        )
        
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@app.post("/api/research/select-method", response_model=SessionResponse)
async def select_method(request: MethodSelectionRequest):
    """
    방법론 선택 후 연구 계속
    Phase 2: Engineer → Critic → Author
    """
    session_id = request.session_id
    
    if session_id not in sessions:
        raise HTTPException(status_code=404, detail="Session not found")
    
    session = sessions[session_id]
    workflow = session["workflow"]
    config = session["config"]
    
    try:
        # 상태 업데이트
        workflow.update_state(config, {"selected_method": request.selected_method})
        
        # 워크플로우 재개
        for step in workflow.stream(None, config):
            pass
        
        # 최종 상태
        final_state = workflow.get_state(config).values
        
        # 세션 업데이트
        sessions[session_id]["state"] = final_state
        
        return SessionResponse(
            session_id=session_id,
            status="completed",
            intent=final_state.get("intent"),
            domain=final_state.get("domain"),
            novelty_score=final_state.get("novelty_score"),
            final_report=final_state.get("final_report_markdown"),
            literature_count=len(final_state.get("literature_context", []))
        )
        
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@app.get("/api/research/{session_id}", response_model=SessionResponse)
async def get_session(session_id: str):
    """세션 상태 조회"""
    if session_id not in sessions:
        raise HTTPException(status_code=404, detail="Session not found")
    
    state = sessions[session_id]["state"]
    proposed_methods = state.get("proposed_methods", [])
    
    return SessionResponse(
        session_id=session_id,
        status="completed" if state.get("final_report_markdown") else "waiting_selection",
        intent=state.get("intent"),
        domain=state.get("domain"),
        novelty_score=state.get("novelty_score"),
        proposed_methods=[
            {
                "id": m["method_id"],
                "title": m["title"],
                "type": m["approach_type"],
                "description": m["description"]
            }
            for m in proposed_methods
        ] if proposed_methods else None,
        final_report=state.get("final_report_markdown"),
        literature_count=len(state.get("literature_context", []))
    )


@app.delete("/api/research/{session_id}")
async def delete_session(session_id: str):
    """세션 삭제"""
    if session_id in sessions:
        del sessions[session_id]
    return {"status": "deleted", "session_id": session_id}


# ===== Run Server =====

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
