"""
ARI Cognitive Module

인지 루프: 크롤러, 파서, 벡터DB, 지식그래프, 가설생성
"""

from .crawler import ArxivCrawler, SemanticScholarClient, PaperSearchResult
from .parser import PDFParser, PaperMetadata
from .vectordb import ChromaStore, HybridSearcher, get_embedding_model
from .knowledge_graph import KnowledgeGraph, Triple, TripleExtractor
from .ideation import HypothesisGenerator, Hypothesis, NoveltyChecker, LiteratureBasedDiscovery

__all__ = [
    # Crawler
    "ArxivCrawler",
    "SemanticScholarClient",
    "PaperSearchResult",
    # Parser
    "PDFParser",
    "PaperMetadata",
    # VectorDB
    "ChromaStore",
    "HybridSearcher",
    "get_embedding_model",
    # Knowledge Graph
    "KnowledgeGraph",
    "Triple",
    "TripleExtractor",
    # Ideation
    "HypothesisGenerator",
    "Hypothesis",
    "NoveltyChecker",
    "LiteratureBasedDiscovery",
]
