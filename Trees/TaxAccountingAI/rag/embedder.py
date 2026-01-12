from sentence_transformers import SentenceTransformer
import os

class Embedder:
    def __init__(self, model_name="jhgan/ko-sroberta-multitask"):
        """
        Initialize the embedder with a sentence-transformer model.
        Using 'jhgan/ko-sroberta-multitask' which is excellent for Korean semantic search.
        """
        print(f"Loading embedding model: {model_name}...")
        self.model = SentenceTransformer(model_name)
        
    def embed_text(self, text: str):
        """
        Embed a single string.
        """
        return self.model.encode(text).tolist()
    
    def embed_documents(self, texts: list[str]):
        """
        Embed a list of strings.
        """
        return self.model.encode(texts).tolist()

if __name__ == "__main__":
    # Test
    embedder = Embedder()
    vector = embedder.embed_text("안녕하세요. 세금 관련 문의입니다.")
    print(f"Embedding dimension: {len(vector)}")
    print("Embedder verification successful.")
