from fastapi import FastAPI, HTTPException
from fastapi.staticfiles import StaticFiles
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
import os
import sys
import uvicorn
from typing import List, Optional
import json
import hashlib
from datetime import datetime, timedelta
import secrets

# Helper to import llm.py
sys.path.append(os.path.dirname(os.path.abspath(__file__)))
try:
    from llm import generate_response
except ImportError:
    generate_response = lambda prompt, system: "LLM Module not found."

from rag.embedder import Embedder
from rag.vector_store import VectorStore

app = FastAPI()

# ============ AUTH SYSTEM ============
USERS_FILE = os.path.join(os.path.dirname(__file__), "data", "users.json")
SECRET_KEY = "trees-tax-ai-secret-key-2026"

def load_users():
    if os.path.exists(USERS_FILE):
        with open(USERS_FILE, "r", encoding="utf-8") as f:
            return json.load(f)
    return {}

def save_users(users):
    os.makedirs(os.path.dirname(USERS_FILE), exist_ok=True)
    with open(USERS_FILE, "w", encoding="utf-8") as f:
        json.dump(users, f, ensure_ascii=False, indent=2)

def hash_password(password: str) -> str:
    return hashlib.sha256((password + SECRET_KEY).encode()).hexdigest()

def generate_token(user_id: str) -> str:
    return hashlib.sha256(f"{user_id}{SECRET_KEY}{datetime.now().timestamp()}".encode()).hexdigest()

class RegisterRequest(BaseModel):
    email: str
    password: str
    name: str
    company: str

class LoginRequest(BaseModel):
    email: str
    password: str

class AuthResponse(BaseModel):
    success: bool
    token: Optional[str] = None
    user: Optional[dict] = None
    message: str

@app.get("/api/health")
async def health_check():
    return {"status": "ok"}

@app.post("/api/auth/register", response_model=AuthResponse)
async def register(req: RegisterRequest):
    users = load_users()
    
    if req.email in users:
        return AuthResponse(success=False, message="이미 가입된 이메일입니다.")
    
    user_id = hashlib.md5(req.email.encode()).hexdigest()[:8]
    hashed_pw = hash_password(req.password)
    token = generate_token(user_id)
    
    users[req.email] = {
        "id": user_id,
        "email": req.email,
        "password": hashed_pw,
        "name": req.name,
        "company": req.company,
        "created_at": datetime.now().isoformat(),
        "onboarding_completed": False,
        "biz_num": None,
        "type": None
    }
    save_users(users)
    
    user_info = {k: v for k, v in users[req.email].items() if k != "password"}
    return AuthResponse(success=True, token=token, user=user_info, message="회원가입 성공!")

@app.post("/api/auth/login", response_model=AuthResponse)
async def login(req: LoginRequest):
    users = load_users()
    
    if req.email not in users:
        return AuthResponse(success=False, message="등록되지 않은 이메일입니다.")
    
    user = users[req.email]
    if user["password"] != hash_password(req.password):
        return AuthResponse(success=False, message="비밀번호가 일치하지 않습니다.")
    
    token = generate_token(user["id"])
    user_info = {k: v for k, v in user.items() if k != "password"}
    return AuthResponse(success=True, token=token, user=user_info, message="로그인 성공!")

class OnboardingRequest(BaseModel):
    email: str
    biz_num: str
    type: str
    target_revenue: Optional[int] = None

@app.post("/api/auth/complete-onboarding")
async def complete_onboarding(req: OnboardingRequest):
    users = load_users()
    
    if req.email not in users:
        raise HTTPException(status_code=404, detail="사용자를 찾을 수 없습니다.")
    
    users[req.email]["biz_num"] = req.biz_num
    users[req.email]["type"] = req.type
    users[req.email]["target_revenue"] = req.target_revenue
    users[req.email]["onboarding_completed"] = True
    save_users(users)
    
    user_info = {k: v for k, v in users[req.email].items() if k != "password"}
    return {"success": True, "user": user_info, "message": "온보딩 완료!"}

class PasswordChangeRequest(BaseModel):
    email: str
    current_password: str
    new_password: str

@app.post("/api/auth/change-password")
async def change_password(req: PasswordChangeRequest):
    users = load_users()
    
    if req.email not in users:
        raise HTTPException(status_code=404, detail="사용자를 찾을 수 없습니다.")
    
    user = users[req.email]
    if user["password"] != hash_password(req.current_password):
        return {"success": False, "message": "현재 비밀번호가 일치하지 않습니다."}
    
    users[req.email]["password"] = hash_password(req.new_password)
    save_users(users)
    
    return {"success": True, "message": "비밀번호가 변경되었습니다."}

class MCPUpdateRequest(BaseModel):
    email: str
    active_mcps: List[str]

