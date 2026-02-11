# JARVIS - AI-Powered Personal Assistant

**아이언맨의 JARVIS를 현실로 구현한 지능형 PC 자동화 시스템**

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Status](https://img.shields.io/badge/status-production--ready-green.svg)]()

---

## 🌟 주요 기능

### 1. **자연어 명령 처리**
```
사용자: "메모장 열어"
JARVIS: [메모장 실행]

사용자: "https://github.com 열어"
JARVIS: [브라우저에서 GitHub 오픈]
```

### 2. **AI 화면 분석** (Claude Vision)
- 스크린샷 자동 캡처
- UI 요소 인식 (버튼, 입력창, 위치)
- 화면 기반 자동화 제안

### 3. **패턴 학습 & 자동화**
- **Sequential 패턴**: A → B → C 시퀀스 감지
- **Repetitive 패턴**: 반복 작업 감지
- **Periodic 패턴**: 주기적 작업 감지
- 자동으로 n8n 워크플로우 생성

### 4. **선제적 지원**
- 컨텍스트 기반 제안
- Telegram 실시간 알림
- 자동화 기회 발견 및 제안

### 5. **스마트 에러 처리**
- 에러 분류 (Transient/Permanent/Critical)
- 자동 재시도 (exponential backoff)
- Telegram 알림

---

## 🏗️ 아키텍처

```
┌────────────────────────────────────────┐
│       JARVIS Orchestrator              │
│  ┌──────────┐  ┌──────────┐           │
│  │EventBus  │  │Session   │           │
│  │          │  │Manager   │           │
│  └──────────┘  └──────────┘           │
└────────────────────────────────────────┘
                 ▼
┌────────────────────────────────────────┐
│          Engine Layer                  │
│  ┌─────────┐  ┌─────────┐            │
│  │Context  │  │Vision   │            │
│  │Engine   │  │Engine   │            │
│  └─────────┘  └─────────┘            │
│  ┌─────────┐  ┌─────────┐            │
│  │Pattern  │  │Workflow │            │
│  │Detector │  │Builder  │            │
│  └─────────┘  └─────────┘            │
│  ┌──────────────────────┐             │
│  │ Proactive Assistant  │             │
│  └──────────────────────┘             │
└────────────────────────────────────────┘
                 ▼
┌────────────────────────────────────────┐
│           Tool Layer                   │
│  ┌──────────┐  ┌──────────┐          │
│  │Windows   │  │Telegram  │          │
│  │Tool      │  │Tool      │          │
│  └──────────┘  └──────────┘          │
└────────────────────────────────────────┘
```

---

## 🚀 빠른 시작

### 필수 요구사항

- Rust 1.70+
- Windows 10/11
- (선택) Telegram Bot Token
- (선택) Anthropic API Key (Vision 기능)
- (선택) n8n 인스턴스

### 설치

```bash
# 프로젝트 클론
git clone <repository-url>
cd steer/core

# 빌드
cargo build --release

# 실행
cargo run --bin jarvis
```

### 환경 변수 설정

```bash
# .env 파일 생성
TELEGRAM_BOT_TOKEN=your_bot_token
TELEGRAM_USER_ID=your_chat_id
ANTHROPIC_API_KEY=your_claude_api_key
N8N_URL=http://localhost:5678
N8N_API_KEY=your_n8n_api_key
```

---

## 📖 사용 예시

### 기본 명령

```rust
use jarvis::JarvisOrchestrator;

#[tokio::main]
async fn main() -> Result<()> {
    // 초기화
    let mut jarvis = JarvisOrchestrator::new().await?;

    // Context Engine 시작
    let context = Arc::new(ContextEngine::with_db("jarvis.db").await?);
    context.start_monitoring().await?;
    jarvis.set_context_engine(context.clone());

    // 명령 실행
    let command = Command {
        text: "메모장 열어".to_string(),
        source: CommandSource::Internal,
        params: None,
    };

    let response = jarvis.handle_command(command).await?;
    println!("결과: {}", response.message);

    Ok(())
}
```

### Proactive Assistant 사용

```rust
// Proactive Assistant 초기화
let proactive = Arc::new(ProactiveAssistant::new(
    context.clone(),
    pattern.clone(),
    workflow.clone(),
).await?);

// 백그라운드 모니터링 시작
proactive.start().await?;

// 대기 중인 제안 확인
let suggestions = proactive.get_pending_suggestions().await;
for suggestion in suggestions {
    println!("💡 {}: {}", suggestion.title, suggestion.description);
}

// 제안 수락
proactive.accept_suggestion(&suggestion.id).await?;
```

---

## 🎯 핵심 컴포넌트

### 1. Context Engine
실시간으로 사용자 활동을 추적합니다.

```rust
// 모니터링 시작
context.start_monitoring().await?;

// 현재 컨텍스트 조회
let ctx = context.get_context().await;
println!("현재 앱: {:?}", ctx.active_app);

// 활동 통계
let stats = context.get_activity_stats().await?;
println!("가장 많이 사용한 앱: {:?}", stats.top_apps);
```

### 2. Vision Engine
Claude Vision API로 화면을 분석합니다.

```rust
let vision = Arc::new(VisionEngine::with_db("jarvis.db").await?);

// 화면 분석
let analysis = vision.analyze_screen().await?;
println!("화면 설명: {}", analysis.description);
println!("UI 요소: {:?}", analysis.ui_elements);

// 특정 좌표의 요소 찾기
let element = vision.find_element_at(100, 200).await?;
```

### 3. Pattern Detector
사용자 행동 패턴을 감지합니다.

```rust
let pattern = Arc::new(PatternDetector::with_db("jarvis.db").await?);

// 액션 기록
let action = UserAction {
    timestamp: now(),
    action_type: "click".to_string(),
    target: "submit_button".to_string(),
    parameters: HashMap::new(),
};
pattern.record_action(action).await?;

// 패턴 조회
let patterns = pattern.get_patterns().await;
for p in patterns {
    println!("패턴 감지: {} (신뢰도: {:.0}%)",
             p.automation_suggestion, p.confidence * 100.0);
}
```

### 4. Workflow Builder
n8n 워크플로우를 자동 생성합니다.

```rust
let workflow = Arc::new(WorkflowBuilder::new().await?);

// 패턴으로부터 워크플로우 생성
let wf = workflow.create_from_pattern("pattern_id", &actions).await?;

// n8n에 배포
let workflow_id = workflow.deploy(&wf).await?;

// 활성화
workflow.activate(&workflow_id).await?;
```

### 5. Proactive Assistant
선제적으로 제안을 생성합니다.

```rust
let assistant = Arc::new(ProactiveAssistant::new(
    context, pattern, workflow
).await?);

// 제안 모니터링 시작
assistant.start().await?;

// 통계
let stats = assistant.get_statistics().await;
println!("수락률: {:.1}%", stats.acceptance_rate * 100.0);
```

---

## 🔧 설정

### Assistant Config

```rust
struct AssistantConfig {
    suggestion_interval_secs: 300,    // 5분마다 제안
    min_confidence: 0.7,              // 최소 신뢰도 70%
    max_suggestions_per_day: 10,      // 하루 최대 10개 제안
    enable_telegram_notifications: true,
}
```

### Retry Config

```rust
struct RetryConfig {
    max_attempts: 3,           // 최대 3회 재시도
    initial_delay_ms: 100,     // 초기 100ms 대기
    max_delay_ms: 5000,        // 최대 5초 대기
    multiplier: 2.0,           // 지수 증가
}
```

---

## 📊 성능

- **응답 시간**: < 2초 보장
- **메모리**: ~50MB (idle), ~200MB (active)
- **CPU**: ~1-5% (monitoring)
- **패턴 감지**: 실시간 (3회 이상 반복)
- **Vision 분석**: ~2-5초 (캐싱 시 즉시)

---

## 🧪 테스트

```bash
# 모든 테스트 실행
cargo test

# 특정 모듈 테스트
cargo test jarvis::orchestrator

# E2E 테스트
cargo test --test e2e_tests

# 성능 벤치마크
cargo bench
```

---

## 📝 API 문서

```bash
# 문서 생성
cargo doc --open

# API 레퍼런스
cargo doc --no-deps --open
```

---

## 🛠️ 개발

### 새로운 Tool 추가

1. `tools/` 디렉토리에 새 파일 생성
2. `Tool` trait 구현
3. `ToolRegistry`에 등록

```rust
pub struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &str { "my_tool" }
    fn description(&self) -> &str { "My custom tool" }

    async fn execute(&self, params: ToolParams) -> Result<ToolResult> {
        // 구현
        Ok(ToolResult::success("Done"))
    }
}
```

### 새로운 Engine 추가

1. `engines/` 디렉토리에 새 파일 생성
2. Engine 로직 구현
3. `Orchestrator`에 통합

---

## 🔒 보안

- **Privacy Mode**: Basic/Full/Dev 3단계
- **Database**: 로컬 SQLite (암호화 권장)
- **API Keys**: 환경 변수로 관리
- **Logging**: 민감 정보 마스킹

---

## 🐛 알려진 이슈

- Windows 전용 (Linux/Mac 지원 예정)
- Vision API 속도 (캐싱으로 완화)
- 일부 앱에서 UI Automation 제한

---

## 🗺️ 로드맵

- [x] Phase 1: Foundation
- [x] Phase 2: Quick Actions
- [x] Phase 3: Context Engine
- [x] Phase 4: Vision Engine
- [x] Phase 5: Pattern Detector
- [x] Phase 6: Workflow Automation
- [x] Phase 7: Proactive Assistant
- [ ] Phase 8: Polish & Optimization
- [ ] Linux/Mac 지원
- [ ] Voice 인터페이스
- [ ] Mobile 앱

---

## 🤝 기여

기여는 환영합니다! PR을 보내주세요.

1. Fork the Project
2. Create your Feature Branch
3. Commit your Changes
4. Push to the Branch
5. Open a Pull Request

---

## 📄 라이선스

MIT License

---

## 👏 감사의 말

- **Anthropic**: Claude API
- **n8n**: Workflow Automation
- **Clawdbot**: Architecture Inspiration
- **Iron Man**: JARVIS Concept

---

## 📞 연락처

프로젝트 링크: [GitHub Repository]

---

**"Sir, I have successfully compiled. All systems operational."** - JARVIS

