from rag.embedder import Embedder
from rag.vector_store import VectorStore
import os
import shutil

def test_integration():
    # Cleanup previous test data
    if os.path.exists("./data/chroma_test"):
        shutil.rmtree("./data/chroma_test")
    
    print("Initializing RAG components...")
    embedder = Embedder()
    store = VectorStore(persist_directory="./data/chroma_test", collection_name="test_collection")
    
    # Sample documents (Tax Law snippets)
    docs = [
        "제1조(목적) 이 법은 법인세의 과세전요건과 절차를 규정함으로써 국세수입을 원활하게 확보하고 공평과세에 이바지함을 목적으로 한다.",
        "제4조(실질과세) 자산이나 사업에서 생기는 수익의 전부 또는 일부가 법률상 귀속되는 법인과 실질상 귀속되는 법인이 서로 다른 경우에는 그 수익이 실질상 귀속되는 법인에게 이 법을 적용한다.",
        "제55조(세율) ① 법인세의 과세표준이 2억원 이하인 경우 세율은 100분의 9로 한다."
    ]
    metadatas = [
        {"source": "법인세법", "article": "1조"},
        {"source": "법인세법", "article": "4조"},
        {"source": "법인세법", "article": "55조"}
    ]
    
    print("Embedding documents...")
    embeddings = embedder.embed_documents(docs)
    
    print("Indexing documents...")
    store.add_documents(docs, metadatas, embeddings)
    
    # Query
    query_text = "법인세율은 얼마인가요?"
    print(f"\nQuerying: {query_text}")
    query_vec = embedder.embed_text(query_text)
    
    results = store.query(query_vec, n_results=3)
    
    found_correct = False
    if results:
        print(f"\nTop {len(results)} Results:")
        for idx, res in enumerate(results):
            print(f"[{idx+1}] Distance: {res['distance']:.4f}")
            print(f"    {res['content'][:100]}...")
            if "100분의 9" in res['content']:
                found_correct = True
    
    if found_correct:
        print("\nSUCCESS: Correct information found in top results.")
    else:
        print("\nFAILURE: Correct information NOT found in top results.")

if __name__ == "__main__":
    test_integration()