@app.post("/api/auth/update-mcps")
async def update_mcps(req: MCPUpdateRequest):
    users = load_users()
    
    if req.email not in users:
        raise HTTPException(status_code=404, detail="사용자를 찾을 수 없습니다.")
    
    users[req.email]["active_mcps"] = req.active_mcps
    save_users(users)
    
    user_info = {k: v for k, v in users[req.email].items() if k != "password"}
    return {"success": True, "user": user_info, "message": "MCP 설정이 저장되었습니다."}

# ============ END AUTH SYSTEM ============

# ============ NTS ELECTRONIC DOCUMENT API ============
from fastapi import UploadFile, File
import xml.etree.ElementTree as ET
import tempfile
import subprocess

@app.post("/api/nts/upload-document")
async def upload_nts_document(file: UploadFile = File(...), password: str = ""):
    """
    국세청 전자문서(PDF) 업로드 및 XML 데이터 추출
    - Step 1: 전자문서 위변조 검증
    - Step 2: XML 데이터 추출
    - Step 3: 세무 데이터 파싱
    """
    from nts_parser import parse_nts_pdf
    
    if not file.filename.lower().endswith('.pdf'):
        return {"success": False, "message": "PDF 파일만 업로드 가능합니다."}
    
    try:
        # Read file content
        content = await file.read()
        
        # 실제 PDF 파싱 (PyMuPDF 기반)
        result = parse_nts_pdf(pdf_bytes=content, password=password)
        
        if result.get("success"):
            return {
                "success": True,
                "message": "전자문서 검증 및 데이터 추출 성공",
                "verification": {
                    "is_authentic": result["verification"].get("is_nts_document", False),
                    "timestamp": datetime.now().isoformat(),
                    "issuer": "국세청" if result["verification"].get("is_nts_document") else "알 수 없음",
                    "page_count": result["verification"].get("page_count", 0),
                    "has_xml": result["verification"].get("has_xml_data", False)
                },
                "data": result.get("data", {})
            }
        else:
            return {
                "success": False,
                "message": result.get("error", "파싱 실패"),
                "error_code": -1
            }
        
    except Exception as e:
        return {"success": False, "message": f"처리 중 오류 발생: {str(e)}", "error_code": -1}

def parse_nts_xml_mock(filename: str):
    """
    국세청 전자문서 XML 데이터를 파싱하여 구조화된 세무 데이터 반환
    (실제 구현 시 XML 파서로 대체)
    """
    # 파일명에서 문서 유형 추론
    doc_type = "unknown"
    if "연말정산" in filename or "year" in filename.lower():
        doc_type = "year_end_settlement"
    elif "부가세" in filename or "vat" in filename.lower():
        doc_type = "vat_return"
    elif "원천세" in filename:
        doc_type = "withholding_tax"
    elif "소득" in filename:
        doc_type = "income_statement"
    
    # Mock 세무 데이터 생성
    if doc_type == "year_end_settlement":
        return {
            "document_type": "연말정산간소화 자료",
            "tax_year": 2025,
            "items": [
                {"category": "보험료", "amount": 2400000, "deductible": 240000},
                {"category": "의료비", "amount": 1500000, "deductible": 150000},
                {"category": "교육비", "amount": 3600000, "deductible": 540000},
                {"category": "신용카드", "amount": 12000000, "deductible": 1800000},
                {"category": "기부금", "amount": 500000, "deductible": 75000},
            ],
            "total_deductible": 2805000,
            "estimated_refund": 420750
        }
    elif doc_type == "vat_return":
        return {
            "document_type": "부가가치세 신고서",
            "tax_period": "2025년 2기",
            "sales": {"taxable": 50000000, "tax_free": 5000000},
            "purchases": {"taxable": 30000000, "tax_free": 2000000},
            "output_vat": 5000000,
            "input_vat": 3000000,
            "vat_payable": 2000000
        }
    else:
        return {
            "document_type": "기타 세무 문서",
            "raw_extracted": True,
            "message": "상세 파싱 지원 예정"
        }

@app.get("/api/nts/document-types")
async def get_supported_document_types():
    """지원하는 전자문서 유형 목록"""
    return {
        "types": [
            {"code": "year_end", "name": "연말정산간소화 자료", "supported": True},
            {"code": "vat", "name": "부가가치세 신고서", "supported": True},
            {"code": "withholding", "name": "원천징수영수증", "supported": True},
            {"code": "income", "name": "소득금액증명", "supported": True},
            {"code": "business_reg", "name": "사업자등록증명", "supported": False},
            {"code": "tax_payment", "name": "납세증명서", "supported": False},
        ]
    }

# ============ END NTS API ============

# ============ BUSINESS INFO API (공공데이터) ============
from crawlers.business_api import BusinessInfoAPI

business_api = BusinessInfoAPI()

@app.get("/api/business/lookup")
async def lookup_business(biz_num: str):
    """
    사업자등록번호로 사업자 정보 조회
    - API 키 설정 시: data.go.kr 실제 API 사용
    - API 키 없을 시: Mock 데이터 반환
    
    환경변수: DATA_GO_KR_API_KEY
    """
    result = business_api.lookup_business(biz_num)
    return result

