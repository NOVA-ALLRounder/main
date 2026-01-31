# =============================================================================
# T_lab Agent - Librarian (Literature Search)
# Combines Semantic Scholar + arXiv search
# =============================================================================

from typing import Dict, Any, List
import httpx
import asyncio

from state import ScientificState
from core import get_settings, get_logger

settings = get_settings()
logger = get_logger("agents.librarian")


class LibrarianAgent:
    """Literature search agent using Semantic Scholar and arXiv."""
    
    S2_API_URL = "https://api.semanticscholar.org/graph/v1/paper/search"
    ARXIV_API_URL = "http://export.arxiv.org/api/query"
    
    def __init__(self):
        self.max_results = settings.max_literature_results
        self.llm = None
        if "langchain_openai" in globals() or "ChatOpenAI" in globals():
             pass # Already imported? No, need to ensure imports or lazy import
        
        from langchain_openai import ChatOpenAI
        from langchain_core.messages import SystemMessage, HumanMessage
        
        if settings.openai_api_key:
            self.llm = ChatOpenAI(
                model="gpt-4o-mini",
                temperature=0.3,
                api_key=settings.openai_api_key
            )
    
    def search(self, state: ScientificState) -> ScientificState:
        """Search literature based on user input."""
        query = state.get('user_input', '')
        domain = state.get('domain', '')
        
        logger.info("Searching literature", query=query[:50], source="librarian")
        
        # Translate query if necessary (very simple heuristic or LLM based)
        # Using LLM for better keyword extraction
        search_query = self._translate_query(query) if self.llm else query
        state['search_queries'] = [search_query]
        
        logger.info(f"Translated query: {search_query}", source="librarian")
        
        # Search both sources
        s2_results = self._search_semantic_scholar(search_query)
        arxiv_results = self._search_arxiv(search_query)
        
        # Merge and deduplicate
        all_papers = s2_results + arxiv_results
        unique_papers = self._deduplicate(all_papers)
        
        state['literature_context'] = unique_papers[:self.max_results]
        state['search_queries'] = [query]
        state['current_step'] = 'literature_searched'
        
        # Add to logic chain
        state['logic_chain'] = state.get('logic_chain', [])
        state['logic_chain'].append({
            "step": "librarian",
            "papers_found": len(unique_papers),
            "sources": ["semantic_scholar", "arxiv"]
        })
        
        # Mark as completed since this is the end of the 'question' path
        state['status'] = 'completed'
        state['current_step_label'] = '✅ 문헌 검색 완료'
        
        logger.info(f"Found {len(unique_papers)} papers", source="librarian")
        return state
    
    def _search_semantic_scholar(self, query: str) -> List[Dict[str, Any]]:
        """Search Semantic Scholar API."""
        try:
            with httpx.Client(timeout=10.0) as client:
                response = client.get(
                    self.S2_API_URL,
                    params={
                        "query": query,
                        "limit": 10,
                        "fields": "title,abstract,year,externalIds,authors,citationCount"
                    }
                )
                
                if response.status_code != 200:
                    return []
                
                data = response.json()
                papers = []
                
                for p in data.get("data", []):
                    ext_ids = p.get("externalIds", {})
                    papers.append({
                        "title": p.get("title", ""),
                        "abstract": p.get("abstract", "")[:500] if p.get("abstract") else "",
                        "year": p.get("year"),
                        "doi": ext_ids.get("DOI"),
                        "arxiv_id": ext_ids.get("ArXiv"),
                        "citation_count": p.get("citationCount", 0),
                        "source": "semantic_scholar"
                    })
                
                return papers
        except Exception as e:
            logger.warning(f"S2 search failed: {e}", source="librarian")
            return []
    
    def _search_arxiv(self, query: str) -> List[Dict[str, Any]]:
        """Search arXiv API."""
        try:
            with httpx.Client(timeout=10.0) as client:
                response = client.get(
                    self.ARXIV_API_URL,
                    params={
                        "search_query": f"all:{query}",
                        "start": 0,
                        "max_results": 5,
                        "sortBy": "relevance"
                    }
                )
                
                if response.status_code != 200:
                    return []
                
                # Parse XML (simplified)
                import xml.etree.ElementTree as ET
                root = ET.fromstring(response.text)
                ns = {"atom": "http://www.w3.org/2005/Atom"}
                
                papers = []
                for entry in root.findall("atom:entry", ns):
                    arxiv_id = entry.find("atom:id", ns).text.split("/")[-1] if entry.find("atom:id", ns) is not None else None
                    papers.append({
                        "title": entry.find("atom:title", ns).text.strip() if entry.find("atom:title", ns) is not None else "",
                        "abstract": (entry.find("atom:summary", ns).text.strip()[:500] if entry.find("atom:summary", ns) is not None else ""),
                        "year": int(entry.find("atom:published", ns).text[:4]) if entry.find("atom:published", ns) is not None else None,
                        "doi": None,
                        "arxiv_id": arxiv_id,
                        "citation_count": 0,
                        "source": "arxiv"
                    })
                
                return papers
        except Exception as e:
            logger.warning(f"arXiv search failed: {e}", source="librarian")
            return []
    
    def _translate_query(self, query: str) -> str:
        """Translate Korean query to English keywords."""
        if not self.llm:
            return query
            
        try:
            from langchain_core.messages import SystemMessage, HumanMessage
            prompt = f"Convert this search query to optimal English academic keywords for Semantic Scholar/ArXiv. Return ONLY the English keywords, nothing else.\n\nQuery: {query}"
            
            response = self.llm.invoke([HumanMessage(content=prompt)])
            return response.content.strip()
        except Exception as e:
            logger.warning(f"Translation failed: {e}", source="librarian")
            return query

    def _deduplicate(self, papers: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
        """Remove duplicate papers based on title similarity."""
        seen_titles = set()
        unique = []
        
        for p in papers:
            title_key = p.get("title", "").lower()[:50]
            if title_key and title_key not in seen_titles:
                seen_titles.add(title_key)
                unique.append(p)
        
        return unique


def search_literature(state: ScientificState) -> ScientificState:
    """Entry point for librarian agent."""
    agent = LibrarianAgent()
    return agent.search(state)
