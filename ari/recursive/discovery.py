"""
Recursive Discovery

논문의 Future Work에서 새로운 가설 시드 생성
"""

from typing import Dict, Any, List, Optional
from dataclasses import dataclass, field
from datetime import datetime
from queue import PriorityQueue

import sys
import os
sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from core.llm import LLMClient, get_llm_client
from core.logger import get_logger
from cognitive.ideation import Hypothesis, HypothesisGenerator

logger = get_logger("discovery")


@dataclass
class ResearchSeed:
    """연구 시드"""
    seed_id: str
    
    # 원본 정보
    source_paper_id: str
    source_section: str  # future_work, conclusion, limitation
    original_text: str
    
    # 생성된 가설
    hypothesis: Optional[Hypothesis] = None
    
    # 우선순위 및 상태
    priority: float = 0.5  # 0-1
    status: str = "pending"  # pending, in_progress, completed, rejected
    
    # 메타데이터
    created_at: datetime = field(default_factory=datetime.now)
    
    def __lt__(self, other):
        # PriorityQueue용 비교 (높은 우선순위 먼저)
        return self.priority > other.priority


class ResearchQueue:
    """연구 큐"""
    
    def __init__(self):
        self._queue: PriorityQueue = PriorityQueue()
        self._all_seeds: Dict[str, ResearchSeed] = {}
    
    def add(self, seed: ResearchSeed):
        """시드 추가"""
        self._all_seeds[seed.seed_id] = seed
        self._queue.put(seed)
    
    def get_next(self) -> Optional[ResearchSeed]:
        """다음 시드 가져오기"""
        while not self._queue.empty():
            seed = self._queue.get()
            if seed.status == "pending":
                seed.status = "in_progress"
                return seed
        return None
    
    def mark_completed(self, seed_id: str):
        """완료 표시"""
        if seed_id in self._all_seeds:
            self._all_seeds[seed_id].status = "completed"
    
    def mark_rejected(self, seed_id: str):
        """거부 표시"""
        if seed_id in self._all_seeds:
            self._all_seeds[seed_id].status = "rejected"
    
    def get_pending(self) -> List[ResearchSeed]:
        """대기 중인 시드들"""
        return [s for s in self._all_seeds.values() if s.status == "pending"]
    
    def size(self) -> int:
        """큐 크기"""
        return len(self.get_pending())
    
    def get_all(self) -> List[ResearchSeed]:
        """모든 시드"""
        return list(self._all_seeds.values())