# ============ END BUSINESS API ============

# ============ FINANCIAL ANALYSIS API ============
from analysis.financial_analyzer import create_sample_analysis, FinancialAnalyzer, FinancialData

@app.get("/api/financial/analysis")
async def get_financial_analysis(
    revenue: int = 500000000,
    industry: str = "startup",
    operating_margin: float = None,
    debt_ratio: float = None
):
    """
    재무제표 분석 API
    
    매출액과 업종을 기반으로 재무비율 계산, 건전성 점수, 업종 평균 비교, 개선 권고사항 제공
    
    Args:
        revenue: 연간 매출액 (기본: 5억)
        industry: 업종 (startup, hospital, commerce, general)
        operating_margin: 영업이익률 (선택, 0-100)
        debt_ratio: 부채비율 (선택, 0-500)
    """
    result = create_sample_analysis(revenue, industry)
    
    # 사용자가 직접 입력한 비율이 있으면 업데이트
    if operating_margin is not None:
        result["ratios"]["operating_margin"]["value"] = operating_margin
    if debt_ratio is not None:
        result["ratios"]["debt_ratio"]["value"] = debt_ratio
        
    return result

@app.get("/api/financial/ratios")
async def get_financial_ratios(revenue: int = 500000000, industry: str = "startup"):
    """주요 재무비율만 반환"""
    result = create_sample_analysis(revenue, industry)
    return {"ratios": result["ratios"]}

@app.get("/api/financial/health")
async def get_financial_health(revenue: int = 500000000, industry: str = "startup"):
    """재무 건전성 점수만 반환"""
    result = create_sample_analysis(revenue, industry)
    return result["health_score"]

# ============ END FINANCIAL ANALYSIS API ============

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
        
        # 2. Augmentation - Enhanced system prompt for better tax advice
        system_msg = """당신은 한국 세무/회계 전문 AI 어드바이저입니다. 다음 원칙을 따르세요:

## 🎯 핵심 역할
- 사업자를 위한 실질적인 절세 전략 제공
- 복잡한 세법을 쉽게 설명
- 구체적인 액션 아이템 제시

## 💡 절세 핵심 가이드
1. **법인세 절세**: 
   - 대표자 급여 적정화 (월 400-600만원 권장)
   - 업무용 승용차 비용 처리 (연 1,500만원 한도)
   - R&D 세액공제 (최대 25%)
   - 고용증대 세액공제 (청년 1,100만원/일반 700만원)

2. **부가세 절세**:
   - 세금계산서 적시 발급/수취
   - 카드매입 증빙 철저히
   - 매입세액 불공제 항목 확인 (접대비, 비영업용 승용차 등)

3. **원천세**:
   - 4대보험 적정 신고
   - 일용직/프리랜서 구분 정확히
   - 퇴직금 충당금 설정

## 📋 업종별 팁
- **스타트업**: 창업중소기업 세액감면(5년간 50-100%), 벤처인증 시 추가 혜택
- **병의원**: 비급여 매출 적정 신고, 의료기기 감가상각, 인건비 구조 최적화
- **커머스**: 재고자산 평가방법 선택, 광고비 손금처리, 물류비 세액공제

## ⚠️ 주의사항
- 답변 시 관련 법령/조항 언급하면 신뢰도 상승
- 불확실한 경우 "세무사 상담 권장" 표시
- 금액 예시는 대략적 추정임을 명시

---
참고 자료:
""" + context_str
        
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
async def get_recommendations(type: str = "startup"):
    """
    Returns predicted subsidy announcements for 2026 based on business type.
    """
    predictor = SubsidyPredictor()
    
    # Get business type from query param
    biz_type = type
    
    # Business-type specific programs
    subsidy_programs = {
        "startup": {
            "초기창업패키지": [date(2024, 1, 30), date(2025, 2, 17)],
            "예비창업패키지": [date(2024, 1, 30), date(2025, 2, 17)],
            "비대면바우처": [date(2024, 5, 10), date(2025, 5, 8)]
        },
        "hospital": {
            "의료기관 정보화 지원사업": [date(2024, 3, 15), date(2025, 3, 10)],
            "감염관리 시설 개선사업": [date(2024, 4, 1), date(2025, 4, 5)],
            "지역의료기관 경쟁력강화": [date(2024, 2, 20), date(2025, 2, 25)]
        },
        "commerce": {
            "중소기업 온라인수출지원": [date(2024, 3, 1), date(2025, 3, 5)],
            "물류혁신 바우처": [date(2024, 4, 15), date(2025, 4, 10)],
            "이커머스 마케팅지원": [date(2024, 5, 1), date(2025, 5, 5)]
        }
    }
    
    example_history = subsidy_programs.get(biz_type, subsidy_programs["startup"])
    
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
import random
from datetime import datetime as dt

