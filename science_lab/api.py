"""
FastAPI Backend - 웹 인터페이스와 에이전트 시스템 연결
"""
from fastapi import FastAPI, HTTPException, Request
from fastapi.staticfiles import StaticFiles
from fastapi.responses import HTMLResponse, FileResponse, JSONResponse
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
from typing import Optional, List, Dict, Any
import json
import os
from pathlib import Path

from config import HOST, PORT, REPORTS_DIR
from state import create_initial_state
from workflow import run_workflow_simple, continue_workflow
from database import save_session, load_session, get_all_sessions


app = FastAPI(
    title="Autonomous Science Discovery System",
    description="자율 과학 발견 및 가상 실험 시스템",
    version="1.0.0"
)

# CORS 설정
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# 정적 파일 서빙
WEB_DIR = Path(__file__).parent / "web"
if WEB_DIR.exists():
    app.mount("/static", StaticFiles(directory=str(WEB_DIR)), name="static")


# ===== 요청/응답 모델 =====

class ResearchRequest(BaseModel):
    """연구 요청"""
    user_input: str
    domain: Optional[str] = ""


class MethodSelectionRequest(BaseModel):
    """방법론 선택 요청"""
    session_id: str
    method_index: int


class SessionResponse(BaseModel):
    """세션 응답"""
    session_id: str
    status: str
    intent: Optional[str] = None
    intent_confidence: Optional[float] = None
    novelty_score: Optional[float] = None
    feasibility_grade: Optional[str] = None
    proposed_methods: Optional[List[Dict[str, Any]]] = None
    literature_context: Optional[List[Dict[str, Any]]] = None
    final_report: Optional[str] = None
    report_path: Optional[str] = None
    message: Optional[str] = None


# ===== 세션 저장소 (메모리) =====
_sessions: Dict[str, Dict[str, Any]] = {}


# ===== API 엔드포인트 =====

@app.get("/", response_class=HTMLResponse)
async def root():
    """메인 페이지"""
    index_path = WEB_DIR / "index.html"
    if index_path.exists():
        return FileResponse(str(index_path))
    return HTMLResponse("<h1>Science Lab - API Server</h1><p>웹 UI가 설정되지 않았습니다.</p>")


