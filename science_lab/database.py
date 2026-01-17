"""
Database module - SQLite + ChromaDB for vector storage
"""
import sqlite3
from pathlib import Path
from typing import List, Dict, Any, Optional
from datetime import datetime
import json

try:
    import chromadb
    from chromadb.config import Settings
    CHROMA_AVAILABLE = True
except ImportError:
    CHROMA_AVAILABLE = False

from config import DB_PATH, VECTOR_DB_PATH


def get_db_connection() -> sqlite3.Connection:
    """SQLite 연결 반환"""
    conn = sqlite3.connect(str(DB_PATH))
    conn.row_factory = sqlite3.Row
    return conn


def init_database():
    """데이터베이스 스키마 초기화"""
    conn = get_db_connection()
    cursor = conn.cursor()
    
    # 연구 세션 테이블
    cursor.execute("""
        CREATE TABLE IF NOT EXISTS research_sessions (
            id TEXT PRIMARY KEY,
            user_query TEXT NOT NULL,
            domain TEXT,
            intent TEXT,
            status TEXT DEFAULT 'processing',
            state_json TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
    """)
    
    # 문헌 지식 베이스
    cursor.execute("""
        CREATE TABLE IF NOT EXISTS literature_knowledge (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT,
            title TEXT,
            authors TEXT,
            abstract TEXT,
            year INTEGER,
            source TEXT,
            url TEXT,
            relevance_score REAL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES research_sessions(id)
        )
    """)
    
    # 실험 결과 테이블
    cursor.execute("""
        CREATE TABLE IF NOT EXISTS experiment_results (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT,
            methodology_type TEXT,
            code_snippet TEXT,
            execution_log TEXT,
            success INTEGER,
            final_report TEXT,
            report_path TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES research_sessions(id)
        )
    """)
    
    conn.commit()
    conn.close()


def save_session(session_id: str, state: Dict[str, Any]):
    """세션 상태 저장"""
    conn = get_db_connection()
    cursor = conn.cursor()
    
    cursor.execute("""
        INSERT OR REPLACE INTO research_sessions 
        (id, user_query, domain, intent, status, state_json, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
    """, (
        session_id,
        state.get('user_input', ''),
        state.get('domain', ''),
        state.get('intent', ''),
        state.get('status', 'processing'),
        json.dumps(state, ensure_ascii=False, default=str),
        datetime.now().isoformat()
    ))
    
    conn.commit()
    conn.close()


def load_session(session_id: str) -> Optional[Dict[str, Any]]:
    """세션 상태 로드"""
    conn = get_db_connection()
    cursor = conn.cursor()
    
    cursor.execute(
        "SELECT state_json FROM research_sessions WHERE id = ?",
        (session_id,)
    )
    row = cursor.fetchone()
    conn.close()
    
    if row:
        return json.loads(row['state_json'])
    return None


def get_all_sessions() -> List[Dict[str, Any]]:
    """모든 세션 목록 반환"""
    conn = get_db_connection()
    cursor = conn.cursor()
    
    cursor.execute("""
        SELECT id, user_query, domain, intent, status, created_at 
        FROM research_sessions 
        ORDER BY created_at DESC
    """)
    rows = cursor.fetchall()
    conn.close()
    
    return [dict(row) for row in rows]


# Vector DB (ChromaDB) functions
_chroma_client = None


def get_chroma_client():
    """ChromaDB 클라이언트 반환"""
    global _chroma_client
    if _chroma_client is None and CHROMA_AVAILABLE:
        VECTOR_DB_PATH.mkdir(exist_ok=True)
        _chroma_client = chromadb.PersistentClient(
            path=str(VECTOR_DB_PATH),
            settings=Settings(anonymized_telemetry=False)
        )
    return _chroma_client


def add_to_knowledge_base(
    texts: List[str],
    metadatas: List[Dict[str, Any]],
    ids: List[str],
    collection_name: str = "literature"
):
    """벡터 DB에 문서 추가"""
    client = get_chroma_client()
    if client is None:
        return
    
    collection = client.get_or_create_collection(name=collection_name)
    collection.add(
        documents=texts,
        metadatas=metadatas,
        ids=ids
    )


def search_knowledge_base(
    query: str,
    n_results: int = 5,
    collection_name: str = "literature"
) -> List[Dict[str, Any]]:
    """벡터 DB에서 유사 문서 검색"""
    client = get_chroma_client()
    if client is None:
        return []
    
    try:
        collection = client.get_collection(name=collection_name)
        results = collection.query(
            query_texts=[query],
            n_results=n_results
        )
        
        documents = []
        if results['documents']:
            for i, doc in enumerate(results['documents'][0]):
                documents.append({
                    'content': doc,
                    'metadata': results['metadatas'][0][i] if results['metadatas'] else {},
                    'distance': results['distances'][0][i] if results['distances'] else 0
                })
        return documents
    except Exception:
        return []


# Initialize database on import
init_database()