def get_profile_from_seed(seed: str, active_mcps: str = "", target_revenue: int = 150000000):
    """
    Generates financial profile based on seed (biz_num), active_mcps, and target_revenue.
    Now uses target_revenue more directly for realistic data.
    """
    if not seed:
        seed = "default"
    
    mcp_list = active_mcps.split(",") if active_mcps else []
    
    # Use target_revenue as primary driver
    if seed == "PRE_FOUNDER":
        # For pre-founders, use their target revenue directly
        base_revenue = target_revenue if target_revenue else 100000000
        
        # Determine risk based on revenue goal and MCPs
        if base_revenue > 500000000:  # 5억 이상
            risk_level = "warning"  # Higher goal = more risk
        elif base_revenue > 100000000:  # 1억 이상
            risk_level = "planning"
        else:
            risk_level = "safe"
            
        return {
            "risk_level": risk_level,
            "base_revenue": base_revenue,
            "active_mcps": mcp_list
        }
    
    # For existing businesses with real biz_num
    h = int(hashlib.md5(seed.encode()).hexdigest(), 16)
    
    # Use target_revenue if provided, otherwise generate from hash
    if target_revenue and target_revenue > 0:
        base_revenue = target_revenue
    else:
        # Generate based on MCP type
        if "hospital" in mcp_list:
            base_revenue = (h % 800) * 5000000 + 200000000  # 2억 ~ 42억
        elif "startup" in mcp_list:
            base_revenue = (h % 300) * 1000000 + 10000000   # 1천만 ~ 3억
        elif "commerce" in mcp_list:
            base_revenue = (h % 500) * 2000000 + 50000000   # 5천만 ~ 10억
        else:
            base_revenue = (h % 400) * 1000000 + 100000000  # 1억 ~ 5억
    
    # Determine risk level based on revenue scale and hash
    revenue_factor = base_revenue / 100000000  # normalize to 억 단위
    risk_roll = (h + int(revenue_factor * 7)) % 10
    
    if risk_roll < 3:
        risk_level = "safe"
    elif risk_roll < 7:
        risk_level = "warning"
    else:
        risk_level = "critical"
        
    return {
        "risk_level": risk_level,
        "base_revenue": base_revenue,
        "active_mcps": mcp_list
    }

# --- Endpoints ---

