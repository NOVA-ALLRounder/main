"""
Literature-Based Discovery (LBD)

문헌 기반 발견: A→B→C 추론을 통한 가설 생성
"""

from typing import List, Dict, Any, Optional, Tuple
from dataclasses import dataclass

import sys
import os
sys.path.append(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
from core.llm import LLMClient, get_llm_client
from cognitive.knowledge_graph.graph import KnowledgeGraph
from cognitive.knowledge_graph.triple_extractor import Triple
from cognitive.ideation.hypothesis import Hypothesis, HypothesisGenerator


@dataclass
class LBDCandidate:
    """LBD 후보 연결"""
    entity_a: str  # 시작 개념
    entity_b: str  # 브릿지 개념
    entity_c: str  # 목표 개념
    
    a_to_b_relations: List[Triple]  # A→B 관계
    b_to_c_relations: List[Triple]  # B→C 관계
    
    inferred_relation: str  # 추론된 A→C 관계
    confidence: float  # 신뢰도
    hypothesis: Optional[str] = None  # 생성된 가설
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "entity_a": self.entity_a,
            "entity_b": self.entity_b,
            "entity_c": self.entity_c,
            "a_to_b_relations": [t.to_dict() for t in self.a_to_b_relations],
            "b_to_c_relations": [t.to_dict() for t in self.b_to_c_relations],
            "inferred_relation": self.inferred_relation,
            "confidence": self.confidence,
            "hypothesis": self.hypothesis
        }


