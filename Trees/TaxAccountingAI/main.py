from fastapi import FastAPI, HTTPException
from fastapi.staticfiles import StaticFiles
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
import os
import sys
import uvicorn
from typing import List, Optional

# Helper to import llm.py
sys.path.append(os.path.dirname(os.path.abspath(__file__)))
try:
    from llm import generate_response
except ImportError:
    generate_response = lambda prompt, system: "LLM Module not found."

from rag.embedder import Embedder
from rag.vector_store import VectorStore

app = FastAPI()

# Initialize RAG Components
print("Initializing RAG Engine...")
embedder = Embedder() # This might take a moment to load
vector_store = VectorStore(persist_directory="./data/chroma", collection_name="tax_accounting_db")
print("RAG Engine Ready.")

# CORS for dev
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

class ChatRequest(BaseModel):
    message: str
    history: Optional[List[dict]] = []

class ChatResponse(BaseModel):
    response: str
    context: Optional[List[dict]] = []

@app.post("/api/chat", response_model=ChatResponse)
async def chat_endpoint(req: ChatRequest):
    """
    RAG-enabled chat endpoint.
    """
    try:
        # 1. Retrieval
        query_vec = embedder.embed_text(req.message)
        results = vector_store.query(query_vec, n_results=3)
        
        context_str = ""
        context_list = []
        if results:
            context_str = "\\n".join([f"- {r['content']} (Source: {r['metadata'].get('source', 'Unknown')})" for r in results])
            context_list = [{"content": r['content'], "source": r['metadata'].get('source')} for r in results]
        
        # 2. Augmentation
        system_msg = (
            "You are a helpful AI assistant specializing in tax, accounting, and government support for Korean users.\\n"
            "Use the following Context to answer the user's question. If the answer is not in the context, use your general knowledge but mention that you are not sure.\\n\\n"
            f"Context:\\n{context_str}"
        )
        
        # 3. Generation
        prompt = req.message
        reply = generate_response(prompt, system_prompt=system_msg)
        
        return ChatResponse(response=reply, context=context_list)
    except Exception as e:
        print(f"Error in chat endpoint: {e}")
        raise HTTPException(status_code=500, detail=str(e))

from analysis.prediction import SubsidyPredictor
from datetime import date

@app.get("/api/recommendations")
async def get_recommendations():
    """
    Returns predicted subsidy announcements for 2026.
    """
    predictor = SubsidyPredictor()
    
    # Real history verified from K-Startup
    example_history = {
        "초기창업패키지": [date(2024, 1, 30), date(2025, 2, 17)],
        "예비창업패키지": [date(2024, 1, 30), date(2025, 2, 17)],
        "비대면바우처": [date(2024, 5, 10), date(2025, 5, 8)] # Keeping mock for secondary item
    }
    
    predictions = []
    for title, history in example_history.items():
        pred = predictor.predict_next_date(history)
        predictions.append({
            "title": title,
            "predicted_date": pred['predicted_date'],
            "range": f"{pred['range_start']} ~ {pred['range_end']}",
            "confidence": pred['confidence'],
            "reason": pred['reason']
        })
        
    return {"recommendations": predictions}

# --- Helper for Deterministic Mock Data ---
import hashlib

def get_profile_from_seed(seed: str, active_mcps: str = ""):
    """
    Generates deterministic financial profile based on seed (biz_num) and active_mcps.
    active_mcps: comma separated string e.g. "startup,commerce"
    """
    if not seed:
        seed = "default"
    
    if seed == "PRE_FOUNDER":
        return {
            "risk_level": "planning",
            "base_revenue": 150000000 
        }
    
    # Simple hash to number
    h = int(hashlib.md5(seed.encode()).hexdigest(), 16)
    
    # 1. Risk Level
    risk_roll = h % 3
    if risk_roll == 0:
        risk_level = "safe"
    elif risk_roll == 1:
        risk_level = "warning"
    else:
        risk_level = "critical"
        
    # 2. Revenue Scale & Base Calculation
    # If multiple MCPs, take the one with highest revenue potential or average
    mcp_list = active_mcps.split(",") if active_mcps else []
    
    base_revenue = (h % 500) * 1000000 + 50000000 # Default
    
    if "hospital" in mcp_list:
         base_revenue = (h % 500) * 5000000 + 100000000 # High rev
    elif "startup" in mcp_list:
        base_revenue = (h % 100) * 1000000 # Pre-revenue mostly
        
    return {
        "risk_level": risk_level,
        "base_revenue": base_revenue,
        "active_mcps": mcp_list
    }

# --- Endpoints ---