@app.get("/api/dashboard")
async def get_dashboard(
    biz_num: str = None, 
    active_mcps: str = "", 
    target_revenue: int = 150000000,
    team_size: int = 0,
    monthly_budget: int = 0
):
    profile = get_profile_from_seed(biz_num, active_mcps, target_revenue)
    risk = profile["risk_level"]
    rev = profile["base_revenue"]
    mcps = profile["active_mcps"]
    
    # RFI-based calculations
    labor_cost = team_size * 5000000 if team_size > 0 else 0  # 인당 월 500만원 평균
    annual_labor = labor_cost * 12
    
    # Generate Chart Data based on profile
    chart_data = []
    current_rev = rev
    
    if risk == "planning" or biz_num == "PRE_FOUNDER":
        # Pre-founder: Use target revenue for realistic projections
        target_rev = rev
        monthly_target = target_rev / 12
        
        for i in range(1, 13):
            # Realistic growth curve (slow start, accelerating)
            growth_factor = (i / 12) ** 1.5  # Slightly exponential
            m_rev = int(monthly_target * growth_factor * (1 + (i * 0.05)))  # 5% monthly acceleration
            m_exp = int(m_rev * (0.7 - (i * 0.02)))  # Expense ratio decreases as scale grows
            chart_data.append({"name": f"M+{i}", "income": m_rev, "expense": max(m_exp, 0)})
        
        # Dynamic KPIs based on target revenue
        estimated_tax = int(target_rev * 0.05)  # 5% estimated tax for new business
        corp_threshold = "매출 2억↑" if target_rev < 200000000 else "즉시 검토"
        setup_cost = min(int(target_rev * 0.02), 5000000)  # 2% of target or max 500만
        
        return {
            "kpi": [
                {"label": "1년차 예상 매출", "value": f"{int(rev):,}원", "trend": "목표", "status": "info"},
                {"label": "예상 절세 가능액", "value": f"{estimated_tax:,}원", "trend": "+5%", "status": "good"},
                {"label": "법인 전환 유리 시점", "value": corp_threshold, "trend": "Guidance", "status": "info" if target_rev < 200000000 else "warning"},
                {"label": "초기 세팅 비용 (예상)", "value": f"{setup_cost:,}원", "trend": "Est.", "status": "warning"}
            ],
            "chart": chart_data
        }

    # Standard Logic
    current_exp = rev * 0.7 
    
    # Dynamic KPI Builder
    kpi_list = []
    
    # 1. Core CFO KPIs (Always Included) - now based on revenue
    if risk == "critical": current_exp = rev * 0.4 
    elif risk == "warning": current_exp = rev * 0.95
    
    tax_rate = 0.1 if rev < 500000000 else 0.15 if rev < 1000000000 else 0.22
    debt_ratio = max(50, min(200, int(150 - (rev / 10000000))))  # 매출 높을수록 부채비율 낮음
    
    kpi_list.append({"label": "예상 납부 세액", "value": f"{int(rev * tax_rate):,}원", "trend": f"+{int(tax_rate*100)}%", "status": "warning" if risk != "safe" else "good"})
    kpi_list.append({"label": "매출 총이익", "value": f"{int(rev - current_exp):,}원", "trend": "+5.2%", "status": "good"})
    kpi_list.append({"label": "부채 비율", "value": f"{debt_ratio}%", "trend": "-2.1%", "status": "info" if debt_ratio < 150 else "warning"})
    
    # 2. Domain MCP KPIs (Additive) - now dynamic based on revenue AND RFI data
    if "startup" in mcps:
        # Use RFI data if available, otherwise estimate
        if labor_cost > 0:
            # RFI-based: 인건비 + 운영비(인건비의 50%)
            burn_rate = labor_cost + int(labor_cost * 0.5)
        else:
            burn_multiplier = 1.5 if risk == "critical" else 1.2
            current_exp = rev * burn_multiplier
            burn_rate = int(current_exp / 12)
        
        # Runway: monthly_budget 있으면 사용, 없으면 매출의 50% 가정
        available_capital = monthly_budget * 12 if monthly_budget > 0 else int(rev * 0.5)
        runway_months = round(available_capital / max(burn_rate, 1), 1)
        
        # RFI 기반 추가 KPI
        if team_size > 0:
            kpi_list.append({"label": "월 인건비 (추정)", "value": f"{labor_cost:,}원", "trend": f"{team_size}명", "status": "info"})
        
        kpi_list.append({"label": "Monthly Burn Rate", "value": f"{burn_rate:,}원", "trend": "▲ 5%", "status": "warning" if burn_rate > rev/10 else "good"})
        kpi_list.append({"label": "Runway", "value": f"{runway_months} Months", "trend": "Critical" if runway_months < 6 else "OK", "status": "critical" if runway_months < 6 else "warning" if runway_months < 12 else "good"})
        kpi_list.append({"label": "R&D 세액공제", "value": f"{int(rev*0.03):,}원", "trend": "가능", "status": "good"})
        
    if "hospital" in mcps:
        monthly_claim = int(rev / 12)
        non_covered_ratio = min(50, max(20, int(30 + (rev / 100000000))))  # 매출 높을수록 비급여 비율 높음
        material_ratio = max(8, min(20, int(15 - (rev / 500000000))))  # 규모 커지면 재료비 비율 낮음
        
        kpi_list.append({"label": "이번 달 청구액", "value": f"{monthly_claim:,}원", "trend": "+2%", "status": "good"})
        kpi_list.append({"label": "비급여 비율", "value": f"{non_covered_ratio}%", "trend": "적정" if non_covered_ratio < 40 else "주의", "status": "info" if non_covered_ratio < 40 else "warning"})
        kpi_list.append({"label": "재료비 비중", "value": f"{material_ratio}%", "trend": "양호", "status": "good" if material_ratio < 15 else "warning"})
        
    if "commerce" in mcps:
        # ROAS: 매출 높을수록 광고 효율 좋다고 가정
        roas = min(500, max(150, int(250 + (rev / 50000000))))
        inventory_days = max(7, min(30, int(20 - (rev / 200000000))))  # 규모 커지면 회전 빠름
        aov = max(20000, min(100000, int(30000 + (rev / 10000000))))  # 매출 높으면 객단가도 높음
        
        kpi_list.append({"label": "ROAS", "value": f"{roas}%", "trend": "+15%" if roas > 300 else "-5%", "status": "good" if roas > 300 else "warning"})
        kpi_list.append({"label": "재고 회전일", "value": f"{inventory_days}일", "trend": "Fast" if inventory_days < 14 else "Normal", "status": "good" if inventory_days < 14 else "info"})
        kpi_list.append({"label": "객단가(AOV)", "value": f"{aov:,}원", "trend": f"+{int(aov*0.01)}원", "status": "good" if aov > 40000 else "info"})
        
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
                {
                    "task": "간이과세자 vs 일반과세자 유리 불리 비교",
                    "amount": 0,
                    "deadline": "등록 전",
                    "risk_reduction": 0,
                    "description": "초기 인테리어/설비 투자가 많다면 부가세 환급이 가능한 **일반과세자**가 유리하며, 초기 비용이 적고 B2C 업종이라면 **간이과세자**가 유리할 수 있습니다 (연 매출 1.04억 미만 시).",
                    "references": [
                        {"title": "국세청: 간이과세자란?", "url": "https://www.nts.go.kr/nts/cm/cntnts/cntntsView.do?mi=2272&cntntsId=7664"},
                        {"title": "찾기쉬운 생활법령", "url": "https://www.easylaw.go.kr/CSP/CnpClsMain.laf?popMenu=ov&csmSeq=679&ccfNo=2&cciNo=1&cnpClsNo=1"}
                    ]
                },
                {
                    "task": "청년창업 중소기업 세액감면 대상 확인",
                    "amount": 5000000,
                    "deadline": "필수",
                    "risk_reduction": 0,
                    "description": "만 15세~34세 이하 청년이 수도권 과밀억제권역 외에서 창업 시 **5년간 법인세/소득세 100% 감면**, 수도권 내 창업 시 50% 감면 혜택이 있습니다. 업종 요건을 반드시 확인하세요.",
                    "references": [
                        {"title": "조세특례제한법 제6조", "url": "https://www.law.go.kr/법령/조세특례제한법/(20240101,19958,20231231)/제6조"},
                        {"title": "K-Startup 가이드", "url": "https://www.k-startup.go.kr/"}
                    ]
                },
                {
                    "task": "초기 사업용 계좌 개설 (분리)",
                    "amount": 0,
                    "deadline": "등록 직후",
                    "risk_reduction": 0,
                    "description": "개인 사용 계좌와 사업용 계좌를 명확히 분리해야 비용 처리가 용이하며, 추후 복식부기 의무자가 되었을 때 가산세 리스크를 피할 수 있습니다.",
                    "references": [
                        {"title": "국세청: 사업용계좌 개설의무", "url": "https://www.nts.go.kr/"}
                    ]
                }
            ],
            "missing_proofs": [],
            "factors": [
                {"name": "감면 혜택", "status": "good", "value": "대상"},
                {"name": "초기 비용", "status": "warning", "value": "발생 예정"},
                {"name": "법인 전환", "status": "info", "value": "고려"}
            ]
        }
    
    if level == "critical":
        # Dynamic amounts based on revenue
        rev = profile["base_revenue"]
        action1_amount = int(rev * 0.01)  # 1% of revenue
        action2_amount = int(rev * 0.005)  # 0.5% of revenue
        action3_amount = int(rev * 0.003)  # 0.3% of revenue
        
        return {
            "level": "critical",
            "score": 85,
            "title": "세무 리스크: 매우 위험",
            "reason": f"매출({int(rev/10000):,}만원) 대비 비용 부족으로 인한 법인세 폭탄 우려",
            "estimated_penalty": base_penalty,
            "action_required": f"비용 {int((action1_amount + action2_amount + action3_amount)/10000):,}만원 추가 증빙 시급",
            "action_items": [
                {"task": "미수취 세금계산서 요청", "amount": action1_amount, "deadline": "D-3", "risk_reduction": 15},
                {"task": "비품(노트북 등) 선구매 및 결제", "amount": action2_amount, "deadline": "D-5", "risk_reduction": 8},
                {"task": "접대비(식대) 법인카드 결제 내역 정리", "amount": action3_amount, "deadline": "이번 주", "risk_reduction": 5}
            ],
            "missing_proofs": [
                {"date": "2026-03-12", "merchant": "(주)오피스허브", "amount": int(rev * 0.004), "type": "세금계산서 누락"},
                {"date": "2026-03-15", "merchant": "강남식당", "amount": int(rev * 0.001), "type": "현금영수증 미발행"},
                {"date": "2026-03-20", "merchant": "페이스북 광고비", "amount": int(rev * 0.003), "type": "해외결제 증빙 부족"}
            ],
            "factors": [
                {"name": "매출 증가율", "status": "high", "value": "+38%"},
                {"name": "비용 증가율", "status": "low", "value": "+4%"},
                {"name": "적격 증빙 수취율", "status": "warning", "value": "82%"}
            ]
        }
    elif level == "warning":
        rev = profile["base_revenue"]
        penalty = int(base_penalty * 0.3)
        action1_amount = int(rev * 0.006)
        action2_amount = int(rev * 0.005)
        
        return {
            "level": "warning",
            "score": 60,
            "title": "부가세 매입세액 부족",
            "reason": f"업종 평균 대비 매입 세금계산서 부족 (매출: {int(rev/10000):,}만원)",
            "estimated_penalty": penalty,
            "action_required": f"세금계산서 {int((action1_amount + action2_amount)/10000):,}만원 추가 수취 필요",
            "action_items": [
                {"task": "거래처 통화 및 계산서 요청", "amount": action1_amount, "deadline": "D-7", "risk_reduction": 12},
                {"task": "임차료 세금계산서 발행 확인", "amount": action2_amount, "deadline": "D-10", "risk_reduction": 10}
            ],
            "missing_proofs": [
                 {"date": "2026-03-05", "merchant": "네이버 서비스", "amount": int(rev * 0.0002), "type": "전자세금계산서 미도착"}
            ],
            "factors": [
                {"name": "부가가치율", "status": "warning", "value": f"{int(40 + (rev/100000000))}%"},
                {"name": "매입 증빙", "status": "warning", "value": "부족"},
                {"name": "인건비 신고", "status": "good", "value": "적정"}
            ]
        }
    else: # Safe
        rev = profile["base_revenue"]
        return {
            "level": "safe",
            "score": 25,
            "title": "세무 건전성 양호",
            "reason": f"적격 증빙 수취가 매우 양호합니다. (매출: {int(rev/10000):,}만원)",
            "estimated_penalty": 0,
            "action_required": "현재 상태 유지",
            "action_items": [
                {"task": "1분기 부가세 예정신고 검토", "amount": 0, "deadline": "D-25", "risk_reduction": 2}
            ],
             "missing_proofs": [],
            "factors": [
                {"name": "소득율", "status": "good", "value": f"{int(10 + (rev/500000000))}%"},
                {"name": "증빙 수취", "status": "good", "value": "98%"},
                {"name": "적격 증빙", "status": "good", "value": "충분"}
            ]
        }

