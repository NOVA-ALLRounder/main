"""
Triple Extractor

LLM을 사용한 (주어, 관계, 목적어) 트리플 추출
"""

from dataclasses import dataclass
from typing import List, Optional, Dict, Any
import json

import sys
import os
sys.path.append(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
from core.llm import LLMClient, get_llm_client


@dataclass
class Triple:
    """지식 그래프 트리플"""
    subject: str
    relation: str
    object: str
    confidence: float = 1.0
    source: str = ""  # 원본 텍스트 또는 논문 ID
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "subject": self.subject,
            "relation": self.relation,
            "object": self.object,
            "confidence": self.confidence,
            "source": self.source
        }
    
    def __hash__(self):
        return hash((self.subject.lower(), self.relation.lower(), self.object.lower()))
    
    def __eq__(self, other):
        if not isinstance(other, Triple):
            return False
        return (
            self.subject.lower() == other.subject.lower() and
            self.relation.lower() == other.relation.lower() and
            self.object.lower() == other.object.lower()
        )


class TripleExtractor:
    """LLM 기반 트리플 추출기"""
    
    EXTRACTION_PROMPT = """Extract knowledge triples (subject, relation, object) from the following scientific text.
Focus on:
1. Technical concepts and their relationships
2. Methods and their applications
3. Problems and their solutions
4. Comparisons between approaches
5. Cause and effect relationships

Return a JSON array of triples with this structure:
{
    "triples": [
        {
            "subject": "entity or concept",
            "relation": "relationship verb or phrase",
            "object": "entity or concept",
            "confidence": 0.0-1.0
        }
    ]
}

Text:
"""
    
    def __init__(self, llm_client: Optional[LLMClient] = None):
        self.llm = llm_client or get_llm_client()
    
    def extract(
        self,
        text: str,
        max_triples: int = 20,
        source: str = ""
    ) -> List[Triple]:
        """
        텍스트에서 트리플 추출
        
        Args:
            text: 추출할 텍스트
            max_triples: 최대 트리플 수
            source: 원본 소스 식별자
        
        Returns:
            추출된 트리플 리스트
        """
        # Truncate text if too long
        if len(text) > 8000:
            text = text[:8000] + "..."
        
        prompt = self.EXTRACTION_PROMPT + text + f"\n\nExtract up to {max_triples} triples."
        
        try:
            response = self.llm.generate_json(
                prompt=prompt,
                system_prompt="You are a knowledge extraction expert. Extract precise, meaningful triples from scientific text.",
                temperature=0.3
            )
            
            triples = []
            for item in response.get("triples", []):
                triple = Triple(
                    subject=item.get("subject", ""),
                    relation=item.get("relation", ""),
                    object=item.get("object", ""),
                    confidence=item.get("confidence", 0.8),
                    source=source
                )
                if triple.subject and triple.relation and triple.object:
                    triples.append(triple)
            
            return triples[:max_triples]
        
        except Exception as e:
            print(f"Triple extraction error: {e}")
            return []
    
    def extract_from_abstract(self, abstract: str, paper_id: str = "") -> List[Triple]:
        """논문 초록에서 트리플 추출"""
        return self.extract(abstract, max_triples=10, source=paper_id)
    
    def extract_from_paper(
        self,
        title: str,
        abstract: str,
        paper_id: str = ""
    ) -> List[Triple]:
        """논문 제목과 초록에서 트리플 추출"""
        combined_text = f"Title: {title}\n\nAbstract: {abstract}"
        return self.extract(combined_text, max_triples=15, source=paper_id)
    
    def merge_triples(self, triples: List[Triple]) -> List[Triple]:
        """중복 트리플 병합"""
        unique_triples = {}
        
        for triple in triples:
            key = (triple.subject.lower(), triple.relation.lower(), triple.object.lower())
            if key in unique_triples:
                # Keep the one with higher confidence
                if triple.confidence > unique_triples[key].confidence:
                    unique_triples[key] = triple
            else:
                unique_triples[key] = triple
        
        return list(unique_triples.values())
