# =============================================================================
# T_lab - FastAPI Main Application
# Combines endpoints from both science_lab and v_lab
# =============================================================================

from fastapi import FastAPI, HTTPException, status, BackgroundTasks
from fastapi.responses import FileResponse
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel, Field
from typing import Any, Dict, List, Optional
from datetime import datetime
import uuid
import asyncio # Added for SSE queue logic


from core import get_settings, get_logger
from state import ScientificState, create_initial_state
from models import SessionModel, SessionLocal, init_db
from sqlalchemy.orm import Session

# =============================================================================
# App Setup
# =============================================================================

settings = get_settings()
logger = get_logger("tlab.api")

app = FastAPI(
    title="T_lab API",
    description="Unified Virtual Laboratory - AI-Powered Research Platform",
    version="1.0.0 BETA",
    docs_url="/docs",
    redoc_url="/redoc"
)

app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:3000", "http://127.0.0.1:3000"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


# =============================================================================
# In-Memory Store
# =============================================================================

class PersistentStore:
    """Manages research session state in PostgreSQL."""
    
    def create_session(self, user_input: str, domain: str = "") -> ScientificState:
        state = create_initial_state(user_input, domain)
        
        # SQLite Fix: Remove string created_at so SQLAlchemy uses the default datetime object
        if "created_at" in state:
            del state["created_at"]
            
        db = SessionLocal()
        try:
            db_session = SessionModel(**state)
            db.add(db_session)
            db.commit()
            db.refresh(db_session)
            
            # Reconstruct state dictionary
            return {c.name: getattr(db_session, c.name) for c in db_session.__table__.columns}
        finally:
            db.close()

    def get_session(self, sid: str) -> Optional[ScientificState]:
        db = SessionLocal()
        try:
            db_session = db.query(SessionModel).filter(SessionModel.session_id == sid).first()
            if db_session:
                # Convert model to dict
                return {c.name: getattr(db_session, c.name) for c in db_session.__table__.columns}
            return None
        finally:
            db.close()

    def update_session(self, sid: str, **updates) -> Optional[ScientificState]:
        db = SessionLocal()
        try:
            db_session = db.query(SessionModel).filter(SessionModel.session_id == sid).first()
            if db_session:
                for key, value in updates.items():
                    # Protect append-only fields and primary keys
                    if key in ["activity_log", "session_id", "created_at"]:
                        continue
                    if hasattr(db_session, key):
                        setattr(db_session, key, value)
                db.commit()
                db.refresh(db_session)
                return {c.name: getattr(db_session, c.name) for c in db_session.__table__.columns}
            return None
        finally:
            db.close()
    
    def add_log(self, sid: str, message: str, agent: str = "System"):
        db = SessionLocal()
        try:
            db_session = db.query(SessionModel).filter(SessionModel.session_id == sid).first()
            if db_session:
                logs = list(db_session.activity_log or [])
                timestamp = datetime.now().strftime("%H:%M:%S")
                logs.append({
                    "time": timestamp,
                    "agent": agent,
                    "message": message
                })
                db_session.activity_log = logs
                db.commit()
                logger.info(f"[{agent}] {message}", session_id=sid)
        finally:
            db.close()
    
    def list_sessions(self, limit: int = 50) -> List[ScientificState]:
        db = SessionLocal()
        try:
            sessions = db.query(SessionModel).order_by(SessionModel.created_at.desc()).limit(limit).all()
            return [{c.name: getattr(s, c.name) for c in s.__table__.columns} for s in sessions]
        finally:
            db.close()

    def delete_session(self, sid: str) -> bool:
        db = SessionLocal()
        try:
            db_session = db.query(SessionModel).filter(SessionModel.session_id == sid).first()
            if db_session:
                db.delete(db_session)
                db.commit()
                return True
            return False
        finally:
            db.close()

# Initialize DB and Store
init_db()
memory_store = PersistentStore()
logger.info(f"Initialized SQLite Persistent Store")


# =============================================================================
# Request/Response Models
# =============================================================================

class ResearchRequest(BaseModel):
    user_input: str = Field(..., min_length=5, description="Hypothesis or research question")
    domain: str = Field(default="", description="Research domain (e.g., biology, physics)")


class MethodSelectionRequest(BaseModel):
    session_id: str
    method_index: int = Field(..., ge=0, le=4)


class SessionResponse(BaseModel):
    session_id: str
    status: str
    intent: Optional[str] = None
    intent_confidence: Optional[float] = None
    novelty_score: Optional[float] = None
    feasibility_grade: Optional[str] = None
    proposed_methods: Optional[List[Dict[str, Any]]] = None
    selected_method: Optional[Dict[str, Any]] = None
    literature_context: Optional[List[Dict[str, Any]]] = None
    simulation_results: Optional[Dict[str, Any]] = None
    final_report: Optional[str] = None
    report_path: Optional[str] = None
    message: Optional[str] = None
    current_step: Optional[str] = None
    current_step_label: Optional[str] = None
    activity_log: Optional[List[Dict[str, str]]] = None


