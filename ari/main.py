"""
ARI Main Orchestrator

Autonomous Research Intelligence 메인 오케스트레이터
"""

from typing import Dict, Any, List, Optional
from pathlib import Path
from datetime import datetime
import json

from config import get_config, ARIConfig
from core import LLMClient, get_llm_client, get_logger

# Cognitive Loop
from cognitive.crawler import ArxivCrawler, SemanticScholarClient
from cognitive.parser import PDFParser
from cognitive.vectordb import ChromaStore, HybridSearcher, get_embedding_model
from cognitive.knowledge_graph import KnowledgeGraph, TripleExtractor
from cognitive.ideation import HypothesisGenerator, NoveltyChecker, LiteratureBasedDiscovery, Hypothesis

# Execution Loop
from execution.tree_search import ExperimentTree, TreeSearcher, SearchConfig, SearchStrategy
from execution.coding_agent import CodingAgent
from execution.sandbox import CodeExecutor
from execution.shinka_evolve import ShinkaEvolve

# Publication Loop
from publication.latex_generator import LaTeXGenerator
from publication.visual_critique import VisualCritic
from publication.review import ReviewerAgent, ReviewerPersona, MultiReviewerPanel, RebuttalManager

# Recursive Loop
from recursive import KnowledgeInternalizer, RecursiveDiscovery


logger = get_logger("ari_main")


