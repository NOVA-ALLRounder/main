"""
Knowledge Internalizer

생성된 논문을 지식 베이스로 내재화
"""

from typing import Dict, Any, List, Optional
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
import json

import sys
import os
sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from core.logger import get_logger
from cognitive.vectordb import ChromaStore
from cognitive.knowledge_graph import KnowledgeGraph, TripleExtractor, Triple
from cognitive.parser import PaperMetadata

logger = get_logger("internalization")


@dataclass
class InternalizedPaper:
    """내재화된 논문"""
    paper_id: str
    title: str
    
    # 임베딩 정보
    vectordb_ids: List[str]
    
    # 지식 그래프 정보
    triples_added: int
    
    # 메타데이터
    internalized_at: datetime
    source: str  # generated, external
    
    # SOTA 업데이트
    sota_updates: Dict[str, float] = None


class KnowledgeInternalizer:
    """지식 내재화 시스템"""
    
    def __init__(
        self,
        vector_store: ChromaStore = None,
        knowledge_graph: KnowledgeGraph = None,
        sota_tracker: Dict[str, float] = None
    ):
        self.vector_store = vector_store
        self.knowledge_graph = knowledge_graph or KnowledgeGraph()
        self.sota_tracker = sota_tracker or {}  # {metric_name: best_value}
        
        self.triple_extractor = TripleExtractor()
        self.internalized_papers: List[InternalizedPaper] = []
        
        # 실패한 실험 아카이브
        self.negative_results: List[Dict[str, Any]] = []
    
    def internalize_paper(
        self,
        title: str,
        abstract: str,
        sections: Dict[str, str],
        metrics: Dict[str, float] = None,
        paper_id: str = None,
        source: str = "generated"
    ) -> InternalizedPaper:
        """
        논문 내재화
        
        Args:
            title: 논문 제목
            abstract: 초록
            sections: 섹션 딕셔너리
            metrics: 성능 지표
            paper_id: 논문 ID
            source: 소스 (generated, external)
        
        Returns:
            InternalizedPaper 객체
        """
        import hashlib
        
        paper_id = paper_id or hashlib.md5(title.encode()).hexdigest()[:12]
        
        vectordb_ids = []
        triples_added = 0
        sota_updates = {}
        
        # 1. 벡터 DB에 추가
        if self.vector_store:
            # 초록 추가
            ids = self.vector_store.add(
                documents=[abstract],
                metadatas=[{
                    "paper_id": paper_id,
                    "title": title,
                    "type": "abstract",
                    "source": source
                }],
                ids=[f"{paper_id}_abstract"]
            )
            vectordb_ids.extend(ids)
            
            # 각 섹션 추가
            for section_name, content in sections.items():
                if len(content) > 100:  # 너무 짧은 섹션 제외
                    ids = self.vector_store.add(
                        documents=[content[:5000]],  # 길이 제한
                        metadatas=[{
                            "paper_id": paper_id,
                            "title": title,
                            "type": "section",
                            "section": section_name,
                            "source": source
                        }],
                        ids=[f"{paper_id}_{section_name}"]
                    )
                    vectordb_ids.extend(ids)
        
        # 2. 지식 그래프에 트리플 추가
        combined_text = f"Title: {title}\n\nAbstract: {abstract}"
        triples = self.triple_extractor.extract(combined_text, source=paper_id)
        
        for triple in triples:
            self.knowledge_graph.add_triple(triple)
            triples_added += 1
        
        # 3. SOTA 업데이트
        if metrics:
            sota_updates = self._update_sota(metrics, paper_id)
        
        internalized = InternalizedPaper(
            paper_id=paper_id,
            title=title,
            vectordb_ids=vectordb_ids,
            triples_added=triples_added,
            internalized_at=datetime.now(),
            source=source,
            sota_updates=sota_updates
        )
        
        self.internalized_papers.append(internalized)
        
        logger.info(f"Internalized paper '{title}': {len(vectordb_ids)} chunks, {triples_added} triples")
        
        return internalized
    
    def _update_sota(
        self,
        metrics: Dict[str, float],
        paper_id: str
    ) -> Dict[str, float]:
        """SOTA 벤치마크 업데이트"""
        updates = {}
        
        # 높을수록 좋은 지표
        higher_better = ["accuracy", "f1", "precision", "recall", "auc", "score"]
        # 낮을수록 좋은 지표
        lower_better = ["loss", "error", "perplexity", "mse", "mae"]
        
        for metric, value in metrics.items():
            metric_lower = metric.lower()
            
            if metric not in self.sota_tracker:
                self.sota_tracker[metric] = value
                updates[metric] = value
                continue
            
            current_best = self.sota_tracker[metric]
            
            is_higher_better = any(hb in metric_lower for hb in higher_better)
            is_lower_better = any(lb in metric_lower for lb in lower_better)
            
            if is_higher_better and value > current_best:
                self.sota_tracker[metric] = value
                updates[metric] = value
                logger.info(f"New SOTA for {metric}: {value} (was {current_best})")
            
            elif is_lower_better and value < current_best:
                self.sota_tracker[metric] = value
                updates[metric] = value
                logger.info(f"New SOTA for {metric}: {value} (was {current_best})")
        
        return updates
    
    def archive_negative_result(
        self,
        hypothesis: Dict[str, Any],
        experiment_results: Dict[str, Any],
        failure_reason: str
    ):
        """
        실패한 실험 아카이빙 (반면교사 데이터)
        
        Args:
            hypothesis: 가설 정보
            experiment_results: 실험 결과
            failure_reason: 실패 원인
        """
        negative = {
            "hypothesis": hypothesis,
            "results": experiment_results,
            "failure_reason": failure_reason,
            "archived_at": datetime.now().isoformat()
        }
        
        self.negative_results.append(negative)
        
        logger.info(f"Archived negative result: {failure_reason}")
    
    def get_similar_failures(
        self,
        hypothesis: str,
        top_k: int = 5
    ) -> List[Dict[str, Any]]:
        """
        유사한 실패 사례 검색 (반복 방지)
        
        Args:
            hypothesis: 현재 가설
            top_k: 반환할 최대 개수
        
        Returns:
            유사한 실패 사례 리스트
        """
        if not self.negative_results:
            return []
        
        # 간단한 키워드 매칭
        hypothesis_lower = hypothesis.lower()
        
        scored_results = []
        for result in self.negative_results:
            result_hypothesis = str(result.get("hypothesis", {})).lower()
            
            # 단어 겹침 계산
            h_words = set(hypothesis_lower.split())
            r_words = set(result_hypothesis.split())
            overlap = len(h_words & r_words)
            
            if overlap > 0:
                scored_results.append((overlap, result))
        
        scored_results.sort(key=lambda x: x[0], reverse=True)
        
        return [r for _, r in scored_results[:top_k]]
    
    def get_sota(self, metric: str = None) -> Dict[str, float]:
        """SOTA 벤치마크 조회"""
        if metric:
            return {metric: self.sota_tracker.get(metric, 0.0)}
        return dict(self.sota_tracker)
    
    def save_state(self, output_dir: str):
        """상태 저장"""
        output_path = Path(output_dir)
        output_path.mkdir(parents=True, exist_ok=True)
        
        # 지식 그래프 저장
        self.knowledge_graph.save(str(output_path / "knowledge_graph.json"))
        
        # SOTA 저장
        with open(output_path / "sota_tracker.json", 'w') as f:
            json.dump(self.sota_tracker, f, indent=2)
        
        # 실패 사례 저장
        with open(output_path / "negative_results.json", 'w') as f:
            json.dump(self.negative_results, f, indent=2, default=str)
        
        logger.info(f"State saved to {output_dir}")
    
    def load_state(self, input_dir: str):
        """상태 로드"""
        input_path = Path(input_dir)
        
        # 지식 그래프 로드
        kg_file = input_path / "knowledge_graph.json"
        if kg_file.exists():
            self.knowledge_graph = KnowledgeGraph.load(str(kg_file))
        
        # SOTA 로드
        sota_file = input_path / "sota_tracker.json"
        if sota_file.exists():
            with open(sota_file) as f:
                self.sota_tracker = json.load(f)
        
        # 실패 사례 로드
        neg_file = input_path / "negative_results.json"
        if neg_file.exists():
            with open(neg_file) as f:
                self.negative_results = json.load(f)
        
        logger.info(f"State loaded from {input_dir}")
    
    def stats(self) -> Dict[str, Any]:
        """통계"""
        return {
            "internalized_papers": len(self.internalized_papers),
            "knowledge_graph": self.knowledge_graph.stats(),
            "sota_metrics": len(self.sota_tracker),
            "negative_results": len(self.negative_results)
        }