@app.post("/api/research", response_model=SessionResponse)
async def start_research(request: ResearchRequest):
    """
    새 연구 세션 시작
    
    - 가설 또는 질문 입력 받음
    - 의도 분류 및 문헌 검색 수행
    - 가설인 경우: 독창성 평가 + 방법론 제안
    - 질문인 경우: 실현 가능성 평가
    """
    try:
        # 워크플로우 실행
        state = run_workflow_simple(request.user_input, request.domain)
        
        session_id = state.get("session_id", "")
        _sessions[session_id] = dict(state)
        
        # DB 저장
        save_session(session_id, dict(state))
        
        # 응답 구성
        response = SessionResponse(
            session_id=session_id,
            status=state.get("status", "processing"),
            intent=state.get("intent"),
            intent_confidence=state.get("intent_confidence"),
            novelty_score=state.get("novelty_score"),
            feasibility_grade=state.get("feasibility_grade"),
            proposed_methods=state.get("proposed_methods"),
            literature_context=state.get("literature_context", [])[:5],  # 상위 5개만
        )
        
        # 메시지 구성
        if state.get("intent") == "hypothesis":
            if state.get("novelty_score", 0) >= 0.85:
                response.message = "독창적인 가설입니다. 3가지 실험 방법론을 제안합니다."
            else:
                response.message = f"유사한 기존 연구가 발견되었습니다. (독창성: {state.get('novelty_score', 0):.0%})"
                response.status = "completed"
        else:
            response.message = f"실현 가능성 평가 완료: {state.get('feasibility_grade', 'unknown')}"
            response.feasibility_grade = state.get("feasibility_grade")
            response.final_report = state.get("feasibility_report")
            response.status = "completed"
        
        return response
        
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@app.post("/api/research/continue", response_model=SessionResponse)
async def continue_research_anyway(request: ResearchRequest):
    """
    독창성이 낮아도 강제로 연구 계속 진행 (방법론 제안 단계로 이동)
    """
    try:
        # DB에서 세션 로드 (Request body에 session_id가 포함되어야 함, 여기서는 user_input 필드를 session_id로 재사용하거나 새 모델 정의)
        # 편의상 ResearchRequest의 user_input을 session_id로 간주하거나, 별도 모델을 만드는게 정석이지만
        # 퀵 픽스를 위해 ResearchRequest를 재사용하되 user_input에 session_id를 넣는다고 가정 (프론트에서 처리)
        session_id = request.user_input
        
        state = _sessions.get(session_id)
        if not state:
            state = load_session(session_id)
        
        if not state:
            raise HTTPException(status_code=404, detail="Session not found")
            
        # PI 에이전트로 방법론 제안 강제 실행
        from agents.pi import PIAgent
        pi = PIAgent()
        state = pi.propose_methods(state)
        
        # 상태 업데이트
        _sessions[session_id] = dict(state)
        save_session(session_id, dict(state))
        
        return SessionResponse(
            session_id=session_id,
            status=state.get("status", "processing"),
            intent=state.get("intent"),
            intent_confidence=state.get("intent_confidence"),
            novelty_score=state.get("novelty_score"),
            proposed_methods=state.get("proposed_methods"),
            literature_context=state.get("literature_context", [])[:5],
            message="강제로 연구를 진행합니다. 3가지 실험 방법론을 제안합니다."
        )
        
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@app.post("/api/select-method", response_model=SessionResponse)
async def select_method(request: MethodSelectionRequest):
    """
    방법론 선택 후 실험 실행
    
    - 선택된 방법론으로 가상 실험 수행
    - 보고서 작성 및 검토
    """
    try:
        # 세션 로드
        state = _sessions.get(request.session_id)
        if not state:
            state = load_session(request.session_id)
        
        if not state:
            raise HTTPException(status_code=404, detail="Session not found")
        
        # 워크플로우 계속
        state = continue_workflow(state, request.method_index)
        
        # 저장
        _sessions[request.session_id] = dict(state)
        save_session(request.session_id, dict(state))
        
        return SessionResponse(
            session_id=request.session_id,
            status=state.get("status", "completed"),
            final_report=state.get("final_report"),
            report_path=state.get("report_pdf_path"),
            message="연구가 완료되었습니다. 보고서가 생성되었습니다."
        )
        
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@app.get("/api/session/{session_id}")
async def get_session(session_id: str):
    """세션 상태 조회"""
    state = _sessions.get(session_id) or load_session(session_id)
    
    if not state:
        raise HTTPException(status_code=404, detail="Session not found")
    
    return state


@app.get("/api/sessions")
async def list_sessions():
    """모든 세션 목록"""
    return get_all_sessions()


@app.get("/api/report/{session_id}")
async def get_report(session_id: str):
    """보고서 다운로드"""
    state = _sessions.get(session_id) or load_session(session_id)
    
    if not state:
        raise HTTPException(status_code=404, detail="Session not found")
    
    report_path = state.get("report_pdf_path")
    
    if not report_path or not os.path.exists(report_path):
        raise HTTPException(status_code=404, detail="Report not found")
    
    return FileResponse(
        report_path,
        media_type="text/markdown",
        filename=os.path.basename(report_path)
    )


@app.get("/api/health")
async def health_check():
    """헬스 체크"""
    return {"status": "healthy", "version": "1.0.0"}


# ===== 메인 실행 =====

if __name__ == "__main__":
    import uvicorn
    print(f"🚀 Science Lab 서버 시작: http://{HOST}:{PORT}")
    uvicorn.run(app, host=HOST, port=PORT)