class LiteratureBasedDiscovery:
    """문헌 기반 발견 시스템"""
    
    INFERENCE_PROMPT = """You are performing literature-based discovery (LBD) to find hidden connections between concepts.

Given the following relationships:
A ({entity_a}) is related to B ({entity_b}):
{a_to_b_relations}

B ({entity_b}) is related to C ({entity_c}):
{b_to_c_relations}

Infer a potential direct relationship between A ({entity_a}) and C ({entity_c}).

Return your analysis in JSON format:
{{
    "inferred_relation": "Description of the potential A→C relationship",
    "confidence": 0.0-1.0,
    "reasoning": "Step-by-step reasoning for this inference",
    "research_hypothesis": "A testable hypothesis based on this connection",
    "potential_experiments": ["experiment1", "experiment2", ...]
}}
"""
    
    def __init__(
        self,
        knowledge_graph: KnowledgeGraph,
        llm_client: Optional[LLMClient] = None
    ):
        self.kg = knowledge_graph
        self.llm = llm_client or get_llm_client()
        self.hypothesis_generator = HypothesisGenerator(llm_client)
    
    def find_candidates(
        self,
        source_entity: str,
        target_entity: Optional[str] = None,
        min_confidence: float = 0.5
    ) -> List[LBDCandidate]:
        """
        LBD 후보 연결 찾기
        
        Args:
            source_entity: 시작 개념 (A)
            target_entity: 목표 개념 (C), None이면 모든 가능한 연결 탐색
            min_confidence: 최소 신뢰도
        
        Returns:
            LBD 후보 리스트
        """
        candidates = []
        
        if target_entity:
            # Find bridges between specific A and C
            bridges = self.kg.find_bridging_concepts(source_entity, target_entity)
            
            for bridge, a_to_b, b_to_c in bridges:
                candidate = self._create_candidate(
                    source_entity, bridge, target_entity,
                    a_to_b, b_to_c
                )
                if candidate and candidate.confidence >= min_confidence:
                    candidates.append(candidate)
        else:
            # Explore all possible connections from A
            a_neighbors = self.kg.get_neighbors(source_entity)
            
            for bridge in a_neighbors:
                c_neighbors = self.kg.get_neighbors(bridge)
                
                for target in c_neighbors:
                    # Skip if A and C are the same or already directly connected
                    if target == source_entity.lower():
                        continue
                    if target in self.kg.get_neighbors(source_entity):
                        continue
                    
                    a_to_b = self.kg.get_outgoing(source_entity)
                    a_to_b = [t for t in a_to_b if t.object.lower() == bridge]
                    
                    b_to_c = self.kg.get_outgoing(bridge)
                    b_to_c = [t for t in b_to_c if t.object.lower() == target]
                    
                    if a_to_b and b_to_c:
                        candidate = self._create_candidate(
                            source_entity, bridge, target,
                            a_to_b, b_to_c
                        )
                        if candidate and candidate.confidence >= min_confidence:
                            candidates.append(candidate)
        
        # Sort by confidence
        candidates.sort(key=lambda x: x.confidence, reverse=True)
        
        return candidates
    
    def _create_candidate(
        self,
        entity_a: str,
        entity_b: str,
        entity_c: str,
        a_to_b: List[Triple],
        b_to_c: List[Triple]
    ) -> Optional[LBDCandidate]:
        """LBD 후보 생성"""
        if not a_to_b or not b_to_c:
            return None
        
        # Calculate confidence based on triple confidences
        avg_confidence = (
            sum(t.confidence for t in a_to_b) / len(a_to_b) +
            sum(t.confidence for t in b_to_c) / len(b_to_c)
        ) / 2
        
        # Simple inferred relation
        inferred = f"{entity_a} may be related to {entity_c} through {entity_b}"
        
        return LBDCandidate(
            entity_a=entity_a,
            entity_b=entity_b,
            entity_c=entity_c,
            a_to_b_relations=a_to_b,
            b_to_c_relations=b_to_c,
            inferred_relation=inferred,
            confidence=avg_confidence
        )
    
    def generate_hypothesis_from_candidate(
        self,
        candidate: LBDCandidate
    ) -> Optional[Hypothesis]:
        """
        LBD 후보에서 가설 생성
        
        Args:
            candidate: LBD 후보
        
        Returns:
            생성된 가설
        """
        # Format relations for LLM
        a_to_b_text = "\n".join([
            f"- {t.subject} {t.relation} {t.object}"
            for t in candidate.a_to_b_relations
        ])
        
        b_to_c_text = "\n".join([
            f"- {t.subject} {t.relation} {t.object}"
            for t in candidate.b_to_c_relations
        ])
        
        prompt = self.INFERENCE_PROMPT.format(
            entity_a=candidate.entity_a,
            entity_b=candidate.entity_b,
            entity_c=candidate.entity_c,
            a_to_b_relations=a_to_b_text,
            b_to_c_relations=b_to_c_text
        )
        
        try:
            response = self.llm.generate_json(
                prompt=prompt,
                system_prompt="You are a research scientist discovering novel connections in scientific literature.",
                temperature=0.5
            )
            
            # Update candidate
            candidate.inferred_relation = response.get("inferred_relation", candidate.inferred_relation)
            candidate.confidence = float(response.get("confidence", candidate.confidence))
            candidate.hypothesis = response.get("research_hypothesis", "")
            
            # Create full hypothesis
            hypothesis = Hypothesis(
                title=f"Exploring the connection between {candidate.entity_a} and {candidate.entity_c}",
                research_question=response.get("research_hypothesis", ""),
                methodology=f"Investigate the relationship through the bridging concept of {candidate.entity_b}",
                experiment_plan=response.get("potential_experiments", []),
                expected_results=response.get("reasoning", ""),
                novelty_score=candidate.confidence * 10,
                feasibility_score=7.0,
                impact_score=candidate.confidence * 10,
                keywords=[candidate.entity_a, candidate.entity_b, candidate.entity_c],
                source="lbd"
            )
            
            return hypothesis
        
        except Exception as e:
            print(f"LBD hypothesis generation error: {e}")
            return None
    
    def discover(
        self,
        source_entities: List[str],
        max_candidates: int = 10
    ) -> List[Tuple[LBDCandidate, Optional[Hypothesis]]]:
        """
        여러 소스 개념에서 LBD 수행
        
        Args:
            source_entities: 시작 개념 리스트
            max_candidates: 최대 후보 수
        
        Returns:
            (LBD 후보, 가설) 튜플 리스트
        """
        all_candidates = []
        
        for entity in source_entities:
            candidates = self.find_candidates(entity)
            all_candidates.extend(candidates)
        
        # Sort by confidence and take top N
        all_candidates.sort(key=lambda x: x.confidence, reverse=True)
        top_candidates = all_candidates[:max_candidates]
        
        # Generate hypotheses
        results = []
        for candidate in top_candidates:
            hypothesis = self.generate_hypothesis_from_candidate(candidate)
            results.append((candidate, hypothesis))
        
        return results
