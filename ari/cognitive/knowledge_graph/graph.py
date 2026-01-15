"""
Knowledge Graph

지식 그래프 구조 및 추론
"""

from typing import List, Dict, Any, Optional, Set, Tuple
from dataclasses import dataclass, field
from collections import defaultdict
import json

from .triple_extractor import Triple


@dataclass
class Entity:
    """그래프 엔티티 (노드)"""
    name: str
    entity_type: str = "concept"  # concept, method, problem, dataset, metric
    aliases: Set[str] = field(default_factory=set)
    properties: Dict[str, Any] = field(default_factory=dict)
    
    def __hash__(self):
        return hash(self.name.lower())
    
    def __eq__(self, other):
        if not isinstance(other, Entity):
            return False
        return self.name.lower() == other.name.lower()


class KnowledgeGraph:
    """지식 그래프"""
    
    def __init__(self):
        self.entities: Dict[str, Entity] = {}  # name -> Entity
        self.triples: List[Triple] = []
        
        # Adjacency lists for fast lookup
        self._outgoing: Dict[str, List[Triple]] = defaultdict(list)  # subject -> triples
        self._incoming: Dict[str, List[Triple]] = defaultdict(list)  # object -> triples
        self._relations: Dict[str, List[Triple]] = defaultdict(list)  # relation -> triples
    
    def add_entity(self, entity: Entity):
        """엔티티 추가"""
        key = entity.name.lower()
        if key not in self.entities:
            self.entities[key] = entity
        else:
            # Merge aliases and properties
            existing = self.entities[key]
            existing.aliases.update(entity.aliases)
            existing.properties.update(entity.properties)
    
    def add_triple(self, triple: Triple):
        """트리플 추가"""
        # Create entities if they don't exist
        self.add_entity(Entity(name=triple.subject))
        self.add_entity(Entity(name=triple.object))
        
        # Add triple
        self.triples.append(triple)
        
        # Update adjacency lists
        subj_key = triple.subject.lower()
        obj_key = triple.object.lower()
        rel_key = triple.relation.lower()
        
        self._outgoing[subj_key].append(triple)
        self._incoming[obj_key].append(triple)
        self._relations[rel_key].append(triple)
    
    def add_triples(self, triples: List[Triple]):
        """트리플 리스트 추가"""
        for triple in triples:
            self.add_triple(triple)
    
    def get_entity(self, name: str) -> Optional[Entity]:
        """엔티티 조회"""
        return self.entities.get(name.lower())
    
    def get_outgoing(self, entity_name: str) -> List[Triple]:
        """엔티티에서 나가는 관계 조회"""
        return self._outgoing.get(entity_name.lower(), [])
    
    def get_incoming(self, entity_name: str) -> List[Triple]:
        """엔티티로 들어오는 관계 조회"""
        return self._incoming.get(entity_name.lower(), [])
    
    def get_neighbors(self, entity_name: str) -> Set[str]:
        """인접 엔티티 조회"""
        neighbors = set()
        
        for triple in self.get_outgoing(entity_name):
            neighbors.add(triple.object.lower())
        
        for triple in self.get_incoming(entity_name):
            neighbors.add(triple.subject.lower())
        
        return neighbors
    
    def find_path(
        self,
        start: str,
        end: str,
        max_depth: int = 4
    ) -> Optional[List[Triple]]:
        """
        두 엔티티 간 경로 찾기 (BFS)
        
        Args:
            start: 시작 엔티티
            end: 끝 엔티티
            max_depth: 최대 탐색 깊이
        
        Returns:
            경로를 구성하는 트리플 리스트 또는 None
        """
        start_key = start.lower()
        end_key = end.lower()
        
        if start_key not in self.entities or end_key not in self.entities:
            return None
        
        if start_key == end_key:
            return []
        
        # BFS
        queue = [(start_key, [])]
        visited = {start_key}
        
        while queue:
            current, path = queue.pop(0)
            
            if len(path) >= max_depth:
                continue
            
            # Check outgoing edges
            for triple in self.get_outgoing(current):
                next_node = triple.object.lower()
                
                if next_node == end_key:
                    return path + [triple]
                
                if next_node not in visited:
                    visited.add(next_node)
                    queue.append((next_node, path + [triple]))
            
            # Check incoming edges (bidirectional search)
            for triple in self.get_incoming(current):
                next_node = triple.subject.lower()
                
                if next_node == end_key:
                    return path + [triple]
                
                if next_node not in visited:
                    visited.add(next_node)
                    queue.append((next_node, path + [triple]))
        
        return None
    
    def find_bridging_concepts(
        self,
        entity_a: str,
        entity_c: str
    ) -> List[Tuple[str, List[Triple], List[Triple]]]:
        """
        문헌 기반 발견(LBD)을 위한 브릿징 개념 찾기
        A -> B -> C 형태의 연결 찾기
        
        Args:
            entity_a: 시작 엔티티
            entity_c: 끝 엔티티
        
        Returns:
            (브릿지 엔티티, A->B 트리플들, B->C 트리플들) 리스트
        """
        bridges = []
        
        a_neighbors = self.get_neighbors(entity_a)
        c_neighbors = self.get_neighbors(entity_c)
        
        # Find common neighbors (bridging concepts)
        bridging = a_neighbors & c_neighbors
        
        for bridge in bridging:
            a_to_b = [t for t in self.get_outgoing(entity_a) 
                      if t.object.lower() == bridge]
            a_to_b += [t for t in self.get_incoming(entity_a) 
                       if t.subject.lower() == bridge]
            
            b_to_c = [t for t in self.get_outgoing(bridge) 
                      if t.object.lower() == entity_c.lower()]
            b_to_c += [t for t in self.get_incoming(bridge) 
                       if t.subject.lower() == entity_c.lower()]
            
            if a_to_b and b_to_c:
                bridges.append((bridge, a_to_b, b_to_c))
        
        return bridges
    
    def get_related_entities(
        self,
        entity_name: str,
        relation_type: Optional[str] = None,
        max_depth: int = 2
    ) -> Dict[str, int]:
        """
        관련 엔티티 탐색 (거리별)
        
        Args:
            entity_name: 시작 엔티티
            relation_type: 관계 유형 필터
            max_depth: 최대 탐색 깊이
        
        Returns:
            {엔티티명: 거리} 딕셔너리
        """
        visited = {entity_name.lower(): 0}
        queue = [(entity_name.lower(), 0)]
        
        while queue:
            current, depth = queue.pop(0)
            
            if depth >= max_depth:
                continue
            
            neighbors = self.get_neighbors(current)
            
            for neighbor in neighbors:
                if neighbor not in visited:
                    visited[neighbor] = depth + 1
                    queue.append((neighbor, depth + 1))
        
        del visited[entity_name.lower()]  # Remove self
        return visited
    
    def to_dict(self) -> Dict[str, Any]:
        """딕셔너리로 변환"""
        return {
            "entities": [
                {
                    "name": e.name,
                    "entity_type": e.entity_type,
                    "aliases": list(e.aliases),
                    "properties": e.properties
                }
                for e in self.entities.values()
            ],
            "triples": [t.to_dict() for t in self.triples]
        }
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "KnowledgeGraph":
        """딕셔너리에서 생성"""
        graph = cls()
        
        for entity_data in data.get("entities", []):
            entity = Entity(
                name=entity_data["name"],
                entity_type=entity_data.get("entity_type", "concept"),
                aliases=set(entity_data.get("aliases", [])),
                properties=entity_data.get("properties", {})
            )
            graph.add_entity(entity)
        
        for triple_data in data.get("triples", []):
            triple = Triple(
                subject=triple_data["subject"],
                relation=triple_data["relation"],
                object=triple_data["object"],
                confidence=triple_data.get("confidence", 1.0),
                source=triple_data.get("source", "")
            )
            graph.add_triple(triple)
        
        return graph
    
    def save(self, filepath: str):
        """파일로 저장"""
        with open(filepath, 'w', encoding='utf-8') as f:
            json.dump(self.to_dict(), f, indent=2, ensure_ascii=False)
    
    @classmethod
    def load(cls, filepath: str) -> "KnowledgeGraph":
        """파일에서 로드"""
        with open(filepath, 'r', encoding='utf-8') as f:
            data = json.load(f)
        return cls.from_dict(data)
    
    def stats(self) -> Dict[str, int]:
        """그래프 통계"""
        return {
            "num_entities": len(self.entities),
            "num_triples": len(self.triples),
            "num_relations": len(self._relations)
        }