@app.post("/api/tools/simulator")
async def simulate_tax_saving(req: dict):
    """
    Dynamic tax saving simulator.
    Supports startup, hospital, and commerce business types.
    Returns estimated saving amount based on revenue scale.
    한국 세법 기준 정확한 절세 항목 지원.
    """
    base_saving = 0
    details = []
    
    # Get revenue for dynamic calculation (default 150M if not provided)
    revenue = req.get("target_revenue", 150000000)
    
    # === STARTUP ITEMS (스타트업 절세 항목) ===
    if req.get("salary_increase"):
        # 대표자 급여 적정화: 소득세법 제20조
        amount = max(600000, min(int(revenue * 0.05), 7200000))
        base_saving += amount
        details.append({"item": "대표자 급여 적정화 (월 400~600만원)", "amount": amount, "legal": "소득세법 제20조"})
        
    if req.get("vehicle_expense"):
        # 업무용 승용차: 법인세법 시행령 제50조, 연 1,500만원 한도
        amount = min(int(revenue * 0.03), 15000000)
        base_saving += amount
        details.append({"item": "업무용 승용차 비용 (연 1,500만원 한도)", "amount": amount, "legal": "법인세법 시행령 제50조"})
        
    if req.get("rnd_credit"):
        # R&D 세액공제: 조특법 제10조, 중소기업 25%
        amount = max(1500000, min(int(revenue * 0.25), 50000000))
        base_saving += amount
        details.append({"item": "R&D 세액공제 (25% 공제)", "amount": amount, "legal": "조특법 제10조"})
    
    if req.get("startup_deduction"):
        # 창업중소기업 세액감면: 조특법 제6조, 5년간 50~100%
        amount = max(2000000, min(int(revenue * 0.50), 100000000))
        base_saving += amount
        details.append({"item": "창업중소기업 세액감면 (5년간 50~100%)", "amount": amount, "legal": "조특법 제6조"})
    
    if req.get("employment_credit"):
        # 고용증대 세액공제: 조특법 제29조의7, 청년 1,100만원/일반 700만원
        amount = max(700000, min(int(revenue * 0.02), 11000000))
        base_saving += amount
        details.append({"item": "고용증대 세액공제", "amount": amount, "legal": "조특법 제29조의7"})
    
    # === HOSPITAL ITEMS (병원 절세 항목) ===
    if req.get("equipment_depreciation"):
        # 의료장비 가속상각: 법인세법 시행령 제26조
        amount = max(3000000, min(int(revenue * 0.08), 40000000))
        base_saving += amount
        details.append({"item": "의료장비 가속상각 (MRI/CT 등)", "amount": amount, "legal": "법인세법 시행령 제26조"})
        
    if req.get("staff_training"):
        # 직원 교육훈련비: 조특법 제7조, 인건비의 10%
        amount = max(1000000, min(int(revenue * 0.10), 20000000))
        base_saving += amount
        details.append({"item": "직원 교육훈련비 공제", "amount": amount, "legal": "조특법 제7조"})
        
    if req.get("medical_consumables"):
        # 의약품/소모품: 부가가치세법 제38조
        amount = max(800000, min(int(revenue * 0.05), 12000000))
        base_saving += amount
        details.append({"item": "의약품/소모품 매입세액 공제", "amount": amount, "legal": "부가가치세법 제38조"})
    
    if req.get("building_maintenance"):
        # 시설 유지보수비: 법인세법 제23조
        amount = max(500000, min(int(revenue * 0.04), 10000000))
        base_saving += amount
        details.append({"item": "시설 유지보수비 비용처리", "amount": amount, "legal": "법인세법 제23조"})
    
    if req.get("insurance_optimization"):
        # 4대보험 최적화: 고용보험법
        amount = max(400000, min(int(revenue * 0.03), 8000000))
        base_saving += amount
        details.append({"item": "4대보험 최적화 및 지원금 활용", "amount": amount, "legal": "고용보험법"})
    
    # === COMMERCE ITEMS (커머스 절세 항목) ===
    if req.get("inventory_valuation"):
        # 재고자산 평가방법: 법인세법 제42조
        amount = max(600000, min(int(revenue * 0.04), 10000000))
        base_saving += amount
        details.append({"item": "재고자산 평가방법 변경", "amount": amount, "legal": "법인세법 제42조"})
        
    if req.get("ad_expense"):
        # 광고선전비: 법인세법 시행령 제45조
        amount = max(1200000, min(int(revenue * 0.06), 20000000))
        base_saving += amount
        details.append({"item": "광고선전비 비용처리", "amount": amount, "legal": "법인세법 시행령 제45조"})
        
    if req.get("logistics_subsidy"):
        # 물류비 세액공제: 조특법 제25조
        amount = max(500000, min(int(revenue * 0.03), 8000000))
        base_saving += amount
        details.append({"item": "물류비 세액공제 (스마트 물류)", "amount": amount, "legal": "조특법 제25조"})
    
    if req.get("platform_fee"):
        # 마켓플레이스 수수료: 법인세법 제19조
        amount = max(800000, min(int(revenue * 0.05), 15000000))
        base_saving += amount
        details.append({"item": "마켓플레이스 수수료 비용처리", "amount": amount, "legal": "법인세법 제19조"})
    
    if req.get("export_credit"):
        # 수출 세액공제: 조특법 제22조
        amount = max(700000, min(int(revenue * 0.04), 12000000))
        base_saving += amount
        details.append({"item": "수출 세액공제 (해외판매)", "amount": amount, "legal": "조특법 제22조"})
    
    # === 레거시 지원 (기존 키 호환) ===
    if req.get("nonprofit_reserve"):
        amount = max(1000000, min(int(revenue * 0.02), 15000000))
        base_saving += amount
        details.append({"item": "고유목적사업준비금 적립", "amount": amount, "legal": "법인세법 제29조"})
    
    if req.get("logistics_cost"):
        amount = max(800000, min(int(revenue * 0.015), 10000000))
        base_saving += amount
        details.append({"item": "물류비 세액공제 (중소기업)", "amount": amount, "legal": "조특법 제25조"})
        
    return {
        "total_saving": base_saving,
        "details": details,
        "message": f"연 매출 {int(revenue/10000):,}만원 기준 예상 절세액입니다."
    }

