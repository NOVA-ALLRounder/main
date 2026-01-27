"""
Librarian Agent - 학술 자료 검색 및 RAG
Semantic Scholar, ArXiv API 연동
"""
from typing import Dict, Any, List
import json
import os

try:
    from langchain_openai import ChatOpenAI
    from langchain_core.messages import SystemMessage, HumanMessage
    LANGCHAIN_AVAILABLE = True
except ImportError:
    LANGCHAIN_AVAILABLE = False

from state import ScientificState
from tools.arxiv_search import search_arxiv
from tools.semantic_scholar import search_semantic_scholar


LIBRARIAN_SYSTEM_PROMPT = """당신은 학술 문헌 검색 전문가입니다.

주어진 연구 주제에 대해:
1. 관련 키워드를 추출하고
2. 검색된 논문들을 분석하여
3. 핵심 요약을 제공합니다.

검색 결과를 바탕으로 다음을 JSON 형식으로 반환하세요:
{
    "search_queries": ["검색에 사용한 쿼리 1", "쿼리 2"],
    "key_findings": ["핵심 발견 1", "핵심 발견 2"],
    "research_gaps": ["연구 공백 1", "공백 2"],
    "most_relevant_papers": ["가장 관련성 높은 논문 제목들"]
}
"""


class LibrarianAgent:
    """사서 에이전트 - 학술 자료 검색"""
    
    def __init__(self, api_key: str = None):
        self.api_key = api_key or os.getenv("OPENAI_API_KEY", "")
        self.llm = None
        if LANGCHAIN_AVAILABLE and self.api_key:
            self.llm = ChatOpenAI(
                model="gpt-4o-mini",
                temperature=0.3,
                api_key=self.api_key
            )
    
    def search(self, state: ScientificState) -> ScientificState:
        """문헌 검색 수행"""
        user_input = state.get('user_input', '')
        domain = state.get('domain', '')
        
        # 검색 쿼리 생성
        search_queries = self._generate_queries(user_input, domain)
        state['search_queries'] = search_queries
        
        # 학술 API 검색
        all_results = []
        
        for i, query in enumerate(search_queries[:3]):  # 최대 3개 쿼리
            # ArXiv 검색 (제한이 덜함)
            arxiv_results = search_arxiv(query, max_results=5)
            all_results.extend(arxiv_results)
            
            # Semantic Scholar 검색 (첫 번째 쿼리에만 수행하여 429 방지)
            if i == 0:
                ss_results = search_semantic_scholar(query, limit=5)
                all_results.extend(ss_results)
        
        # 중복 제거 및 정렬
        unique_results = self._deduplicate_results(all_results)
        
        state['literature_context'] = unique_results[:15]  # 상위 15개
        state['current_step'] = 'literature_searched'
        
        return state
    
    def _generate_queries(self, user_input: str, domain: str) -> List[str]:
        """검색 쿼리 생성"""
        if self.llm:
            return self._generate_queries_llm(user_input, domain)
        else:
            return self._generate_queries_simple(user_input, domain)
    
    def _generate_queries_llm(self, user_input: str, domain: str) -> List[str]:
        """LLM을 이용한 쿼리 생성"""
        prompt = f"""다음 연구 주제에 대한 학술 검색 쿼리를 3개 생성하세요.
도메인: {domain or '일반 과학'}

주제: {user_input}

영어로 검색 쿼리를 생성하고, JSON 배열로 반환하세요:
["query1", "query2", "query3"]
"""
        messages = [HumanMessage(content=prompt)]
        response = self.llm.invoke(messages)
        
        try:
            content = response.content
            if '[' in content:
                start = content.index('[')
                end = content.rindex(']') + 1
                return json.loads(content[start:end])
        except:
            pass
        
        return self._generate_queries_simple(user_input, domain)
    
    def _generate_queries_simple(self, user_input: str, domain: str) -> List[str]:
        """간단한 쿼리 생성"""
        # 기본 영어 키워드 추출
        queries = [user_input[:100]]
        
        if domain:
            queries.append(f"{domain} {user_input[:50]}")
        
        return queries
    
    def _deduplicate_results(self, results: List[Dict]) -> List[Dict]:
        """결과 중복 제거"""
        seen_titles = set()
        unique = []
        
        for item in results:
            title = item.get('title', '').lower()[:50]
            if title and title not in seen_titles:
                seen_titles.add(title)
                unique.append(item)
        
        # 관련성 점수로 정렬
        unique.sort(key=lambda x: x.get('relevance_score', 0), reverse=True)
        return unique


def librarian_node(state: ScientificState) -> ScientificState:
    """LangGraph 노드 함수"""
    agent = LibrarianAgent()
    return agent.search(state)
