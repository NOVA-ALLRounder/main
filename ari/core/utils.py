"""
Utility Functions

ARI 시스템 공통 유틸리티
"""

import time
import tiktoken
from typing import List, Callable, Any, Optional
from functools import wraps


def count_tokens(text: str, model: str = "gpt-4o") -> int:
    """
    텍스트의 토큰 수 계산
    
    Args:
        text: 토큰을 셀 텍스트
        model: 토큰화에 사용할 모델
    
    Returns:
        토큰 수
    """
    try:
        encoding = tiktoken.encoding_for_model(model)
    except KeyError:
        encoding = tiktoken.get_encoding("cl100k_base")
    
    return len(encoding.encode(text))


def chunk_text(
    text: str,
    max_tokens: int = 1000,
    overlap_tokens: int = 100,
    model: str = "gpt-4o"
) -> List[str]:
    """
    텍스트를 토큰 기준으로 청킹
    
    Args:
        text: 청킹할 텍스트
        max_tokens: 청크당 최대 토큰 수
        overlap_tokens: 청크 간 오버랩 토큰 수
        model: 토큰화에 사용할 모델
    
    Returns:
        텍스트 청크 리스트
    """
    try:
        encoding = tiktoken.encoding_for_model(model)
    except KeyError:
        encoding = tiktoken.get_encoding("cl100k_base")
    
    tokens = encoding.encode(text)
    chunks = []
    
    start = 0
    while start < len(tokens):
        end = min(start + max_tokens, len(tokens))
        chunk_tokens = tokens[start:end]
        chunk_text = encoding.decode(chunk_tokens)
        chunks.append(chunk_text)
        
        if end >= len(tokens):
            break
        
        start = end - overlap_tokens
    
    return chunks


def retry_with_backoff(
    max_retries: int = 3,
    initial_delay: float = 1.0,
    max_delay: float = 60.0,
    exponential_base: float = 2.0,
    exceptions: tuple = (Exception,)
) -> Callable:
    """
    지수 백오프를 사용하는 재시도 데코레이터
    
    Args:
        max_retries: 최대 재시도 횟수
        initial_delay: 초기 지연 시간 (초)
        max_delay: 최대 지연 시간 (초)
        exponential_base: 지수 기준
        exceptions: 재시도할 예외 타입
    
    Returns:
        데코레이터 함수
    """
    def decorator(func: Callable) -> Callable:
        @wraps(func)
        def wrapper(*args, **kwargs) -> Any:
            delay = initial_delay
            last_exception = None
            
            for attempt in range(max_retries + 1):
                try:
                    return func(*args, **kwargs)
                except exceptions as e:
                    last_exception = e
                    if attempt == max_retries:
                        raise
                    
                    time.sleep(min(delay, max_delay))
                    delay *= exponential_base
            
            raise last_exception
        return wrapper
    return decorator


def clean_text(text: str) -> str:
    """
    텍스트 정제 (불필요한 공백 제거 등)
    
    Args:
        text: 정제할 텍스트
    
    Returns:
        정제된 텍스트
    """
    import re
    
    # Remove multiple whitespaces
    text = re.sub(r'\s+', ' ', text)
    # Remove leading/trailing whitespace
    text = text.strip()
    
    return text


def extract_json_from_text(text: str) -> Optional[dict]:
    """
    텍스트에서 JSON 추출
    
    Args:
        text: JSON을 포함할 수 있는 텍스트
    
    Returns:
        추출된 JSON 딕셔너리 또는 None
    """
    import json
    import re
    
    # Try to find JSON block in markdown code block
    json_match = re.search(r'```(?:json)?\s*([\s\S]*?)```', text)
    if json_match:
        try:
            return json.loads(json_match.group(1))
        except json.JSONDecodeError:
            pass
    
    # Try to parse entire text as JSON
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        pass
    
    # Try to find JSON object in text
    json_match = re.search(r'\{[\s\S]*\}', text)
    if json_match:
        try:
            return json.loads(json_match.group())
        except json.JSONDecodeError:
            pass
    
    return None


def sanitize_filename(filename: str) -> str:
    """
    파일명 안전하게 변환
    
    Args:
        filename: 원본 파일명
    
    Returns:
        안전한 파일명
    """
    import re
    
    # 1. 길이 제한 (윈도우 경로 제한 고려하여 50자로 축소)
    max_len = 50
    
    # 2. 허용된 문자만 남기고 모두 제거 (알파벳, 한글, 숫자, 공백, -, _)
    # 윈도우에서 문제될 수 있는 특수문자들 모두 배제
    filename = re.sub(r'[^a-zA-Z0-9가-힣\s\-_]', '', filename)
    
    # 3. 연속된 공백이나 _를 하나로 줄임
    filename = re.sub(r'[\s_-]+', '_', filename)
    
    # 4. 앞뒤 공백/특수문자 제거
    filename = filename.strip(' ._-')
    
    if len(filename) > max_len:
        filename = filename[:max_len].strip(' ._-')
    
    return filename or "unnamed"


def format_file_size(size_bytes: int) -> str:
    """
    바이트 크기를 읽기 쉬운 형식으로 변환
    
    Args:
        size_bytes: 바이트 크기
    
    Returns:
        포맷된 크기 문자열
    """
    for unit in ['B', 'KB', 'MB', 'GB', 'TB']:
        if size_bytes < 1024:
            return f"{size_bytes:.2f} {unit}"
        size_bytes /= 1024
    return f"{size_bytes:.2f} PB"