# =============================================================================
# Endpoints
# =============================================================================

@app.get("/health")
async def health_check():
    """Health check endpoint."""
    return {
        "status": "healthy",
        "version": "1.0.0 BETA",
        "storage": "memory",
        "api_keys": {
            "openai": bool(settings.openai_api_key),
            "tavily": bool(settings.tavily_api_key)
        }
    }



# =============================================================================
# Stream Endpoint (WebSocket)
# =============================================================================

from fastapi import WebSocket, WebSocketDisconnect
from fastapi.staticfiles import StaticFiles
from connection_manager import ws_manager
import os

# Mount static directory for plots
if not os.path.exists("static"):
    os.makedirs("static")
app.mount("/static", StaticFiles(directory="static"), name="static")

# Hook into persistent store
# Hook into persistent store
original_add_log = PersistentStore.add_log

# Capture main loop for thread-safe execution
main_event_loop = None

@app.on_event("startup")
async def startup_event():
    global main_event_loop
    main_event_loop = asyncio.get_running_loop()

def broadcast_log_wrapper(self, sid: str, message: str, agent: str = "System"):
    # Call original to save to DB
    original_add_log(self, sid, message, agent)
    # Broadcast to WebSocket
    coro = ws_manager.broadcast(sid, {
        "type": "log",
        "timestamp": datetime.now().strftime("%H:%M:%S"),
        "agent": agent,
        "message": message
    })
    
    try:
        # If in async context, use create_task
        loop = asyncio.get_running_loop()
        loop.create_task(coro)
    except RuntimeError:
        # If in sync thread (LangGraph node), use threadsafe
        if main_event_loop:
            asyncio.run_coroutine_threadsafe(coro, main_event_loop)

PersistentStore.add_log = broadcast_log_wrapper


@app.websocket("/ws/research/{session_id}")
async def websocket_endpoint(websocket: WebSocket, session_id: str):
    await ws_manager.connect(websocket, session_id)
    try:
        while True:
            # Keep connection alive
            await websocket.receive_text()
    except WebSocketDisconnect:
        ws_manager.disconnect(websocket, session_id)



async def run_initial_research_background(session_id: str):
    """Run the initial research graph in background."""
    try:
        from workflow import build_research_graph
        graph = build_research_graph()
        
        state = memory_store.get_session(session_id)
        if not state:
            return

        # Run the graph
        # Note: LangGraph invoke returns the final state
        # We need to rely on the nodes updating the DB incrementally
        # But we should also sync the final state
        final_state = graph.invoke(state)
        
        # Ensure final state is saved (especially status updates from graph)
        # Note: State from graph might be a Dict, convert as needed
        memory_store.update_session(session_id, **final_state)
        
        logger.info("Initial research graph completed", session_id=session_id)
        
    except Exception as e:
        logger.error(f"Initial research failed: {e}", session_id=session_id)
        memory_store.update_session(session_id, status="failed", error=str(e))



@app.post("/research/start", response_model=SessionResponse, status_code=status.HTTP_202_ACCEPTED)
async def start_research(request: ResearchRequest, background_tasks: BackgroundTasks):
    """
    Start the research process for a given input in background.
    """
    logger.info("Starting research session", input=request.user_input[:50], source="api")
    
    # Create session
    state = memory_store.create_session(request.user_input, request.domain)
    session_id = state["session_id"]
    
    try:
        # Start background task
        background_tasks.add_task(run_initial_research_background, session_id)
        
        return SessionResponse(
            session_id=session_id,
            status="running",
            current_step="init",
            current_step_label="🚀 연구 프로세스를 시작하는 중",
            message="Research initialization started."
        )
        
    except Exception as e:
        logger.error(f"Research start failed: {e}", source="api")
        memory_store.update_session(session_id, status="failed", error=str(e))
        raise HTTPException(status_code=500, detail=str(e))


async def run_research_background(session_id: str):
    """Run execution graph in background."""
    try:
        from workflow import build_execution_graph
        graph = build_execution_graph()
        
        state = memory_store.get_session(session_id)
        if not state:
            return

        final_state = await graph.ainvoke(state)
        memory_store.update_session(session_id, **final_state)
        
        logger.info("Research execution graph completed", session_id=session_id)
        
    except Exception as e:
        logger.error(f"Research execution failed: {e}", session_id=session_id)
        memory_store.update_session(session_id, status="failed", error=str(e))



