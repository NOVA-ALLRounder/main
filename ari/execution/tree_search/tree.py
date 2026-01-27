"""
Experiment Tree

실험 트리 구조 관리
"""

from typing import List, Dict, Any, Optional, Callable
from dataclasses import dataclass, field
import json
from pathlib import Path

from .node import ExperimentNode, NodeStatus


class ExperimentTree:
    """실험 트리"""
    
    def __init__(self, root_description: str = "Initial experiment"):
        """
        실험 트리 초기화
        
        Args:
            root_description: 루트 노드 설명
        """
        self.nodes: Dict[str, ExperimentNode] = {}
        
        # Create root node
        self.root = ExperimentNode(
            description=root_description,
            depth=0
        )
        self.nodes[self.root.node_id] = self.root
    
    def add_child(
        self,
        parent_id: str,
        description: str,
        code_changes: str = "",
        config_changes: Dict[str, Any] = None
    ) -> ExperimentNode:
        """
        자식 노드 추가
        
        Args:
            parent_id: 부모 노드 ID
            description: 노드 설명
            code_changes: 코드 변경 내용
            config_changes: 설정 변경
        
        Returns:
            생성된 자식 노드
        """
        if parent_id not in self.nodes:
            raise ValueError(f"Parent node {parent_id} not found")
        
        parent = self.nodes[parent_id]
        
        child = ExperimentNode(
            description=description,
            code_changes=code_changes,
            config_changes=config_changes or {},
            parent_id=parent_id,
            depth=parent.depth + 1
        )
        
        self.nodes[child.node_id] = child
        parent.children_ids.append(child.node_id)
        
        return child
    
    def get_node(self, node_id: str) -> Optional[ExperimentNode]:
        """노드 조회"""
        return self.nodes.get(node_id)
    
    def get_parent(self, node_id: str) -> Optional[ExperimentNode]:
        """부모 노드 조회"""
        node = self.get_node(node_id)
        if node and node.parent_id:
            return self.get_node(node.parent_id)
        return None
    
    def get_children(self, node_id: str) -> List[ExperimentNode]:
        """자식 노드들 조회"""
        node = self.get_node(node_id)
        if not node:
            return []
        return [self.nodes[cid] for cid in node.children_ids if cid in self.nodes]
    
    def get_path_to_root(self, node_id: str) -> List[ExperimentNode]:
        """루트까지의 경로 반환 (노드 → 루트)"""
        path = []
        current = self.get_node(node_id)
        
        while current:
            path.append(current)
            current = self.get_parent(current.node_id)
        
        return path
    
    def get_path_from_root(self, node_id: str) -> List[ExperimentNode]:
        """루트부터의 경로 반환 (루트 → 노드)"""
        return list(reversed(self.get_path_to_root(node_id)))
    
    def get_leaves(self) -> List[ExperimentNode]:
        """모든 리프 노드 반환"""
        return [node for node in self.nodes.values() if node.is_terminal()]
    
    def get_successful_leaves(self) -> List[ExperimentNode]:
        """성공한 리프 노드들 반환"""
        return [node for node in self.get_leaves() if node.is_successful()]
    
    def get_best_node(self) -> Optional[ExperimentNode]:
        """가장 높은 점수의 노드 반환"""
        successful = [n for n in self.nodes.values() if n.is_successful()]
        if not successful:
            return None
        return max(successful, key=lambda n: n.score)
    
    def get_best_path(self) -> List[ExperimentNode]:
        """가장 좋은 경로 반환"""
        best = self.get_best_node()
        if not best:
            return []
        return self.get_path_from_root(best.node_id)
    
    def prune_subtree(self, node_id: str, reason: str = ""):
        """서브트리 가지치기"""
        node = self.get_node(node_id)
        if not node:
            return
        
        def prune_recursive(n: ExperimentNode):
            n.mark_pruned(reason)
            for child_id in n.children_ids:
                child = self.get_node(child_id)
                if child:
                    prune_recursive(child)
        
        prune_recursive(node)
    
    def backtrack(self, node_id: str) -> Optional[ExperimentNode]:
        """
        백트래킹: 부모 노드로 돌아가서 다른 경로 탐색
        
        Returns:
            백트래킹한 부모 노드 또는 None
        """
        node = self.get_node(node_id)
        if not node:
            return None
        
        node.status = NodeStatus.BACKTRACKED
        
        parent = self.get_parent(node_id)
        return parent
    
    def get_expandable_nodes(self) -> List[ExperimentNode]:
        """확장 가능한 노드들 반환 (성공했지만 아직 자식이 많지 않은 노드)"""
        expandable = []
        
        for node in self.nodes.values():
            if node.status == NodeStatus.SUCCESS and len(node.children_ids) < 3:
                expandable.append(node)
        
        # 점수와 깊이로 정렬
        expandable.sort(key=lambda n: (n.score, -n.depth), reverse=True)
        return expandable
    
    def get_pending_nodes(self) -> List[ExperimentNode]:
        """대기 중인 노드들 반환"""
        return [n for n in self.nodes.values() if n.status == NodeStatus.PENDING]
    
    def stats(self) -> Dict[str, Any]:
        """트리 통계"""
        status_counts = {status.value: 0 for status in NodeStatus}
        for node in self.nodes.values():
            status_counts[node.status.value] += 1
        
        depths = [n.depth for n in self.nodes.values()]
        
        return {
            "total_nodes": len(self.nodes),
            "max_depth": max(depths) if depths else 0,
            "status_counts": status_counts,
            "best_score": self.get_best_node().score if self.get_best_node() else 0.0,
            "num_leaves": len(self.get_leaves()),
            "num_successful": status_counts.get("success", 0)
        }
    
    def to_dict(self) -> Dict[str, Any]:
        """딕셔너리로 변환"""
        return {
            "root_id": self.root.node_id,
            "nodes": {nid: node.to_dict() for nid, node in self.nodes.items()}
        }
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "ExperimentTree":
        """딕셔너리에서 생성"""
        tree = cls.__new__(cls)
        tree.nodes = {}
        
        for nid, node_data in data.get("nodes", {}).items():
            tree.nodes[nid] = ExperimentNode.from_dict(node_data)
        
        tree.root = tree.nodes.get(data.get("root_id", ""))
        return tree
    
    def save(self, filepath: str):
        """파일로 저장"""
        Path(filepath).parent.mkdir(parents=True, exist_ok=True)
        with open(filepath, 'w', encoding='utf-8') as f:
            json.dump(self.to_dict(), f, indent=2, ensure_ascii=False)
    
    @classmethod
    def load(cls, filepath: str) -> "ExperimentTree":
        """파일에서 로드"""
        with open(filepath, 'r', encoding='utf-8') as f:
            data = json.load(f)
        return cls.from_dict(data)
    
    def visualize_text(self, max_depth: int = None) -> str:
        """텍스트로 트리 시각화"""
        lines = []
        
        def visualize_node(node: ExperimentNode, prefix: str = "", is_last: bool = True):
            if max_depth is not None and node.depth > max_depth:
                return
            
            connector = "└── " if is_last else "├── "
            status_icon = {
                NodeStatus.PENDING: "⏳",
                NodeStatus.RUNNING: "🔄",
                NodeStatus.SUCCESS: "✅",
                NodeStatus.FAILED: "❌",
                NodeStatus.PRUNED: "✂️",
                NodeStatus.BACKTRACKED: "↩️"
            }.get(node.status, "?")
            
            score_str = f" (score: {node.score:.2f})" if node.score > 0 else ""
            lines.append(f"{prefix}{connector}{status_icon} {node.description[:50]}{score_str}")
            
            children = self.get_children(node.node_id)
            for i, child in enumerate(children):
                new_prefix = prefix + ("    " if is_last else "│   ")
                visualize_node(child, new_prefix, i == len(children) - 1)
        
        lines.append(f"🌳 Experiment Tree (nodes: {len(self.nodes)})")
        visualize_node(self.root, "", True)
        
        return "\n".join(lines)