class ARI:
    """Autonomous Research Intelligence"""
    
    def __init__(
        self,
        config: ARIConfig = None,
        output_dir: str = None
    ):
        """
        ARI 초기화
        
        Args:
            config: ARI 설정
            output_dir: 출력 디렉토리
        """
        self.config = config or get_config()
        self.output_dir = Path(output_dir or self.config.output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)
        
        # Core
        self.llm = get_llm_client()
        
        # Cognitive Loop 컴포넌트
        self._init_cognitive()
        
        # Execution Loop 컴포넌트
        self._init_execution()
        
        # Publication Loop 컴포넌트
        self._init_publication()
        
        # Recursive Loop 컴포넌트
        self._init_recursive()
        
        # 상태
        self.current_hypothesis: Optional[Hypothesis] = None
        self.research_history: List[Dict[str, Any]] = []
        
        logger.info("ARI initialized successfully")
    
    def _init_cognitive(self):
        """Cognitive Loop 초기화"""
        # Crawlers
        self.arxiv_crawler = ArxivCrawler()
        self.semantic_scholar = SemanticScholarClient()
        
        # Parser
        self.pdf_parser = PDFParser()
        
        # Vector DB
        vectordb_path = self.output_dir / "vectordb"
        self.embedding_model = get_embedding_model("openai")
        self.vector_store = ChromaStore(
            collection_name="ari_papers",
            persist_directory=str(vectordb_path),
            embedding_model=self.embedding_model
        )
        self.hybrid_searcher = HybridSearcher(self.vector_store)
        
        # Knowledge Graph
        self.knowledge_graph = KnowledgeGraph()
        self.triple_extractor = TripleExtractor(self.llm)
        
        # Ideation
        self.hypothesis_generator = HypothesisGenerator(self.llm)
        self.novelty_checker = NoveltyChecker(self.semantic_scholar, self.llm)
        self.lbd = LiteratureBasedDiscovery(self.knowledge_graph, self.llm)
    
    def _init_execution(self):
        """Execution Loop 초기화"""
        experiments_dir = self.output_dir / "experiments"
        
        self.coding_agent = CodingAgent(
            llm_client=self.llm,
            working_dir=str(experiments_dir)
        )
        
        self.code_executor = CodeExecutor(
            timeout=self.config.experiment.sandbox_timeout,
            working_dir=str(experiments_dir / "sandbox")
        )
        
        self.shinka_evolve = ShinkaEvolve(llm_client=self.llm)
    
    def _init_publication(self):
        """Publication Loop 초기화"""
        self.latex_generator = LaTeXGenerator(llm_client=self.llm)
        self.visual_critic = VisualCritic(llm_client=self.llm)
        self.reviewer_panel = MultiReviewerPanel(llm_client=self.llm)
        self.rebuttal_manager = RebuttalManager(llm_client=self.llm)
    
    def _init_recursive(self):
        """Recursive Loop 초기화"""
        self.internalizer = KnowledgeInternalizer(
            vector_store=self.vector_store,
            knowledge_graph=self.knowledge_graph
        )
        self.recursive_discovery = RecursiveDiscovery(llm_client=self.llm)
    
    # =========================================================================
    # =========================================================================
    # Cognitive Loop API
    # =========================================================================
    
    def search_literature(
        self,
        query: str,
        max_results: int = 20
    ) -> List[Dict[str, Any]]:
        """문헌 검색"""
        import asyncio
        
        # 0. 쿼리 번역/정제 (영어 검색 최적화)
        english_query = query
        try:
            # 한글이 포함되어 있는지 간단히 체크
            has_korean = any(ord('가') <= ord(c) <= ord('힣') for c in query)
            if has_korean:
                logger.info(f"Translating query to English: {query}")
                english_query = self.llm.complete(
                    prompt=f"Translate the following research topic into a concise English keywords for academic database search (arXiv). Output ONLY the keywords.\n\nTopic: {query}",
                    temperature=0.3
                ).strip().strip('"')
                logger.info(f"Translated query: {english_query}")
        except Exception as e:
            logger.warning(f"Query translation failed: {e}")
            english_query = query

        results = []
        
        async def _run_search():
            tasks = []
            
            # arXiv 검색 (영어 쿼리 사용)
            tasks.append(self.arxiv_crawler.search(english_query, max_results=max_results // 2))
            
            # Semantic Scholar 검색 (영어 쿼리 사용)
            tasks.append(self.semantic_scholar.search(english_query, max_results=max_results // 2))
            
            return await asyncio.gather(*tasks, return_exceptions=True)
        
        try:
            search_results = asyncio.run(_run_search())
            
            # ArXiv 결과 처리
            if isinstance(search_results[0], list):
                for r in search_results[0]:
                    results.append({
                        "source": "arxiv",
                        "title": r.title,
                        "abstract": r.abstract,
                        "authors": r.authors,
                        "url": r.url,
                        "published": r.published_date
                    })
            
            # Semantic Scholar 결과 처리
            if isinstance(search_results[1], list):
                for r in search_results[1]:
                    results.append({
                        "source": "semantic_scholar",
                        "title": r.title,
                        "abstract": r.abstract,
                        "authors": r.authors,
                        "url": r.url,
                        "citations": r.citation_count
                    })
                    
        except Exception as e:
            logger.error(f"Search failed: {e}")
            
        return results
    
    def generate_hypothesis(
        self,
        research_context: str,
        domain: str = "Machine Learning"
    ) -> Hypothesis:
        """가설 생성"""
        hypotheses = self.hypothesis_generator.generate(
            context=research_context,
            domain=domain,
            num_hypotheses=1
        )
        
        if hypotheses:
            self.current_hypothesis = hypotheses[0]
            return hypotheses[0]
        
        return None
    
    async def check_novelty(self, hypothesis_text: str) -> Dict[str, Any]:
        """가설 신규성 검증"""
        result = await self.novelty_checker.check_novelty(hypothesis_text)
        return result.to_dict()
    
    # =========================================================================
    # Execution Loop API
    # =========================================================================
    
    def run_experiment(
        self,
        hypothesis: Hypothesis,
        max_iterations: int = 10
    ) -> Dict[str, Any]:
        """
        실험 실행
        
        Args:
            hypothesis: 실행할 가설
            max_iterations: 최대 반복 횟수
        
        Returns:
            실험 결과
        """
        # 실험 코드 생성
        experiment_code = self.coding_agent.create_experiment(
            hypothesis=hypothesis.to_dict(),
            experiment_plan=hypothesis.experiment_plan
        )
        
        # 폴더명: 가설 제목을 안전한 파일명으로 변환
        from core.utils import sanitize_filename
        folder_name = sanitize_filename(hypothesis.title)[:80]
        if not folder_name:
            folder_name = hypothesis.hypothesis_id
        
        # 코드 저장
        exp_dir = self.coding_agent.save_experiment(
            experiment_code,
            experiment_id=folder_name
        )
        
        # 트리 탐색 설정
        tree = ExperimentTree(root_description=hypothesis.title)
        
        config = SearchConfig(
            strategy=SearchStrategy.BEST_FIRST,
            max_depth=5,
            max_nodes=max_iterations
        )
        
        searcher = TreeSearcher(tree, config)
        
        # 실행 함수
        def executor(node):
            code = experiment_code.main_code.code
            
            if node.code_changes:
                modified = self.coding_agent.modify_for_node(code, {
                    "description": node.description,
                    "code_changes": node.code_changes,
                    "config_changes": node.config_changes
                })
                code = modified.code
            
            result = self.code_executor.execute(code)
            
            return {
                "success": result.success,
                "metrics": result.metrics,
                "output": result.stdout,
                "error": result.stderr
            }
        
        searcher.set_executor(executor)
        
        # 변형 생성 함수
        def child_generator(node):
            variants = self.coding_agent.generate_variants(
                experiment_code.main_code.code,
                num_variants=3
            )
            
            return [
                {
                    "description": v.description,
                    "code_changes": v.code,
                    "config_changes": {}
                }
                for v in variants
            ]
        
        # 탐색 실행
        best_node = searcher.search(child_generator, max_iterations)
        
        result = {
            "hypothesis_id": hypothesis.hypothesis_id,
            "best_score": best_node.score if best_node else 0,
            "best_metrics": best_node.metrics if best_node else {},
            "tree_stats": tree.stats(),
            "experiment_dir": exp_dir
        }
        
        return result
    
    # =========================================================================
    # Publication Loop API
    # =========================================================================
    
    def write_paper(
        self,
        hypothesis: Hypothesis,
        experiment_results: Dict[str, Any],
        related_papers: List[Dict[str, Any]] = None,
        compile_pdf: bool = True
    ) -> Dict[str, Any]:
        """
        논문 작성
        
        Args:
            hypothesis: 가설
            experiment_results: 실험 결과
            related_papers: 관련 논문들
            compile_pdf: PDF로 컴파일 여부
        
        Returns:
            생성된 논문 정보
        """
        from core.utils import sanitize_filename
        
        paper = self.latex_generator.generate_paper(
            title=hypothesis.title,
            hypothesis=hypothesis.to_dict(),
            experiment_results=experiment_results,
            related_papers=related_papers or []
        )
        
        # 폴더명: 가설 제목을 안전한 파일명으로 변환
        folder_name = sanitize_filename(hypothesis.title)[:80]  # 최대 80자
        if not folder_name:
            folder_name = hypothesis.hypothesis_id
        
        # 저장
        paper_dir = self.output_dir / "papers" / folder_name
        files = paper.save_all(str(paper_dir))
        
        # PDF 컴파일
        pdf_path = None
        if compile_pdf:
            logger.info("Compiling paper to PDF...")
            pdf_path = self.latex_generator.compile_to_pdf(paper, str(paper_dir))
            if pdf_path:
                files["pdf"] = pdf_path
                logger.info(f"PDF generated: {pdf_path}")
            else:
                logger.warning("PDF compilation failed. LaTeX output saved.")
        
        return {
            "title": paper.title,
            "files": files,
            "sections": [s.title for s in paper.sections],
            "pdf_path": pdf_path
        }
    
    def review_paper(
        self,
        title: str,
        abstract: str,
        sections: Dict[str, str]
    ) -> List[Dict[str, Any]]:
        """논문 리뷰"""
        reviews = self.reviewer_panel.review_paper(title, abstract, sections)
        return [r.to_dict() for r in reviews]
    
    # =========================================================================
    # Recursive Loop API
    # =========================================================================
    
    def internalize_results(
        self,
        title: str,
        abstract: str,
        sections: Dict[str, str],
        metrics: Dict[str, float] = None
    ):
        """결과 내재화"""
        return self.internalizer.internalize_paper(
            title=title,
            abstract=abstract,
            sections=sections,
            metrics=metrics,
            source="generated"
        )
    
    def get_next_research_seed(self):
        """다음 연구 시드 가져오기"""
        return self.recursive_discovery.get_next_research_cycle()
    
    # =========================================================================
    # Full Pipeline
    # =========================================================================
    
    def full_research_cycle(
        self,
        research_topic: str,
        domain: str = "Machine Learning"
    ) -> Dict[str, Any]:
        """
        전체 연구 사이클 실행
        
        Args:
            research_topic: 연구 주제
            domain: 연구 분야
        
        Returns:
            연구 결과
        """
        cycle_start = datetime.now()
        
        logger.info(f"Starting full research cycle: {research_topic}")
        
        # 1. 문헌 검색
        logger.info("Step 1: Searching literature...")
        papers = self.search_literature(research_topic)
        
        # 2. 가설 생성
        logger.info("Step 2: Generating hypothesis...")
        context = "\n".join([
            f"- {p['title']}: {p.get('abstract', '')[:200]}"
            for p in papers[:10]
        ])
        hypothesis = self.generate_hypothesis(context, domain)
        
        if not hypothesis:
            return {"error": "Failed to generate hypothesis"}
        
        # 3. 실험 실행
        logger.info("Step 3: Running experiments...")
        experiment_results = self.run_experiment(hypothesis)
        
        # 4. 논문 작성
        logger.info("Step 4: Writing paper...")
        paper_info = self.write_paper(hypothesis, experiment_results, papers[:5])
        
        # 5. 리뷰
        logger.info("Step 5: Reviewing paper...")
        # 섹션 딕셔너리 구성이 필요함
        
        # 6. 내재화
        logger.info("Step 6: Internalizing results...")
        # TODO: 적절한 섹션 전달
        
        cycle_end = datetime.now()
        
        result = {
            "topic": research_topic,
            "hypothesis": hypothesis.to_dict(),
            "experiment_results": experiment_results,
            "paper": paper_info,
            "duration": (cycle_end - cycle_start).total_seconds(),
            "timestamp": cycle_start.isoformat()
        }
        
        self.research_history.append(result)
        
        logger.info(f"Research cycle completed in {result['duration']:.1f}s")
        
        return result
    
    def save_state(self):
        """상태 저장"""
        state_dir = self.output_dir / "state"
        self.internalizer.save_state(str(state_dir))
        
        # 연구 히스토리 저장
        with open(state_dir / "research_history.json", 'w') as f:
            json.dump(self.research_history, f, indent=2, default=str)
        
        logger.info("State saved")
    
    def load_state(self):
        """상태 로드"""
        state_dir = self.output_dir / "state"
        
        if state_dir.exists():
            self.internalizer.load_state(str(state_dir))
            
            history_file = state_dir / "research_history.json"
            if history_file.exists():
                with open(history_file) as f:
                    self.research_history = json.load(f)
            
            logger.info("State loaded")
    
    def stats(self) -> Dict[str, Any]:
        """시스템 통계"""
        return {
            "vector_store_count": self.vector_store.count(),
            "knowledge_graph": self.knowledge_graph.stats(),
            "internalizer": self.internalizer.stats(),
            "recursive_discovery": self.recursive_discovery.stats(),
            "research_cycles": len(self.research_history)
        }


def main():
    """CLI 엔트리포인트"""
    import argparse
    
    parser = argparse.ArgumentParser(description="ARI - Autonomous Research Intelligence")
    parser.add_argument("--topic", type=str, help="Research topic")
    parser.add_argument("--domain", type=str, help="Research domain")
    parser.add_argument("--output", type=str, default="./output", help="Output directory")
    
    args = parser.parse_args()
    
    print("=" * 60)
    print("  ARI - Autonomous Research Intelligence")
    print("  자율 과학 연구 에이전트")
    print("=" * 60)
    print()
    
    ari = ARI(output_dir=args.output)
    
    # 대화형 입력 모드
    topic = args.topic
    domain = args.domain
    
    if not topic:
        topic = input("📚 연구 주제를 입력하세요 (Research Topic): ").strip()
        if not topic:
            print("❌ 연구 주제가 입력되지 않았습니다.")
            return
    
    if not domain:
        domain = input("🔬 연구 도메인을 입력하세요 (기본값: Machine Learning): ").strip()
        if not domain:
            domain = "Machine Learning"
    
    print()
    print(f"📝 연구 주제: {topic}")
    print(f"🔬 연구 도메인: {domain}")
    print()
    
    confirm = input("위 설정으로 연구를 시작하시겠습니까? (y/n): ").strip().lower()
    if confirm != 'y':
        print("❌ 연구가 취소되었습니다.")
        return
    
    print()
    print("🚀 연구 사이클을 시작합니다...")
    print("-" * 60)
    
    result = ari.full_research_cycle(topic, domain)
    
    print()
    print("=" * 60)
    print("✅ 연구 완료!")
    print("=" * 60)
    print()
    print(f"📄 논문 제목: {result.get('hypothesis', {}).get('title', 'N/A')}")
    print(f"⏱️  소요 시간: {result.get('duration', 0):.1f}초")
    
    paper_info = result.get('paper', {})
    files = paper_info.get('files', {})
    if files.get('pdf'):
        print(f"📁 PDF 위치: {files['pdf']}")
    elif files.get('tex'):
        print(f"📁 LaTeX 위치: {files['tex']}")
    
    print()
    print("상세 결과:")
    print(json.dumps(result, indent=2, default=str, ensure_ascii=False))


if __name__ == "__main__":
    main()

