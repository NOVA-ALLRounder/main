# 복잡한 시나리오 5개

## 시나리오 1: Safari → Calculator → Notes (웹 검색 + 계산 + 메모)
**목표:** 웹에서 정보를 찾아 계산하고 Notes에 저장

**단계:**
1. Safari 열기
2. Google 검색: "Apple stock price"
3. 첫 번째 결과 클릭
4. 주가 숫자를 화면에서 읽기 (Vision)
5. Calculator 열기
6. 읽은 숫자 × 100 계산
7. 결과 복사 (Cmd+C)
8. Notes 열기
9. 새 메모 생성 (Cmd+N)
10. 제목 타이핑: "Apple Stock Calculation"
11. 계산 결과 붙여넣기 (Cmd+V)

**검증 포인트:**
- Safari 네비게이션 작동
- Vision으로 화면 읽기
- Calculator 계산 정확도
- Cmd+C clipboard 복사
- Notes Cmd+N, Cmd+V 메뉴 클릭

---

## 시나리오 2: TextEdit → Mail → Notes (문서 작성 + 이메일 초안 + 메모)
**목표:** 다중 앱 간 텍스트 전달 체인

**단계:**
1. TextEdit 열기
2. Cmd+N (새 문서)
3. 타이핑: "Meeting Summary: Discussed Q1 results and Q2 plans."
4. 전체 선택 (Cmd+A)
5. 복사 (Cmd+C)
6. Mail 앱 열기
7. 새 이메일 작성 (Cmd+N)
8. 본문에 붙여넣기 (Cmd+V)
9. 제목 입력: "Q1 Meeting Notes"
10. 본문 전체 선택 (Cmd+A)
11. 복사 (Cmd+C)
12. Notes 열기
13. 새 메모 (Cmd+N)
14. 붙여넣기 (Cmd+V)

**검증 포인트:**
- TextEdit Cmd+A, Cmd+C 메뉴 클릭
- Mail 앱 Cmd+N, Cmd+V
- 클립보드 체인 전달
- Notes 최종 붙여넣기

---

## 시나리오 3: Finder → Preview → Notes (파일 찾기 + 이미지 복사 + 메모)
**목표:** 파일 시스템 탐색 및 이미지 처리

**단계:**
1. Finder 열기
2. ~/Desktop으로 이동
3. 이미지 파일 검색 (*.png, *.jpg)
4. 첫 번째 이미지 더블클릭 (Preview 열림)
5. 이미지 전체 선택 (Cmd+A)
6. 복사 (Cmd+C)
7. Notes 열기
8. 새 메모 (Cmd+N)
9. 제목 타이핑: "Image Archive"
10. 이미지 붙여넣기 (Cmd+V)
11. 추가 텍스트: "Source: Desktop"

**검증 포인트:**
- Finder 파일 탐색
- Preview 이미지 열기
- 이미지 clipboard 복사
- Notes에 이미지 + 텍스트 혼합

---

## 시나리오 4: Calculator → Safari → TextEdit (계산 + 검색 + 문서화)
**목표:** 역방향 워크플로우 (계산 → 웹 → 문서)

**단계:**
1. Calculator 열기
2. 계산: (365 × 24) = 8760 (연간 시간)
3. 결과 복사 (Cmd+C)
4. Safari 열기
5. Google 검색: "8760 hours in days"
6. 검색 결과 확인
7. Cmd+L (주소창 포커스)
8. 현재 URL 복사 (Cmd+C)
9. TextEdit 열기
10. 새 문서 (Cmd+N)
11. 타이핑: "Total hours per year: "
12. URL 붙여넣기 (Cmd+V)
13. 전체 선택 (Cmd+A)
14. 복사 (Cmd+C)

**검증 포인트:**
- Calculator Cmd+C
- Safari URL 복사
- TextEdit 다중 작업
- Clipboard 덮어쓰기 처리

---

## 시나리오 5: Notes → Safari → Notes → Mail (메모 기반 연구 워크플로우)
**목표:** 가장 복잡한 순환 워크플로우