@app.get("/api/dashboard")
async def get_dashboard(biz_num: str = None, active_mcps: str = ""):
    profile = get_profile_from_seed(biz_num, active_mcps)
    risk = profile["risk_level"]
    rev = profile["base_revenue"]
    mcps = profile["active_mcps"]
    
    # Generate Chart Data based on profile
    chart_data = []
    current_rev = rev
    
    if risk == "planning":
        # ... (Pre-founder logic same as before)
        target_rev = rev
        for i in range(1, 13):
            growth = target_rev * (i / 12) * (i / 12) 
            m_rev = int(growth / 12)
            m_exp = int(m_rev * 0.6)
            chart_data.append({"name": f"M+{i}", "income": m_rev, "expense": m_exp})
            
        return {
            "kpi": [
                {"label": "1년차 예상 매출", "value": f"{int(rev):,}원", "trend": "목표", "status": "info"},
                {"label": "법인 전환 유리 시점", "value": "매출 2억↑", "trend": "Guidance", "status": "good"},
                {"label": "초기 세팅 비용", "value": "250만원", "trend": "Est.", "status": "warning"}
            ],
            "chart": chart_data
        }

    # Standard Logic
    current_exp = rev * 0.7 
    
    # Dynamic KPI Builder
    kpi_list = []
    
    # 1. Core CFO KPIs (Always Included)
    if risk == "critical": current_exp = rev * 0.4 
    elif risk == "warning": current_exp = rev * 0.95
    
    kpi_list.append({"label": "예상 납부 세액", "value": f"{int(rev * 0.1):,}원", "trend": "+12.5%", "status": "warning" if risk != "safe" else "good"})
    kpi_list.append({"label": "매출 총이익", "value": f"{int(rev - current_exp):,}원", "trend": "+5.2%", "status": "good"})
    kpi_list.append({"label": "부채 비율", "value": "125%", "trend": "-2.1%", "status": "info"})
    
    # 2. Domain MCP KPIs (Additive)
    if "startup" in mcps:
        # Startup Override: High Expense
        current_exp = rev * 1.5 if risk == "critical" else rev * 1.2
        kpi_list.append({"label": "Monthly Burn Rate", "value": f"{int(current_exp/12):,}원", "trend": "▲ 5%", "status": "warning"})
        kpi_list.append({"label": "Runway", "value": "5.2 Months", "trend": "Critical", "status": "critical" if risk=="critical" else "warning"})
        kpi_list.append({"label": "R&D 세액공제", "value": "가능", "trend": "D-10", "status": "good"})
        
    if "hospital" in mcps:
        kpi_list.append({"label": "이번 달 청구액", "value": f"{int(rev/12):,}원", "trend": "+2%", "status": "good"})
        kpi_list.append({"label": "비급여 비율", "value": "35%", "trend": "적정", "status": "info"})
        kpi_list.append({"label": "재료비 비중", "value": "12%", "trend": "양호", "status": "good"})
        
    if "commerce" in mcps:
        kpi_list.append({"label": "ROAS", "value": "320%", "trend": "-15%", "status": "warning"})
        kpi_list.append({"label": "재고 회전일", "value": "14일", "trend": "Fast", "status": "good"})
        kpi_list.append({"label": "객단가(AOV)", "value": "42,000원", "trend": "+500원", "status": "info"})
        
    # Ensure always valid chart data
    for i in range(1, 13):
        # Add some random variance using hash
        variance = ((h := int(hashlib.md5(f"{biz_num}{i}".encode()).hexdigest(), 16)) % 20 - 10) / 100
        m_rev = int(current_rev * (1 + variance) / 12)
        m_exp = int(current_exp * (1 + variance) / 12)
        chart_data.append({"name": f"{i}월", "income": m_rev, "expense": m_exp})

    return {
        "kpi": kpi_list,
        "chart": chart_data
    }

from crawlers.competition_crawler import CompetitionCrawler

@app.get("/api/competitions")
async def get_competitions():
    """
    Returns list of active competitions (Kaggle/Dacon)
    """
    crawler = CompetitionCrawler()
    return {"competitions": crawler.get_all_competitions()}

# --- Advanced SaaS Features ---

