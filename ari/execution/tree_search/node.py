"""
Experiment Node

실험 트리의 노드 클래스
"""

from dataclasses import dataclass, field
from typing import List, Dict, Any, Optional
from enum import Enum
from datetime import datetime
import uuid


class NodeStatus(Enum):
    """노드 상태"""
    PENDING = "pending"      # 대기 중
    RUNNING = "running"      # 실행 중
    SUCCESS = "success"      # 성공
    FAILED = "failed"        # 실패
    PRUNED = "pruned"        # 가지치기됨
    BACKTRACKED = "backtracked"  # 백트래킹됨


@dataclass
class ExperimentNode:
    """실험 트리 노드"""
    
    # 식별자
    node_id: str = field(default_factory=lambda: str(uuid.uuid4())[:8])
    
    # 실험 정보
    description: str = ""           # 이 노드에서 수행한 변경 설명
    code_changes: str = ""          # 코드 변경 내용
    config_changes: Dict[str, Any] = field(default_factory=dict)  # 설정 변경
    
    # 상태
    status: NodeStatus = NodeStatus.PENDING
    
    # 결과
    metrics: Dict[str, float] = field(default_factory=dict)  # 성능 지표
    output: str = ""                # 실행 출력
    error: str = ""                 # 에러 메시지
    
    # 트리 구조
    parent_id: Optional[str] = None
    children_ids: List[str] = field(default_factory=list)
    depth: int = 0
    
    # 메타데이터
    created_at: datetime = field(default_factory=datetime.now)
    completed_at: Optional[datetime] = None
    execution_time: float = 0.0     # 실행 시간 (초)
    
    # 평가
    score: float = 0.0              # 종합 점수
    priority: float = 0.0           # 탐색 우선순위
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "node_id": self.node_id,
            "description": self.description,
            "code_changes": self.code_changes,
            "config_changes": self.config_changes,
            "status": self.status.value,
            "metrics": self.metrics,
            "output": self.output[:1000] if self.output else "",  # Truncate
            "error": self.error,
            "parent_id": self.parent_id,
            "children_ids": self.children_ids,
            "depth": self.depth,
            "created_at": self.created_at.isoformat(),
            "completed_at": self.completed_at.isoformat() if self.completed_at else None,
            "execution_time": self.execution_time,
            "score": self.score,
            "priority": self.priority
        }
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "ExperimentNode":
        return cls(
            node_id=data.get("node_id", ""),
            description=data.get("description", ""),
            code_changes=data.get("code_changes", ""),
            config_changes=data.get("config_changes", {}),
            status=NodeStatus(data.get("status", "pending")),
            metrics=data.get("metrics", {}),
            output=data.get("output", ""),
            error=data.get("error", ""),
            parent_id=data.get("parent_id"),
            children_ids=data.get("children_ids", []),
            depth=data.get("depth", 0),
            created_at=datetime.fromisoformat(data["created_at"]) if data.get("created_at") else datetime.now(),
            completed_at=datetime.fromisoformat(data["completed_at"]) if data.get("completed_at") else None,
            execution_time=data.get("execution_time", 0.0),
            score=data.get("score", 0.0),
            priority=data.get("priority", 0.0)
        )
    
    def is_terminal(self) -> bool:
        """터미널 노드인지 확인 (자식 없음)"""
        return len(self.children_ids) == 0
    
    def is_successful(self) -> bool:
        """성공 노드인지 확인"""
        return self.status == NodeStatus.SUCCESS
    
    def is_failed(self) -> bool:
        """실패 노드인지 확인"""
        return self.status in [NodeStatus.FAILED, NodeStatus.PRUNED]
    
    def mark_success(self, metrics: Dict[str, float] = None, output: str = ""):
        """노드를 성공으로 표시"""
        self.status = NodeStatus.SUCCESS
        self.completed_at = datetime.now()
        if metrics:
            self.metrics.update(metrics)
        self.output = output
        self._calculate_score()
    
    def mark_failed(self, error: str = ""):
        """노드를 실패로 표시"""
        self.status = NodeStatus.FAILED
        self.completed_at = datetime.now()
        self.error = error
        self.score = 0.0
    
    def mark_pruned(self, reason: str = ""):
        """노드를 가지치기됨으로 표시"""
        self.status = NodeStatus.PRUNED
        self.completed_at = datetime.now()
        self.error = f"Pruned: {reason}"
        self.score = 0.0
    
    def _calculate_score(self):
        """점수 계산"""
        if not self.metrics:
            self.score = 0.0
            return
        
        # 기본 점수 계산: 주요 지표의 가중 평균
        score = 0.0
        weights = {
            "accuracy": 1.0,
            "precision": 0.8,
            "recall": 0.8,
            "f1": 0.9,
            "loss": -0.5,  # 낮을수록 좋음
            "error_rate": -1.0,  # 낮을수록 좋음
        }
        
        total_weight = 0.0
        for metric, value in self.metrics.items():
            metric_lower = metric.lower()
            for key, weight in weights.items():
                if key in metric_lower:
                    if weight < 0:
                        score += weight * (1 - min(1.0, value))  # Invert
                    else:
                        score += weight * min(1.0, value)
                    total_weight += abs(weight)
                    break
            else:
                # 알 수 없는 지표는 양수로 가정
                score += 0.5 * min(1.0, value)
                total_weight += 0.5
        
        self.score = score / total_weight if total_weight > 0 else 0.0
