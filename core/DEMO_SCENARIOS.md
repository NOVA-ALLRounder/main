# 🎬 Steer 심화 데모 시나리오 (Advanced Workflows)

단순 앱 실행을 넘어, **API 연동, UI 자동화, 쉘 제어**가 결합된 엔터프라이즈급 복합 시나리오 모음입니다.
Steer의 **하이브리드 인텔리전스(기억+계획)** 능력을 보여주기에 최적화되었습니다.

---

## 1. 🌅 모닝 브리핑 (Morning Briefing)
> **흐름:** Gmail API ➡️ LLM 요약 ➡️ Notion API
> **가치:** 반복적인 아침 정보 수집 업무를 0초로 단축.

- **명령어:**
  ```text
  surf 지난 밤에 온 '긴급(Urgent)' 태그 메일 확인해서 내용을 요약한 다음, 노션 'Daily Standup' 페이지에 정리해줘
  ```
- **작동 원리 (Logic):**
  1. **Gmail Integration**: `gmail.rs` 모듈이 읽지 않은 메일 중 필터링.
  2. **Thinking**: 메일 본문을 LLM 컨텍스트에 로드하여 요약문 생성.
  3. **Notion Integration**: `notion.rs`를 통해 지정된 Page ID에 블록 추가 (Append).

---

## 2. 💰 지출 결의서 작성 (Expense Report)
> **흐름:** Notion 데이터 ➡️ Excel/Numbers UI 제어 ➡️ Email 발송
> **가치:** 데이터 조회(API)와 레거시 툴(Excel) 입력을 연결 (RPA).

- **명령어:**
  ```text
  surf 노션 '2월 지출' DB에서 승인된 내역 확인하고, 엑셀 켜서 테이블로 정리한 뒤 회계팀장에게 메일로 보내
  ```
- **작동 원리:**
  1. **Notion API**: DB 쿼리로 'Approved' 상태 항목 추출.
  2. **Visual Automation (Surf)**: 엑셀 앱 실행 -> 셀 좌표 인식 -> 데이터 타이핑 (`Vision-to-Action`).
  3. **File Handling**: 저장된 엑셀 파일 경로 확보.
  4. **Gmail API**: 해당 파일 첨부하여 메일 발송.

---

## 3. 🔍 경쟁사 동향 리서치 (Market Research)
> **흐름:** Safari 웹 검색 ➡️ 스크린샷/텍스트 추출 ➡️ Keynote/Slack 공유
> **가치:** 웹 정보 탐색과 업무 툴 간의 장벽 제거.

- **명령어:**
  ```text
  surf 사파리로 'OpenAI 최신 뉴스' 검색해서 상위 3개 기사 캡처하고, 슬랙 #news 채널에 요약과 함께 공유해줘
  ```
- **작동 원리:**
  1. **Browser Automation**: 브라우저 실행 및 검색어 입력.
  2. **Visual Cortex**: 검색 결과 화면 캡처 및 OCR/Vision 분석.
  3. **App Control**: 슬랙 앱 실행(또는 API) -> 채널 찾기 -> 이미지 및 텍스트 업로드.

---

## 4. 🚨 서버 장애 대응 (DevOps Incident)
> **흐름:** Shell 명령어 ➡️ 에러 감지 ➡️ Jira 티켓 생성
> **가치:** 개발자의 터미널 작업과 이슈 트래킹 자동화.

- **명령어:**
  ```text
  surf 로컬 서버 로그(tail -n 100 error.log) 확인해서 'Panic' 에러 있으면 지라(Jira)에 버그 티켓 생성해줘
  ```
- **작동 원리:**
  1. **Shell Executor**: `executor.rs`가 안전하게 로그 파일 읽기 수행.
  2. **Pattern Detector**: 'Panic', 'Error' 키워드 감지.
  3. **Web Automation**: 사파리로 Jira 접속 -> 로그인(세션 유지) -> 'Create Issue' 버튼 클릭 -> 로그 붙여넣기.

---

## 💡 시연 팁 (Pro Tips)
- **복합 명령의 처리:** 에이전트는 긴 명령을 받으면 스스로 **하위 작업(Sub-tasks)**으로 분리하여 순차 실행합니다. "A하고 B해줘"라고 명령했을 때 로그에서 `Planning Step 1... Done. Planning Step 2...`로 넘어가는 과정을 강조하세요.
- **실패 시 복구:** 엑셀 입력 중 오타가 나거나 UI가 꼬일 때, 에이전트가 백스페이스를 누르거나 ESC를 눌러 **스스로 복구(Self-Healing)**하는 모습은 오히려 데모의 신뢰도를 높여줍니다.
