"""
Tree Searcher

트리 탐색 전략 및 실행
"""

from typing import List, Dict, Any, Optional, Callable
from enum import Enum
from dataclasses import dataclass
import random

from .node import ExperimentNode, NodeStatus
from .tree import ExperimentTree


class SearchStrategy(Enum):
    """탐색 전략"""
    BFS = "bfs"          # 너비 우선 탐색
    DFS = "dfs"          # 깊이 우선 탐색
    BEST_FIRST = "best"  # 최선 우선 탐색
    UCB = "ucb"          # Upper Confidence Bound


@dataclass
class SearchConfig:
    """탐색 설정"""
    strategy: SearchStrategy = SearchStrategy.BEST_FIRST
    max_depth: int = 10
    max_nodes: int = 100
    max_children_per_node: int = 3
    exploration_weight: float = 1.0  # UCB exploration weight
    prune_threshold: float = 0.1     # 이 점수 이하면 가지치기
    backtrack_on_failure: bool = True


class TreeSearcher:
    """트리 탐색기"""
    
    def __init__(
        self,
        tree: ExperimentTree,
        config: SearchConfig = None,
        executor: Callable[[ExperimentNode], Dict[str, Any]] = None
    ):
        """
        트리 탐색기 초기화
        
        Args:
            tree: 실험 트리
            config: 탐색 설정
            executor: 노드 실행 함수 (노드를 받아 결과 반환)
        """
        self.tree = tree
        self.config = config or SearchConfig()
        self.executor = executor
        
        self._frontier: List[str] = []  # 탐색 대기 노드 ID 목록
        self._visited: set = set()
    
    def set_executor(self, executor: Callable[[ExperimentNode], Dict[str, Any]]):
        """실행 함수 설정"""
        self.executor = executor
    
    def _select_next_node(self) -> Optional[ExperimentNode]:
        """다음 탐색할 노드 선택"""
        if not self._frontier:
            return None
        
        if self.config.strategy == SearchStrategy.BFS:
            # FIFO
            node_id = self._frontier.pop(0)
        
        elif self.config.strategy == SearchStrategy.DFS:
            # LIFO
            node_id = self._frontier.pop()
        
        elif self.config.strategy == SearchStrategy.BEST_FIRST:
            # 최고 점수/우선순위 선택
            best_idx = 0
            best_priority = -float('inf')
            
            for i, nid in enumerate(self._frontier):
                node = self.tree.get_node(nid)
                if node and node.priority > best_priority:
                    best_priority = node.priority
                    best_idx = i
            
            node_id = self._frontier.pop(best_idx)
        
        elif self.config.strategy == SearchStrategy.UCB:
            # Upper Confidence Bound
            import math
            
            total_visits = len(self._visited) + 1
            best_idx = 0
            best_ucb = -float('inf')
            
            for i, nid in enumerate(self._frontier):
                node = self.tree.get_node(nid)
                if not node:
                    continue
                
                # UCB formula
                exploitation = node.score if node.score > 0 else 0.5
                exploration = self.config.exploration_weight * math.sqrt(
                    math.log(total_visits) / (1 + len(self._visited))
                )
                ucb = exploitation + exploration
                
                if ucb > best_ucb:
                    best_ucb = ucb
                    best_idx = i
            
            node_id = self._frontier.pop(best_idx)
        
        else:
            node_id = self._frontier.pop(0)
        
        return self.tree.get_node(node_id)
    
    def _add_to_frontier(self, node: ExperimentNode):
        """프론티어에 노드 추가"""
        if node.node_id not in self._visited and node.node_id not in self._frontier:
            self._frontier.append(node.node_id)
    
    def _should_prune(self, node: ExperimentNode) -> bool:
        """가지치기 여부 판단"""
        # 깊이 제한
        if node.depth > self.config.max_depth:
            return True
        
        # 점수 임계값
        if node.score < self.config.prune_threshold and node.status == NodeStatus.SUCCESS:
            # 부모보다 현저히 낮은 점수
            parent = self.tree.get_parent(node.node_id)
            if parent and node.score < parent.score * 0.5:
                return True
        
        return False
    
    def _generate_children(
        self,
        node: ExperimentNode,
        child_generator: Callable[[ExperimentNode], List[Dict[str, Any]]]
    ) -> List[ExperimentNode]:
        """
        자식 노드 생성
        
        Args:
            node: 부모 노드
            child_generator: 자식 후보 생성 함수
        
        Returns:
            생성된 자식 노드 리스트
        """
        if len(node.children_ids) >= self.config.max_children_per_node:
            return []
        
        # 자식 후보 생성
        candidates = child_generator(node)
        
        children = []
        for candidate in candidates[:self.config.max_children_per_node - len(node.children_ids)]:
            child = self.tree.add_child(
                parent_id=node.node_id,
                description=candidate.get("description", ""),
                code_changes=candidate.get("code_changes", ""),
                config_changes=candidate.get("config_changes", {})
            )
            
            # 우선순위 설정
            child.priority = node.score + random.uniform(0, 0.1)
            children.append(child)
        
        return children
    
    def search(
        self,
        child_generator: Callable[[ExperimentNode], List[Dict[str, Any]]],
        max_iterations: int = None
    ) -> ExperimentNode:
        """
        트리 탐색 수행
        
        Args:
            child_generator: 자식 노드 후보 생성 함수
            max_iterations: 최대 반복 횟수
        
        Returns:
            가장 좋은 결과의 노드
        """
        if not self.executor:
            raise ValueError("Executor not set. Call set_executor() first.")
        
        max_iterations = max_iterations or self.config.max_nodes
        
        # 초기화
        self._frontier = [self.tree.root.node_id]
        self._visited = set()
        
        iteration = 0
        
        while self._frontier and iteration < max_iterations:
            if len(self.tree.nodes) >= self.config.max_nodes:
                break
            
            # 다음 노드 선택
            node = self._select_next_node()
            if not node or node.node_id in self._visited:
                continue
            
            self._visited.add(node.node_id)
            iteration += 1
            
            # 노드 실행
            node.status = NodeStatus.RUNNING
            
            try:
                result = self.executor(node)
                
                if result.get("success", False):
                    node.mark_success(
                        metrics=result.get("metrics", {}),
                        output=result.get("output", "")
                    )
                    
                    # 가지치기 검사
                    if self._should_prune(node):
                        self.tree.prune_subtree(node.node_id, "Below threshold")
                        continue
                    
                    # 자식 생성
                    children = self._generate_children(node, child_generator)
                    for child in children:
                        self._add_to_frontier(child)
                
                else:
                    node.mark_failed(error=result.get("error", "Unknown error"))
                    
                    # 백트래킹
                    if self.config.backtrack_on_failure:
                        parent = self.tree.backtrack(node.node_id)
                        if parent and parent.node_id not in self._visited:
                            self._add_to_frontier(parent)
            
            except Exception as e:
                node.mark_failed(error=str(e))
        
        return self.tree.get_best_node()
    
    def search_step(
        self,
        child_generator: Callable[[ExperimentNode], List[Dict[str, Any]]]
    ) -> Optional[ExperimentNode]:
        """
        단일 탐색 스텝 수행 (대화형 탐색용)
        
        Returns:
            실행된 노드 또는 None (탐색 완료 시)
        """
        if not self._frontier:
            self._frontier = [self.tree.root.node_id]
        
        node = self._select_next_node()
        if not node:
            return None
        
        if node.node_id in self._visited:
            return self.search_step(child_generator)
        
        self._visited.add(node.node_id)
        
        if self.executor:
            node.status = NodeStatus.RUNNING
            try:
                result = self.executor(node)
                
                if result.get("success", False):
                    node.mark_success(
                        metrics=result.get("metrics", {}),
                        output=result.get("output", "")
                    )
                    
                    if not self._should_prune(node):
                        children = self._generate_children(node, child_generator)
                        for child in children:
                            self._add_to_frontier(child)
                else:
                    node.mark_failed(error=result.get("error", ""))
                    
            except Exception as e:
                node.mark_failed(error=str(e))
        
        return node
    
    def reset(self):
        """탐색 상태 초기화"""
        self._frontier = []
        self._visited = set()