**단계:**
1. Notes 열기
2. 새 메모 (Cmd+N)
3. 타이핑: "Research Topic: Rust programming language"
4. "Rust programming" 텍스트 선택
5. 복사 (Cmd+C)
6. Safari 열기
7. Google 검색창에 붙여넣기 (Cmd+V)
8. 검색 실행
9. 첫 번째 결과 URL 복사 (Cmd+L → Cmd+C)
10. Notes로 돌아가기 (Cmd+Tab)
11. 기존 메모에 붙여넣기 (Cmd+V)
12. 메모 전체 선택 (Cmd+A)
13. 복사 (Cmd+C)
14. Mail 열기
15. 새 이메일 (Cmd+N)
16. 제목: "Research Findings"
17. 본문 붙여넣기 (Cmd+V)

**검증 포인트:**
- Notes 내부 편집
- Safari 검색 + URL 복사
- Cmd+Tab 앱 전환
- Notes → Safari → Notes → Mail 체인
- 모든 단축키 메뉴 클릭 작동

---

## 실행 명령어

```bash
# 시나리오 1
cargo run --bin local_os_agent -- surf "Safari를 열어 Google에서 'Apple stock price'를 검색하세요. 첫 번째 결과를 클릭하고 주가 숫자를 화면에서 읽으세요. Calculator를 열어 읽은 숫자에 100을 곱하세요. 결과를 복사(Cmd+C)하고 Notes에서 새 메모(Cmd+N)를 만들어 'Apple Stock Calculation'을 입력한 뒤 결과를 붙여넣으세요(Cmd+V)."

# 시나리오 2  
cargo run --bin local_os_agent -- surf "TextEdit을 열어 새 문서(Cmd+N)를 만들고 'Meeting Summary: Discussed Q1 results and Q2 plans.'를 입력하세요. 전체 선택(Cmd+A) 후 복사(Cmd+C)하세요. Mail을 열어 새 이메일(Cmd+N)을 만들고 본문에 붙여넣기(Cmd+V)한 뒤 제목에 'Q1 Meeting Notes'를 입력하세요. 본문 전체 선택(Cmd+A) 후 복사(Cmd+C)하고 Notes로 가서 새 메모(Cmd+N)에 붙여넣으세요(Cmd+V)."

# 시나리오 3
cargo run --bin local_os_agent -- surf "Finder를 열어 ~/Desktop으로 이동하세요. .png 또는 .jpg 이미지를 찾아 첫 번째 파일을 더블클릭해 Preview로 여세요. 이미지 전체 선택(Cmd+A) 후 복사(Cmd+C)하고 Notes에서 새 메모(Cmd+N)를 만들어 제목 'Image Archive'를 입력한 다음 이미지를 붙여넣고(Cmd+V) 다음 줄에 'Source: Desktop'을 입력하세요."

# 시나리오 4
cargo run --bin local_os_agent -- surf "Calculator를 열어 365×24를 계산하세요. 결과를 복사(Cmd+C)하고 Safari를 열어 Google에서 '8760 hours in days'를 검색하세요. 결과를 확인한 뒤 주소창에 포커스(Cmd+L)하고 URL을 복사(Cmd+C)하세요. TextEdit을 열어 새 문서(Cmd+N)를 만든 뒤 'Total hours per year: '를 입력하고 URL을 붙여넣으세요(Cmd+V). 전체 선택(Cmd+A) 후 복사(Cmd+C)하세요."

# 시나리오 5
cargo run --bin local_os_agent -- surf "Notes를 열어 새 메모(Cmd+N)를 만들고 'Research Topic: Rust programming language'를 입력하세요. 'Rust programming' 텍스트를 선택해 복사(Cmd+C)하고 Safari를 열어 Google 검색창에 붙여넣기(Cmd+V) 후 검색하세요. 첫 번째 결과의 URL을 복사(Cmd+L, Cmd+C)하고 Notes로 돌아가 메모에 붙여넣으세요(Cmd+V). 메모 전체 선택(Cmd+A) 후 복사(Cmd+C)하고 Mail에서 새 이메일(Cmd+N)을 만들어 제목 'Research Findings'을 입력한 뒤 본문에 붙여넣으세요(Cmd+V)."
```
