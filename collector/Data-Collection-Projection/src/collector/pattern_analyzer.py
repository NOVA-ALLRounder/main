"""
Pattern Analyzer - 사용자 행동 패턴 분석 (통합 버전).

os_data의 PatternAnalyzer를 os_test의 SQLiteStore와 통합.
독립 SQLite 접근 대신 SQLiteStore의 potential_patterns 테이블을 사용합니다.

동작: Action 시퀀스를 분석하여 반복 패턴을 감지하고 저장.
"""

import hashlib
import json
import logging
import re
from collections import deque
from datetime import datetime
from typing import Any, Dict, List, Optional

try:
    from .action_buffer import ActionRecord
except ImportError:
    from action_buffer import ActionRecord

logger = logging.getLogger(__name__)


class PatternAnalyzer:
    """
    Action 시퀀스에서 반복 패턴 감지.

    N-gram 기반으로 사용자의 반복적인 행동 패턴을 식별하고,
    SQLiteStore에 저장합니다.
    """

    def __init__(
        self,
        store,
        history_size: int = 100,
        min_pattern_length: int = 2,
        max_pattern_length: int = 5,
        min_occurrences: int = 2,
    ):
        """
        Args:
            store: SQLiteStore 인스턴스 (insert_pattern 메서드 필요)
            history_size: 분석할 최근 액션 수
            min_pattern_length: 최소 패턴 길이
            max_pattern_length: 최대 패턴 길이
            min_occurrences: 패턴으로 인정할 최소 발생 횟수
        """
        self._store = store
        self._history: deque = deque(maxlen=history_size)
        self._min_len = min_pattern_length
        self._max_len = max_pattern_length
        self._min_occ = min_occurrences
        self._known_patterns: Dict[str, int] = {}  # pattern_hash → count

    def on_action(self, action: ActionRecord):
        """새 Action이 들어올 때마다 호출."""
        refined = self._refine_action(action)
        self._history.append(refined)

        # 분석 트리거: 10개마다 패턴 분석 수행
        if len(self._history) % 10 == 0:
            self._analyze()

    def _refine_action(self, action: ActionRecord) -> str:
        """Action을 분석 가능한 문자열로 변환.

        변수 부분(값, 시간 등)을 제거하고 구조만 남김.
        """
        parts = [action.action_type, action.app or ""]

        if action.control_name:
            parts.append(action.control_name)
        if action.control_type:
            parts.append(action.control_type)

        # 값이 있으면 placeholder로 대체 (패턴에서 변수 분리)
        if action.final_value:
            # URL, 이메일 등 변수 부분을 placeholder로
            value = action.final_value
            value = re.sub(r'[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+', '{EMAIL}', value)
            value = re.sub(r'https?://\S+', '{URL}', value)
            value = re.sub(r'\d{4}[-/]\d{1,2}[-/]\d{1,2}', '{DATE}', value)
            value = re.sub(r'\d+', '{NUM}', value)
            
            # 항상 값을 패턴에 포함 (단, 길이 제한)
            parts.append(value[:50])

        return "|".join(parts)

    def _analyze(self):
        """현재 히스토리에서 N-gram 패턴 분석."""
        if len(self._history) < self._min_len:
            return

        history_list = list(self._history)

        for n in range(self._min_len, self._max_len + 1):
            ngrams: Dict[str, int] = {}
            for i in range(len(history_list) - n + 1):
                gram = "→".join(history_list[i:i + n])
                ngrams[gram] = ngrams.get(gram, 0) + 1

            for gram, count in ngrams.items():
                if count >= self._min_occ:
                    pattern_hash = hashlib.md5(gram.encode()).hexdigest()[:16]
                    prev_count = self._known_patterns.get(pattern_hash, 0)

                    if count > prev_count:
                        self._known_patterns[pattern_hash] = count
                        self._save_pattern(pattern_hash, gram, count, n)

    def _save_pattern(
        self, pattern_hash: str, sequence: str, count: int, length: int
    ):
        """패턴을 SQLiteStore에 저장."""
        try:
            self._store.insert_pattern({
                "pattern_hash": pattern_hash,
                "sequence": sequence,
                "occurrence_count": count,
                "pattern_length": length,
                "detected_at": datetime.utcnow().isoformat() + "Z",
                "actions_json": json.dumps(
                    sequence.split("→"), ensure_ascii=False
                ),
            })
            logger.info(
                "Pattern detected [%s] count=%d len=%d: %s",
                pattern_hash[:8], count, length,
                sequence[:80] + ("..." if len(sequence) > 80 else ""),
            )
        except Exception as e:
            logger.error("Failed to save pattern: %s", e)

    def get_stats(self) -> Dict[str, Any]:
        return {
            "history_size": len(self._history),
            "known_patterns": len(self._known_patterns),
        }