class RecursiveDiscovery:
    """재귀적 발견 시스템"""
    
    EXTRACTION_PROMPT = """Analyze the following paper sections to identify potential future research directions.

Paper Title: {title}

Conclusion:
{conclusion}

Future Work / Limitations:
{future_work}

Extract specific, actionable research directions. For each, provide:
1. A clear research question
2. Why it's important
3. How feasible it is to investigate

Return as JSON:
{{
    "research_directions": [
        {{
            "question": "Clear research question",
            "importance": "Why this matters",
            "feasibility": "How to approach this",
            "priority": 0.0-1.0
        }}
    ]
}}
"""
    
    def __init__(
        self,
        llm_client: Optional[LLMClient] = None,
        max_queue_size: int = 100
    ):
        self.llm = llm_client or get_llm_client()
        self.hypothesis_generator = HypothesisGenerator(self.llm)
        self.research_queue = ResearchQueue()
        self.max_queue_size = max_queue_size
        
        # 발견 히스토리
        self.discovery_history: List[Dict[str, Any]] = []
    
    def extract_seeds_from_paper(
        self,
        paper_id: str,
        title: str,
        sections: Dict[str, str]
    ) -> List[ResearchSeed]:
        """
        논문에서 연구 시드 추출
        
        Args:
            paper_id: 논문 ID
            title: 논문 제목
            sections: 섹션 딕셔너리
        
        Returns:
            추출된 연구 시드 리스트
        """
        import hashlib
        
        # 관련 섹션 찾기
        conclusion = ""
        future_work = ""
        
        for section_name, content in sections.items():
            name_lower = section_name.lower()
            if "conclusion" in name_lower:
                conclusion = content
            elif "future" in name_lower or "limitation" in name_lower:
                future_work = content
        
        if not conclusion and not future_work:
            # 마지막 섹션 사용
            sections_list = list(sections.values())
            if sections_list:
                conclusion = sections_list[-1]
        
        prompt = self.EXTRACTION_PROMPT.format(
            title=title,
            conclusion=conclusion[:2000],
            future_work=future_work[:2000]
        )
        
        seeds = []
        
        try:
            response = self.llm.generate_json(
                prompt=prompt,
                system_prompt="You are identifying promising future research directions.",
                temperature=0.5
            )
            
            for i, direction in enumerate(response.get("research_directions", [])):
                seed_id = hashlib.md5(
                    f"{paper_id}_{i}_{direction.get('question', '')}".encode()
                ).hexdigest()[:10]
                
                seed = ResearchSeed(
                    seed_id=seed_id,
                    source_paper_id=paper_id,
                    source_section="future_work",
                    original_text=direction.get("question", ""),
                    priority=float(direction.get("priority", 0.5)),
                    status="pending"
                )
                
                seeds.append(seed)
        
        except Exception as e:
            logger.error(f"Seed extraction failed: {e}")
        
        return seeds
    
    def process_completed_paper(
        self,
        paper_id: str,
        title: str,
        sections: Dict[str, str],
        auto_queue: bool = True
    ) -> List[ResearchSeed]:
        """
        완료된 논문 처리 (재귀적 사이클의 시작점)
        
        Args:
            paper_id: 논문 ID
            title: 논문 제목
            sections: 섹션 딕셔너리
            auto_queue: 자동으로 큐에 추가할지
        
        Returns:
            생성된 연구 시드 리스트
        """
        seeds = self.extract_seeds_from_paper(paper_id, title, sections)
        
        if auto_queue:
            for seed in seeds:
                if self.research_queue.size() < self.max_queue_size:
                    self.research_queue.add(seed)
        
        # 히스토리 기록
        self.discovery_history.append({
            "paper_id": paper_id,
            "title": title,
            "seeds_generated": len(seeds),
            "timestamp": datetime.now().isoformat()
        })
        
        logger.info(f"Extracted {len(seeds)} research seeds from '{title}'")
        
        return seeds
    
    def generate_hypothesis_for_seed(
        self,
        seed: ResearchSeed,
        domain: str = "Machine Learning"
    ) -> Hypothesis:
        """
        시드에서 가설 생성
        
        Args:
            seed: 연구 시드
            domain: 연구 분야
        
        Returns:
            생성된 가설
        """
        context = f"""
Based on this research direction from a previous paper:

Research Question: {seed.original_text}

Generate a detailed, actionable hypothesis that can be experimentally verified.
"""
        
        hypotheses = self.hypothesis_generator.generate(
            context=context,
            domain=domain,
            num_hypotheses=1
        )
        
        if hypotheses:
            seed.hypothesis = hypotheses[0]
            return hypotheses[0]
        
        return None
    
    def get_next_research_cycle(self) -> Optional[ResearchSeed]:
        """
        다음 연구 사이클 시작
        
        Returns:
            다음으로 연구할 시드 또는 None
        """
        seed = self.research_queue.get_next()
        
        if seed:
            logger.info(f"Starting research cycle for seed: {seed.original_text[:50]}...")
        
        return seed
    
    def complete_research_cycle(
        self,
        seed: ResearchSeed,
        success: bool,
        results: Dict[str, Any] = None
    ):
        """
        연구 사이클 완료
        
        Args:
            seed: 완료된 시드
            success: 성공 여부
            results: 결과 정보
        """
        if success:
            self.research_queue.mark_completed(seed.seed_id)
            logger.info(f"Completed research cycle for seed {seed.seed_id}")
        else:
            self.research_queue.mark_rejected(seed.seed_id)
            logger.info(f"Rejected research cycle for seed {seed.seed_id}")
    
    def get_research_agenda(self) -> List[Dict[str, Any]]:
        """
        연구 아젠다 반환 (대기 중인 모든 시드)
        
        Returns:
            연구 아젠다 리스트
        """
        pending = self.research_queue.get_pending()
        
        return [
            {
                "seed_id": seed.seed_id,
                "question": seed.original_text,
                "priority": seed.priority,
                "source_paper": seed.source_paper_id,
                "has_hypothesis": seed.hypothesis is not None
            }
            for seed in sorted(pending, key=lambda s: s.priority, reverse=True)
        ]
    
    def stats(self) -> Dict[str, Any]:
        """통계"""
        all_seeds = self.research_queue.get_all()
        
        return {
            "total_seeds": len(all_seeds),
            "pending": len([s for s in all_seeds if s.status == "pending"]),
            "in_progress": len([s for s in all_seeds if s.status == "in_progress"]),
            "completed": len([s for s in all_seeds if s.status == "completed"]),
            "rejected": len([s for s in all_seeds if s.status == "rejected"]),
            "discovery_cycles": len(self.discovery_history)
        }