@app.get("/api/calendar/alerts")
async def get_calendar_alerts():
    """
    Returns upcoming tax deadlines with dynamic D-day calculation.
    """
    from datetime import datetime, date
    today = date.today()
    current_year = today.year
    current_month = today.month
    
    # Calculate key tax dates for current/next period
    alerts = []
    
    # 1. 부가세 신고 (1월 25일, 7월 25일)
    vat_month = 1 if current_month <= 1 else 7 if current_month <= 7 else 1
    vat_year = current_year if vat_month >= current_month else current_year + 1
    vat_date = date(vat_year, vat_month, 25)
    vat_d_day = (vat_date - today).days
    if vat_d_day > 0:
        alerts.append({
            "date": vat_date.isoformat(),
            "d_day": vat_d_day,
            "title": f"{vat_month == 1 and '2기 확정' or '1기 확정'} 부가세 신고",
            "type": "mandatory"
        })
    
    # 2. 원천세 (매월 10일)
    wt_month = current_month if today.day < 10 else (current_month % 12) + 1
    wt_year = current_year if wt_month >= current_month else current_year + 1
    wt_date = date(wt_year, wt_month, 10)
    wt_d_day = (wt_date - today).days
    if wt_d_day > 0:
        alerts.append({
            "date": wt_date.isoformat(),
            "d_day": wt_d_day,
            "title": f"{wt_month-1 if wt_month > 1 else 12}월분 원천세 신고/납부",
            "type": "routine"
        })
    
    # 3. 법인세 (3월 31일)
    corp_year = current_year if current_month <= 3 or (current_month == 3 and today.day <= 31) else current_year + 1
    corp_date = date(corp_year, 3, 31)
    corp_d_day = (corp_date - today).days
    if corp_d_day > 0 and corp_d_day <= 120:
        alerts.append({
            "date": corp_date.isoformat(),
            "d_day": corp_d_day,
            "title": "법인세 신고",
            "type": "critical"
        })
    
    # 4. 종합소득세 (5월 31일)
    income_year = current_year if current_month <= 5 else current_year + 1
    income_date = date(income_year, 5, 31)
    income_d_day = (income_date - today).days
    if income_d_day > 0 and income_d_day <= 150:
        alerts.append({
            "date": income_date.isoformat(),
            "d_day": income_d_day,
            "title": "종합소득세 신고",
            "type": "critical"
        })
    
    # Sort by d_day
    alerts.sort(key=lambda x: x["d_day"])
    
    return {"alerts": alerts[:5]}  # Return top 5 upcoming

# Serve React App
# Using 'client/dist' - user must build the frontend first
if os.path.exists("client/dist"):
    app.mount("/", StaticFiles(directory="client/dist", html=True), name="static")

if __name__ == "__main__":
    uvicorn.run("main:app", host="0.0.0.0", port=8000, reload=True)
