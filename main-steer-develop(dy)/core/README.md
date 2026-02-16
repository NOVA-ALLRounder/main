# Steer: 로컬 OS 에이전트 (Rust 네이티브)

🚀 **MacOS 데스크톱 자동화를 위한 자율 AI 에이전트**

Steer는 사용자의 로컬 MacOS 환경을 직접 인식하고, 계획하며, 제어하는 고성능 Rust 기반 에이전트입니다. LLM의 지능과 저수준(Low-level) OS 제어 능력을 연결하는 "디지털 신경계(Digital Nervous System)" 역할을 수행합니다.

---

## 🎯 할 수 있는 것 (Capabilities)

Steer는 단순 자동화를 넘어 OS 전반에 걸친 통합 제어를 제공합니다.

| 기능 | 예시 | 비고 |
|------|------|------|
| **UI 자동화** | "Safari 열어서 최신 뉴스 검색해줘" | Vision-to-Action (Surf) |
| **파일 처리** | "Downloads 폴더의 PDF 내용 요약해줘" | RAG / Text Processing |
| **쉘 실행** | "git status 확인하고 커밋해줘" | Safe Shell Executor |
| **원격 제어** | (텔레그램) "지금 컴퓨터 켜져있어?" | Telegram Bot 연동 |
| **외부 연동** | "내일 일정 캘린더에 추가하고 노션에 백업해" | Gmail / Notion / Calendar API |

---

## 🔁 심화 워크플로우 (Advanced Scenarios)

단일 명령으로 여러 앱과 API를 연결하는 복합 작업이 가능합니다.

### 시나리오 1: 데일리 업무 보고 자동화
> **명령:** "노션의 '오늘의 할일' 확인해서 완료된 항목 엑셀에 정리하고 팀장님께 메일로 보내줘"
- **작동 방식:**
  1. `Notion API`: 페이지 조회 및 'Done' 상태 필터링.
  2. `UI Automation`: Excel 실행 -> 데이터 시각적 입력.
  3. `Gmail API`: 엑셀 파일 첨부하여 보고서 메일 발송.

### 시나리오 2: 업무 브리핑 및 정리
> **명령:** "지난 밤에 온 업무 메일 확인해서 요약한 다음 노션 '주간 리포트'에 추가해줘"
- **작동 방식:**
  1. `Gmail API`: 읽지 않은 업무 관련 메일 수신 (`search: "work"`).
  2. `LLM Brain`: 메일 내용 요약 및 핵심 안건 도출.
  3. `Notion API`: 지정된 데이터베이스에 요약 내용(Bullet point) 자동 생성.

---

## 🌟 핵심 기능 (Key Features)

### 1. **다이내믹 비전 자동화 ("Surf")**
에이전트가 사람처럼 화면을 "보고" 앱과 상호작용합니다.
- **Visual Cortex (시각 피질)**: `CoreGraphics`를 활용한 저지연 스크린샷 캡처 및 최적화 리사이징 (Base64).
- **Vision-to-Action**: 멀티모달 LLM(Gemini/GPT-4o)을 사용하여 UI 요소의 좌표를 추론하고 즉각적인 동작(클릭, 타이핑, 단축키) 수행.
- **Self-Healing (자가 치유)**: UI 변경이나 팝업 등으로 동작 실패 시, `Combat Protocol`이 발동하여 ESC/Enter 등을 시도하며 스스로 복구.

### 2. **하이브리드 인텔리전스 엔진**
비용, 속도, 프라이버시를 최적화하기 위해 작업을 지능적으로 라우팅합니다.
- **Primary (High-Fidelity)**: 복잡한 추론 및 계획 수립을 위한 **OpenAI GPT-4o**.
- **Fallback (Vision Safety)**: OpenAI Safety Filter 발동 시, 제약 없는 비전 작업을 위해 **Gemini CLI** (Argument Mode/Sandbox)로 자동 전환.
- **Local (Privacy)**: 단순 텍스트 처리 및 프라이버시 보호를 위한 **Ollama (Llama 3)** [Planned].

### 3. **Rust 코어 성능**
속도와 안전성을 위해 설계되었습니다.
- **Event Tap**: `CoreFoundation`을 사용하여 시스템 레벨의 키보드/마우스 이벤트를 캡처하고 제어.
- **Memory Safety**: Rust의 소유권 모델(Ownership)을 통해 메모리 누수 및 충돌 방지.
- **Async Runtime**: `Tokio` 프레임워크 기반으로 동시다발적인 I/O(API 호출, 쉘 실행, 화면 캡처)를 논블로킹으로 처리.

