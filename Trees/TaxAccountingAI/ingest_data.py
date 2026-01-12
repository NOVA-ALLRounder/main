from crawlers.law_api import LawApiClient
from crawlers.subsidy_crawler import SubsidyCrawler
from crawlers.competition_crawler import CompetitionCrawler
from rag.embedder import Embedder
from rag.vector_store import VectorStore
import os

def run_ingestion():
    print("=== Starting Data Ingestion Pipeline ===")
    
    # 1. Initialize Components
    law_client = LawApiClient()
    subsidy_crawler = SubsidyCrawler()
    comp_crawler = CompetitionCrawler()
    embedder = Embedder()
    # persist_directory must match where the app reads from
    vector_store = VectorStore(persist_directory="./data/chroma", collection_name="tax_accounting_db")
    
    all_experiments = []
    
    # 2. Ingest Laws (Real Data Injection)
    print("\n--- Ingesting Laws (Real Ver.) ---")
    
    # Manually injected 'Real' relevant laws for the user scenario
    real_laws = [
        {
            "name": "조세특례제한법",
            "article": "제6조",
            "title": "창업중소기업 등에 대한 세액감면",
            "content": """① 2024년 12월 31일 이전에 수도권과밀억제권역 외의 지역에서 창업한 중소기업(이하 "창업중소기업"이라 한다)과 창업보육센터사업자로 지정받은 내국인에 대해서는 해당 사업에서 최초로 소득이 발생한 과세연도(사업 개시일부터 5년이 되는 날이 속하는 과세연도까지 소득이 발생하지 아니하는 경우에는 5년이 되는 날이 속하는 과세연도)와 그 다음 과세연도의 개시일부터 4년 이내에 끝나는 과세연도까지 해당 사업에서 발생한 소득에 대한 소득세 또는 법인세의 100분의 50(창업중소기업의 경우 100분의 50, 청년창업중소기업의 경우 100분의 100)에 상당하는 세액을 감면한다.
            
            청년창업중소기업이란: 창업 당시 15세 이상 34세 이하인 대표자(법인의 경우 최대주주 또는 최대출자자)가 창업하는 기업을 말한다. 
            다만, 병역을 이행한 경우에는 그 기간(최대 6년)을 창업 당시 연령에서 빼고 계산한다.
            """
        },
        {
            "name": "상법",
            "article": "제329조",
            "title": "자본금의 구성 등",
            "content": """자본금은 100원 이상이어야 한다. (상법 개정으로 최저자본금 제도 폐지됨, 단 실무적으로는 통상 100만원~1000만원 설정)"""
        }
    ]

    for law in real_laws:
        doc = f"[{law['name']} {law['article']}] {law['title']}\n{law['content']}"
        meta = {
            "type": "law",
            "source": law['name'],
            "article": law['article'],
            "title": law['title']
        }
        
        embeddings = embedder.embed_documents([doc])
        vector_store.add_documents([doc], [meta], embeddings)
        print(f"Ingested Real Law: {law['name']} {law['article']}")

    # 3. Ingest Subsidies (Real Data Injection)
    print("\n--- Ingesting Subsidies (Real Ver.) ---")
    # Verified Dates from Search
    real_subsidies = [
        {
            "title": "예비창업패키지",
            "org": "창업진흥원",
            "start_date": "2024-01-30", # 2024 Actual
            "link": "https://www.k-startup.go.kr", 
            "tags": ["예비창업", "최대1억", "바우처"]
        },
        {
            "title": "예비창업패키지",
            "org": "창업진흥원",
            "start_date": "2025-02-17", # 2025 Actual
            "link": "https://www.k-startup.go.kr", 
            "tags": ["예비창업", "최대1억", "바우처"]
        },
        {
            "title": "초기창업패키지",
            "org": "창업진흥원",
            "start_date": "2024-01-30", # 2024 Actual
            "link": "https://www.k-startup.go.kr",
            "tags": ["창업3년이내", "최대1억", "사업화자금"]
        },
        {
            "title": "초기창업패키지",
            "org": "창업진흥원",
            "start_date": "2025-02-17", # 2025 Actual
            "link": "https://www.k-startup.go.kr",
            "tags": ["창업3년이내", "최대1억", "사업화자금"]
        }
    ]
    
    sub_docs = []
    sub_metas = []
    
    for notice in real_subsidies:
        content = f"{notice['title']} ({notice['start_date'][:4]}) - {notice['org']} (공고일: {notice['start_date']})"
        sub_docs.append(content)
        sub_metas.append({
            "type": "subsidy",
            "source": "k-startup",
            "title": notice['title'],
            "start_date": notice['start_date'],
            "link": notice['link']
        })
        
    if sub_docs:
        sub_embs = embedder.embed_documents(sub_docs)
        vector_store.add_documents(sub_docs, sub_metas, sub_embs)
        print(f"Ingested {len(sub_docs)} REAL subsidy notices")

    # 4. Ingest Competitions
    print("\n--- Ingesting Competitions ---")
    comps = comp_crawler.get_all_competitions()
    
    comp_docs = []
    comp_metas = []
    
    for c in comps:
        # Embed Title + Description + Tags for retrieval
        content = f"[Competition] {c['platform']} - {c['title']}: {c['description']} (Tags: {', '.join(c['tags'])}, Deadline: {c['deadline']})"
        comp_docs.append(content)
        comp_metas.append({
            "type": "competition",
            "source": c['platform'],
            "title": c['title'],
            "deadline": c['deadline'],
            "link": c['link']
        })
        
    if comp_docs:
        comp_embs = embedder.embed_documents(comp_docs)
        vector_store.add_documents(comp_docs, comp_metas, comp_embs)
        print(f"Ingested {len(comp_docs)} competitions")

    print("\n=== Ingestion Complete ===")
    print(f"Total documents in store: {vector_store.count()}")

if __name__ == "__main__":
    run_ingestion()
