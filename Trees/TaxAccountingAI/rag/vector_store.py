import chromadb
from chromadb.config import Settings
import uuid
from typing import List, Dict, Optional

class VectorStore:
    def __init__(self, collection_name="tax_law", persist_directory="./data/chroma"):
        """
        Initialize ChromaDB client and collection.
        """
        self.client = chromadb.PersistentClient(path=persist_directory)
        self.collection = self.client.get_or_create_collection(
            name=collection_name,
            metadata={"hnsw:space": "cosine"}
        )
        print(f"VectorStore initialized. Collection: {collection_name}, Persist Dir: {persist_directory}")

    def add_documents(self, documents: List[str], metadatas: List[Dict], embeddings: List[List[float]]):
        """
        Add documents to the collection.
        """
        ids = [str(uuid.uuid4()) for _ in documents]
        self.collection.add(
            documents=documents,
            metadatas=metadatas,
            embeddings=embeddings,
            ids=ids
        )
        print(f"Added {len(documents)} documents to vector store.")

    def query(self, query_embedding: List[float], n_results=5, where: Optional[Dict] = None):
        """
        Query the collection.
        """
        results = self.collection.query(
            query_embeddings=[query_embedding],
            n_results=n_results,
            where=where
        )
        
        # Parse results into a friendly format
        parsed_results = []
        if results['documents']:
            for i in range(len(results['documents'][0])):
                doc = results['documents'][0][i]
                meta = results['metadatas'][0][i] if results['metadatas'] else {}
                score = results['distances'][0][i] if results['distances'] else 0.0
                parsed_results.append({
                    "content": doc,
                    "metadata": meta,
                    "distance": score
                })
        
        return parsed_results

    def count(self):
        return self.collection.count()

if __name__ == "__main__":
    # Test (requires Embedder to run effectively, but we can test logic)
    pass