---

## 🛠️ 기술적 심층 분석 (Technical Deep Dive)

### 시스템 아키텍처

```mermaid
graph TD
    User[사용자 명령] --> API[API Server (Axum / Port: 5680)]
    API --> Controller[Dynamic Controller]
    
    subgraph Brain [지능 계층 (Intelligence)]
        Controller --> LLM[LLM Gateway]
        LLM --"REST"--> OpenAI[OpenAI API]
        LLM --"Process"--> Gemini[Gemini CLI (Fallback)]
        LLM --"Local"--> Ollama[Local LLM]
    end
    
    subgraph Body [실행 계층 (Execution)]
        Controller --> Vision[Visual Driver]
        Vision --"CGDisplay"--> Screen[Screen Capture]
        Vision --"CGEvent"--> Mouse[Mouse/Keyboard Control]
        Controller --> Shell[Shell Executor]
        Controller --> Integrations[Notion/Gmail/Cal API]
    end
    
    Vision --"Feedback (Image Frame)"--> LLM
```

### 주요 모듈 설명
- **`dynamic_controller.rs`**: 에이전트의 메인 루프. 관찰(Observe) -> 계획(Plan) -> 실행(Act) 사이클을 관리합니다.
- **`cli_llm.rs`**: 로컬 CLI(Gemini 등)와의 인터페이스. 샌드박스모드 실행 및 프로세스 제어를 담당합니다.
- **`visual_driver.rs`**: MacOS 네이티브 API(`CoreGraphics`, `Accessibility`)를 래핑하여 화면 인식/제어를 수행합니다.
- **`integrations/`**: Notion, Gmail, Calendar 등 외부 서비스와의 API 통신 모듈.

---

## 🚀 시작하기 (Getting Started)

### 필수 조건 (Prerequisites)
1. **MacOS**: 손쉬운 사용(Accessibility) 및 화면 기록(Screen Recording) 권한 필요.
2. **Rust Toolchain**: 최신 `cargo` 설치 (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`).
3. **Gemini CLI**: 버전 `v0.22.5` 이상 (Vision Fallback용).

### 설치 및 실행 가이드

1. **저장소 클론 및 이동**
   ```bash
   git clone <repo-url>
   cd Steer/local-os-agent/core
   ```

2. **환경 변수 설정 (`.env`)**
   ```bash
   # 필수 (Primary Brain)
   OPENAI_API_KEY=sk-proj-...
   
   # 필수 (Vision Fallback)
   STEER_CLI_LLM=gemini 
   
   # 선택 (Integrations)
   NOTION_API_KEY=secret_...
   GMAIL_CREDENTIALS_PATH=./credentials.json
   ```

3. **빌드 및 실행**
   ```bash
   # Release 모드로 빌드 및 실행 (최적화)
   cargo run --release --bin local_os_agent
   ```

4. **사용법 (Commands)**
   실행 후 인터랙티브 쉘에 명령 입력:
   - `surf <목표>`: 화면을 보며 자율 수행 (예: `surf 사파리 열어서 뉴스 보여줘`)
   - `help`: 사용 가능한 명령어 목록 확인

---

## ❓ 트러블슈팅 (Troubleshooting)

**Q: "The application is not allowed to capture the screen" 오류가 뜹니다.**
A: 시스템 설정 > 개인정보 보호 및 보안 > 화면 기록에서 터미널(iTerm/Terminal) 권한을 허용해주세요.

**Q: Gemini CLI가 응답하지 않습니다.**
A: `gemini --version`으로 설치 여부를 확인하고, `.env`에 `STEER_CLI_LLM=gemini`가 있는지 확인하세요.

**Q: 한글 입력이 깨집니다.**
A: MacOS 입력 소스가 '한글' 상태인지 확인하세요. 에이전트는 현재 시스템의 입력 소스를 그대로 사용합니다.

---

## 📄 라이선스 & 보안
- **License**: Internal Team Use Only.
- **Security**: 모든 외부 프로세스 실행은 `Sandbox` 내에서 이루어지며, 중요 데이터는 마스킹 처리됩니다.