@app.get("/api/analysis/risk")
async def get_tax_risk(biz_num: str = None, active_mcps: str = ""):
    profile = get_profile_from_seed(biz_num, active_mcps)
    level = profile["risk_level"]
    
    # Base penalty logic
    base_penalty = int(profile["base_revenue"] * 0.05) # 5% of revenue as penalty risk
    
    if level == "planning":
        # Pre-Founder Mode
        return {
            "level": "safe", # Reuse safe color scheme or handle 'planning' in frontend
            "score": 95, # High readiness score?
            "title": "예비 창업자 세무 설계 가이드",
            "reason": "사업자 등록 전, 유리한 유형(개인/법인)을 선택하여 절세할 수 있는 골든타임입니다.",
            "estimated_penalty": 1200000, # Positive value here implies SAVING potential in this context
            "action_required": "사업자 유형 결정 필요",
            "action_items": [
                {"task": "간이과세자 vs 일반과세자 유리 불리 비교", "amount": 0, "deadline": "등록 전", "risk_reduction": 0},
                {"task": "청년창업 중소기업 세액감면 대상 확인", "amount": 5000000, "deadline": "필수", "risk_reduction": 0},
                {"task": "초기 사업용 계좌 개설 (분리)", "amount": 0, "deadline": "등록 직후", "risk_reduction": 0}
            ],
            "missing_proofs": [],
            "factors": [
                {"name": "감면 혜택", "status": "good", "value": "대상"},
                {"name": "초기 비용", "status": "warning", "value": "발생 예정"},
                {"name": "법인 전환", "status": "info", "value": "고려"}
            ]
        }
    
    if level == "critical":
        return {
            "level": "critical",
            "score": 85,
            "title": "세무 리스크: 매우 위험",
            "reason": "매출 대비 비용 부족으로 인한 법인세 폭탄 우려",
            "estimated_penalty": base_penalty, # e.g. 15,000,000
            "action_required": "비용 450만원 추가 증빙 시급",
            "action_items": [
                {"task": "미수취 세금계산서 요청 (3건)", "amount": 2500000, "deadline": "D-3", "risk_reduction": 15},
                {"task": "비품(노트북 등) 선구매 및 결제", "amount": 1200000, "deadline": "D-5", "risk_reduction": 8},
                {"task": "접대비(식대) 법인카드 결제 내역 정리", "amount": 800000, "deadline": "이번 주", "risk_reduction": 5}
            ],
            "missing_proofs": [
                {"date": "2026-03-12", "merchant": "(주)오피스허브", "amount": 1200000, "type": "세금계산서 누락"},
                {"date": "2026-03-15", "merchant": "강남식당", "amount": 340000, "type": "현금영수증 미발행"},
                {"date": "2026-03-20", "merchant": "페이스북 광고비", "amount": 890000, "type": "해외결제 증빙 부족"}
            ],
            "factors": [
                {"name": "매출 증가율", "status": "high", "value": "+38%"},
                {"name": "비용 증가율", "status": "low", "value": "+4%"},
                {"name": "적격 증빙 수취율", "status": "warning", "value": "82%"}
            ]
        }
    elif level == "warning":
        penalty = int(base_penalty * 0.3)
        return {
            "level": "warning",
            "score": 60,
            "title": "부가세 매입세액 부족",
            "reason": "업종 평균 대비 매입 세금계산서 부족",
            "estimated_penalty": penalty,
            "action_required": "세금계산서 2건 추가 수취 필요",
            "action_items": [
                {"task": "거래처 (주)한라 통화 및 계산서 요청", "amount": 1500000, "deadline": "D-7", "risk_reduction": 12},
                {"task": "임차료 세금계산서 발행 확인", "amount": 1200000, "deadline": "D-10", "risk_reduction": 10}
            ],
            "missing_proofs": [
                 {"date": "2026-03-05", "merchant": "네이버 서비스", "amount": 55000, "type": "전자세금계산서 미도착"}
            ],
            "factors": [
                {"name": "부가가치율", "status": "warning", "value": "45%"},
                {"name": "매입 증빙", "status": "warning", "value": "부족"},
                {"name": "인건비 신고", "status": "good", "value": "적정"}
            ]
        }
    else: # Safe
        return {
            "level": "safe",
            "score": 25,
            "title": "세무 건전성 양호",
            "reason": "적격 증빙 수취가 매우 양호합니다.",
            "estimated_penalty": 0,
            "action_required": "현재 상태 유지",
            "action_items": [
                {"task": "1분기 부가세 예정신고 검토", "amount": 0, "deadline": "D-25", "risk_reduction": 2}
            ],
             "missing_proofs": [],
            "factors": [
                {"name": "소득율", "status": "good", "value": "15%"},
                {"name": "증빙 수취", "status": "good", "value": "98%"},
                {"name": "적격 증빙", "status": "good", "value": "충분"}
            ]
        }

@app.post("/api/tools/simulator")
async def simulate_tax_saving(req: dict):
    """
    Dynamic tax saving simulator.
    Receives toggles (salary_increase, vehicle_expense, rnd_credit)
    Returns estimated saving amount.
    """
    base_saving = 0
    details = []
    
    if req.get("salary_increase"):
        amount = 1500000
        base_saving += amount
        details.append({"item": "대표자 급여 인상 (비용 처리)", "amount": amount})
        
    if req.get("vehicle_expense"):
        amount = 800000
        base_saving += amount
        details.append({"item": "업무용 승용차 관련 비용", "amount": amount})
        
    if req.get("rnd_credit"):
        amount = 2000000
        base_saving += amount
        details.append({"item": "기업부설연구소 R&D 세액공제", "amount": amount})
        
    return {
        "total_saving": base_saving,
        "details": details,
        "message": "시뮬레이션 결과, 총 예상 절세액입니다."
    }

@app.get("/api/calendar/alerts")
async def get_calendar_alerts():
    """
    Returns upcoming tax deadlines.
    """
    return {
        "alerts": [
            {"date": "2026-01-25", "d_day": 13, "title": "2기 확정 부가세 신고", "type": "mandatory"},
            {"date": "2026-02-10", "d_day": 29, "title": "1월분 원천세 신고/납부", "type": "routine"},
            {"date": "2026-03-31", "d_day": 78, "title": "법인세 신고", "type": "critical"},
        ]
    }

# Serve React App
# Using 'client/dist' - user must build the frontend first
if os.path.exists("client/dist"):
    app.mount("/", StaticFiles(directory="client/dist", html=True), name="static")

if __name__ == "__main__":
    uvicorn.run("main:app", host="0.0.0.0", port=8000, reload=True)