@app.post("/research/select-method", response_model=SessionResponse, status_code=status.HTTP_202_ACCEPTED)
async def select_method(request: MethodSelectionRequest, background_tasks: BackgroundTasks):
    """
    Continue research after method selection in background.
    """
    session = memory_store.get_session(request.session_id)
    if not session:
        raise HTTPException(status_code=404, detail="Session not found")
    
    logger.info("Method selected, starting background task", session_id=request.session_id, index=request.method_index, source="api")
    
    try:
        # Set selected method
        methods = session.get("proposed_methods", [])
        if request.method_index >= len(methods):
            raise HTTPException(status_code=400, detail="Invalid method index")
        
        selected = methods[request.method_index]
        
        # Clear previous logs (Router/PI) and start fresh for execution phase
        memory_store.update_session(request.session_id, 
            selected_method=selected,
            selected_method_index=request.method_index,
            status="running",
            activity_log=[]  # Clear logs for execution phase
        )
        
        # Add tasks to background
        background_tasks.add_task(run_research_background, request.session_id)
        
        # Return FULL updated session
        updated_session = memory_store.get_session(request.session_id)
        if not updated_session:
             raise HTTPException(status_code=404, detail="Session lost")
             
        return SessionResponse(**updated_session)
        
    except Exception as e:
        logger.error(f"Method selection failed: {e}", source="api")
        memory_store.update_session(request.session_id, status="failed", error=str(e))
        raise HTTPException(status_code=500, detail=str(e))


@app.get("/sessions")
async def list_sessions(limit: int = 50):
    """List all research sessions."""
    sessions = memory_store.list_sessions(limit)
    return {
        "count": len(sessions),
        "sessions": [
            {
                "session_id": s["session_id"],
                "status": s["status"],
                "intent": s.get("intent"),
                "domain": s.get("domain"),
                "created_at": s.get("created_at"),
                "user_input": s["user_input"][:100]
            }
            for s in sessions
        ]
    }


@app.get("/sessions/{session_id}")
async def get_session(session_id: str):
    """Get session details."""
    session = memory_store.get_session(session_id)
    if not session:
        raise HTTPException(status_code=404, detail="Session not found")
    return session


@app.delete("/sessions/{session_id}")
async def delete_session(session_id: str):
    """Delete a session."""
    if memory_store.delete_session(session_id):
        return {"message": "Session deleted"}
    raise HTTPException(status_code=404, detail="Session not found")


# =============================================================================
# Paper Synthesis (from v_lab)
# =============================================================================

class SynthesisRequest(BaseModel):
    session_ids: List[str] = Field(..., min_length=1)


@app.post("/papers/synthesize")
async def synthesize_papers(request: SynthesisRequest):
    """Synthesize multiple research sessions into a review paper."""
    from agents.paper_synthesizer import synthesize_paper
    
    sessions = []
    for sid in request.session_ids:
        session = memory_store.get_session(sid)
        if session and session.get("status") == "completed":
            sessions.append(session)
    
    if not sessions:
        raise HTTPException(status_code=400, detail="No valid completed sessions found")
    
    paper = synthesize_paper(sessions)
    return paper


# =============================================================================
# Main Entry
# =============================================================================

if __name__ == "__main__":
    import uvicorn
    logger.info(f"Starting T_lab API on {settings.api_host}:{settings.api_port}")
    uvicorn.run(app, host=settings.api_host, port=settings.api_port)

@app.delete("/sessions/{session_id}", status_code=status.HTTP_204_NO_CONTENT)
async def delete_session(session_id: str):
    """Delete a research session."""
    success = memory_store.delete_session(session_id)
    if not success:
        raise HTTPException(status_code=404, detail="Session not found")
    return None


@app.get("/sessions/{session_id}/download")
async def download_report(session_id: str):
    """Generate and download PDF version of the research report."""
    session = memory_store.get_session(session_id)
    if not session:
        raise HTTPException(status_code=404, detail="Session not found")
    
    report_text = session.get("final_report")
    if not report_text:
        raise HTTPException(status_code=400, detail="Report not generated yet")
    
    # Import PDF generator
    from tools.pdf_generator import create_pdf
    
    # Create temp directory for PDFs if it doesn't exist
    pdf_dir = os.path.join(os.getcwd(), "tmp_pdfs")
    os.makedirs(pdf_dir, exist_ok=True)
    
    pdf_path = os.path.join(pdf_dir, f"report_{session_id[:8]}.pdf")
    
    # Check for result image
    image_path = f"static/{session_id}_result.png"
    if not os.path.exists(image_path):
        image_path = None
        
    create_pdf(report_text, pdf_path, title=f"Research Report: {session.get('user_input', 'Untitled')[:50]}...", image_path=image_path)
    
    return FileResponse(
        path=pdf_path,
        filename=f"T_lab_Research_{session_id[:8]}.pdf",
        media_type="application/pdf"
    )

# =============================================================================
# Stream Endpoint
# =============================================================================

from fastapi.responses import StreamingResponse
import asyncio
import json



import os

