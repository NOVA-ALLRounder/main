# 복잡한 시나리오 5개 (Windows 버전)

## 시나리오 1: Edge → Calculator → 메모장 (웹 검색 + 계산 + 메모)
**목표:** 웹에서 정보를 찾아 계산하고 메모장에 저장

**단계:**
1. Edge 열기
2. Google 검색: "Apple stock price"
3. 첫 번째 결과 클릭
4. 주가 숫자를 화면에서 읽기 (Vision)
5. Calculator 열기
6. 읽은 숫자 × 100 계산
7. 결과 복사 (Ctrl+C)
8. 메모장 열기
9. 새 파일 생성 (Ctrl+N)
10. 제목 타이핑: "Apple Stock Calculation"
11. 계산 결과 붙여넣기 (Ctrl+V)

**검증 포인트:**
- Edge 네비게이션 작동
- Vision으로 화면 읽기
- Calculator 계산 정확도
- Ctrl+C clipboard 복사
- 메모장 Ctrl+N, Ctrl+V 동작

---

## 시나리오 2: 메모장 → 메일(Outlook) → 메모장 (문서 작성 + 이메일 초안 + 메모)
**목표:** 다중 앱 간 텍스트 전달 체인

**단계:**
1. 메모장 열기
2. Ctrl+N (새 문서)
3. 타이핑: "Meeting Summary: Discussed Q1 results and Q2 plans."
4. 전체 선택 (Ctrl+A)
5. 복사 (Ctrl+C)
6. 메일 앱/Outlook 열기
7. 새 이메일 작성 (Ctrl+N)
8. 본문에 붙여넣기 (Ctrl+V)
9. 제목 입력: "Q1 Meeting Notes"
10. 본문 전체 선택 (Ctrl+A)
11. 복사 (Ctrl+C)
12. 메모장 열기
13. 새 문서 (Ctrl+N)
14. 붙여넣기 (Ctrl+V)

**검증 포인트:**
- 메모장 Ctrl+A, Ctrl+C
- Outlook/메일 앱 Ctrl+N, Ctrl+V
- 클립보드 체인 전달
- 메모장 최종 붙여넣기

---

## 시나리오 3: 파일 탐색기 → 사진 → 메모장 (파일 찾기 + 이미지 처리 + 메모)
**목표:** 파일 시스템 탐색 및 이미지 처리

**단계:**
1. 파일 탐색기 열기
2. `C:\Users\<사용자>\Desktop`으로 이동
3. 이미지 파일 검색 (*.png, *.jpg)
4. 첫 번째 이미지 더블클릭 (사진 앱 열림)
5. 이미지 복사 (Ctrl+C)
6. 메모장 열기
7. 새 문서 (Ctrl+N)
8. 제목 타이핑: "Image Archive"
9. 메모장에 텍스트 추가: "Source: Desktop"

**검증 포인트:**
- 파일 탐색기 파일 탐색
- 사진 앱 이미지 열기
- 클립보드 동작 확인
- 메모장 텍스트 작성

---

## 시나리오 4: Calculator → Edge → 메모장 (계산 + 검색 + 문서화)
**목표:** 역방향 워크플로우 (계산 → 웹 → 문서)

**단계:**
1. Calculator 열기
2. 계산: (365 × 24) = 8760 (연간 시간)
3. 결과 복사 (Ctrl+C)
4. Edge 열기
5. Google 검색: "8760 hours in days"
6. 검색 결과 확인
7. Ctrl+L (주소창 포커스)
8. 현재 URL 복사 (Ctrl+C)
9. 메모장 열기
10. 새 문서 (Ctrl+N)
11. 타이핑: "Total hours per year: "
12. URL 붙여넣기 (Ctrl+V)
13. 전체 선택 (Ctrl+A)
14. 복사 (Ctrl+C)

**검증 포인트:**
- Calculator Ctrl+C
- Edge URL 복사
- 메모장 다중 작업
- Clipboard 덮어쓰기 처리

---

## 시나리오 5: 메모장 → Edge → 메모장 → 메일 (메모 기반 연구 워크플로우)
**목표:** 가장 복잡한 순환 워크플로우

**단계:**
1. 메모장 열기
2. 새 문서 (Ctrl+N)
3. 타이핑: "Research Topic: Rust programming language"
4. "Rust programming" 텍스트 선택
5. 복사 (Ctrl+C)
6. Edge 열기
7. Google 검색창에 붙여넣기 (Ctrl+V)
8. 검색 실행
9. 첫 번째 결과 URL 복사 (Ctrl+L → Ctrl+C)
10. 메모장으로 돌아가기 (Alt+Tab)
11. 기존 문서에 붙여넣기 (Ctrl+V)
12. 문서 전체 선택 (Ctrl+A)
13. 복사 (Ctrl+C)
14. 메일 앱/Outlook 열기
15. 새 이메일 (Ctrl+N)
16. 제목: "Research Findings"
17. 본문 붙여넣기 (Ctrl+V)

**검증 포인트:**
- 메모장 내부 편집
- Edge 검색 + URL 복사
- Alt+Tab 앱 전환
- 메모장 → Edge → 메모장 → 메일 체인
- 모든 단축키 메뉴 클릭 작동

---

## 실행 명령어

```powershell
# 시나리오 1
cargo run --bin local_os_agent -- surf "Edge를 열어 Google에서 'Apple stock price'를 검색하세요. 첫 번째 결과를 클릭하고 주가를 확인하세요. Calculator를 열어 그 숫자에 100을 곱하세요. 결과를 복사하고(Ctrl+C) 메모장에서 새 문서(Ctrl+N)를 만들어 'Apple Stock Calculation'이라고 입력한 뒤 결과를 붙여넣으세요(Ctrl+V)."

# 시나리오 2  
cargo run --bin local_os_agent -- surf "메모장을 열어 새 문서(Ctrl+N)를 만들고 'Meeting Summary: Discussed Q1 results'를 입력하세요. 전체 선택(Ctrl+A)하고 복사(Ctrl+C)하세요. 메모장에서 새 문서(Ctrl+N)를 만들어 붙여넣으세요(Ctrl+V)."

# 시나리오 3
cargo run --bin local_os_agent -- surf "Calculator를 열어 999 곱하기 3을 계산하세요. 결과를 복사(Ctrl+C)하고 메모장을 열어 새 문서(Ctrl+N)를 만들어 붙여넣으세요(Ctrl+V)."

# 시나리오 4
cargo run --bin local_os_agent -- surf "Calculator를 열어 365 곱하기 24를 계산하세요. 결과를 복사(Ctrl+C)하고 메모장에서 새 문서(Ctrl+N)를 만들어 'Total: '를 입력한 뒤 붙여넣으세요(Ctrl+V)."

# 시나리오 5
cargo run --bin local_os_agent -- surf "메모장을 열어 새 문서(Ctrl+N)를 만들고 'Test 12345'를 입력하세요. 전체 선택(Ctrl+A)하고 복사(Ctrl+C)한 다음 새 문서(Ctrl+N)를 만들어 붙여넣으세요(Ctrl+V)."
```
