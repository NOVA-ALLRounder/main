# Tool Interface Definitions

## Observe Tools
* `ui.snapshot(scope: string) -> JSON`: 현재 활성 윈도우의 UI 트리 반환.
* `ui.find(query: string) -> ElementID[]`: 자연어 쿼리 또는 속성으로 UI 요소 검색.

## Act Tools
* `ui.click(element_id: string)`: 특정 UI 요소 클릭 (좌표 아님).
* `mouse.move(x: int, y: int)`: (Fallback) 좌표 기반 이동.
* `keyboard.type(text: string)`: 텍스트 입력.

## Verify Tools
* `verify.changed(snapshot_id_before: string, snapshot_id_after: string) -> bool`: 상태 변경 감지.
* `verify.exists(element_id: string) -> bool`: 요소 존재 여부 확인.
