# 🌳 TREES - AI CFO (Tax & Revenue Enhancement Expert System)

스타트업을 위한 **AI 세무/재무 비서 시스템**

## 📋 소개

TREES는 예비창업자부터 성장기업까지, 업종별 맞춤 세무/재무 인사이트를 제공하는 AI 기반 CFO 시스템입니다.

### 핵심 기능

| 기능 | 설명 |
|------|------|
| 📊 **대시보드** | 실시간 재무 현황, KPI, 트렌드 차트 |
| 💬 **AI 상담** | LLM 기반 세무/재무 질의응답 |
| 🎯 **절세 시뮬레이터** | 예상 절세액 계산 |
| 📄 **전자문서 파서** | 국세청 PDF 자동 분석 |
| 🔔 **세무 캘린더** | 신고/납부 마감일 알림 |

### 지원 업종 (MCP)

- **Startup Growth Pack** - R&D 세액공제, 런웨이 관리
- **Medi-Tech Accounting** - 요양급여 청구, 비급여 분석
- **E-Commerce Analytics** - 몰인몰 정산, ROAS 분석

---

## 🚀 시작하기

### 요구사항

- Python 3.10+
- Node.js 18+
- PyMuPDF, FastAPI, React

### 설치

```bash
# 1. 저장소 클론
git clone <repository-url>
cd TaxAccountingAI

# 2. 백엔드 의존성 설치
pip install -r requirements.txt

# 3. 프론트엔드 의존성 설치
cd client
npm install
cd ..
```

### 실행

```bash
# 터미널 1: 백엔드 서버
python main.py

# 터미널 2: 프론트엔드 개발 서버
cd client
npm run dev
```

접속: http://localhost:5173

---

## 📁 프로젝트 구조

```
TaxAccountingAI/
├── main.py              # FastAPI 백엔드 메인
├── nts_parser.py        # 국세청 전자문서 PDF 파서
├── llm.py               # LLM 연동 모듈
├── embedder.py          # RAG 임베딩
├── rag_engine.py        # RAG 엔진
├── vector_store.py      # 벡터 저장소
├── client/              # React 프론트엔드
│   ├── src/
│   │   ├── App.tsx      # 메인 앱 컴포넌트
│   │   └── services/    # API 서비스
│   └── public/
├── sample_pdfs/         # 테스트용 샘플 PDF
│   ├── 연말정산간소화_2025.pdf
│   ├── hospital_companyB/
│   └── commerce_companyC/
└── data/                # RAG 데이터
```

---

## 🔧 API 엔드포인트

### 인증
- `POST /api/auth/register` - 회원가입
- `POST /api/auth/login` - 로그인
- `POST /api/auth/change-password` - 비밀번호 변경

### 대시보드
- `GET /api/dashboard` - KPI 및 차트 데이터
- `GET /api/recommendations` - AI 추천 액션
- `GET /api/calendar/alerts` - 세무 마감일

### 분석
- `GET /api/analysis/risk` - 세무 리스크 분석
- `POST /api/tools/simulator` - 절세 시뮬레이션

### 전자문서
- `POST /api/nts/upload-document` - PDF 업로드 & 파싱
- `GET /api/nts/document-types` - 지원 문서 유형

### AI 채팅
- `POST /api/chat` - LLM 기반 질의응답

---

## 🧪 테스트

### 샘플 PDF 생성

```bash
# 일반 세무 문서 (연말정산, 부가세 등)
python generate_sample_pdfs.py

# 병원용 문서 (요양급여, 진료비 등)
python generate_hospital_samples.py

# 커머스용 문서 (정산, 광고비 등)
python generate_commerce_samples.py
```

### PDF 파서 테스트

```bash
python nts_parser.py sample_pdfs/연말정산간소화_2025.pdf
```

---

## 🐳 Docker 배포

```bash
# 빌드
docker build -t trees-ai-cfo .

# 실행
docker run -p 8000:8000 trees-ai-cfo
```

---

## 📜 라이선스

MIT License

---

## 👥 Team

AI CFO OS - 예비창업자와 스타트업을 위한 지능형 세무/재무 비서
