"""
LLM Client Module

OpenAI API를 사용한 LLM 클라이언트
"""

import os
from typing import Optional, List, Dict, Any, Union
from openai import OpenAI
from tenacity import retry, stop_after_attempt, wait_exponential

import sys
sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from config import get_config


class LLMClient:
    """
    OpenAI LLM 클라이언트
    
    가설 생성, 코드 작성, 논문 작성 등 다양한 태스크에 사용
    """
    
    def __init__(
        self,
        api_key: Optional[str] = None,
        model: Optional[str] = None,
        temperature: float = 0.7,
        max_tokens: int = 4096
    ):
        config = get_config()
        self.api_key = api_key or config.openai.api_key
        self.model = model or config.openai.model
        self.temperature = temperature
        self.max_tokens = max_tokens
        
        if not self.api_key:
            raise ValueError("OpenAI API key is required. Set OPENAI_API_KEY environment variable.")
        
        self.client = OpenAI(api_key=self.api_key)
    
    @retry(stop=stop_after_attempt(3), wait=wait_exponential(multiplier=1, min=4, max=10))
    def chat(
        self,
        messages: List[Dict[str, str]],
        temperature: Optional[float] = None,
        max_tokens: Optional[int] = None,
        response_format: Optional[Dict[str, str]] = None
    ) -> str:
        """
        Chat completion 요청
        
        Args:
            messages: 대화 메시지 리스트 [{"role": "user", "content": "..."}]
            temperature: 샘플링 온도 (0.0 ~ 2.0)
            max_tokens: 최대 토큰 수
            response_format: 응답 형식 (예: {"type": "json_object"})
        
        Returns:
            LLM 응답 텍스트
        """
        kwargs = {
            "model": self.model,
            "messages": messages,
            "temperature": temperature or self.temperature,
            "max_tokens": max_tokens or self.max_tokens,
        }
        
        if response_format:
            kwargs["response_format"] = response_format
        
        response = self.client.chat.completions.create(**kwargs)
        return response.choices[0].message.content
    
    def complete(
        self,
        prompt: str,
        system_prompt: Optional[str] = None,
        temperature: Optional[float] = None,
        max_tokens: Optional[int] = None,
        response_format: Optional[Dict[str, str]] = None
    ) -> str:
        """
        단일 프롬프트에 대한 응답 생성
        
        Args:
            prompt: 사용자 프롬프트
            system_prompt: 시스템 프롬프트
            temperature: 샘플링 온도
            max_tokens: 최대 토큰 수
            response_format: 응답 형식
        
        Returns:
            LLM 응답 텍스트
        """
        messages = []
        
        if system_prompt:
            messages.append({"role": "system", "content": system_prompt})
        
        messages.append({"role": "user", "content": prompt})
        
        return self.chat(
            messages=messages,
            temperature=temperature,
            max_tokens=max_tokens,
            response_format=response_format
        )
    
    def generate_json(
        self,
        prompt: str,
        system_prompt: Optional[str] = None,
        temperature: float = 0.3
    ) -> Dict[str, Any]:
        """
        JSON 형식의 응답 생성
        
        Args:
            prompt: 사용자 프롬프트
            system_prompt: 시스템 프롬프트
            temperature: 샘플링 온도 (정확도를 위해 낮게 설정)
        
        Returns:
            파싱된 JSON 딕셔너리
        """
        import json
        
        response = self.complete(
            prompt=prompt,
            system_prompt=system_prompt,
            temperature=temperature,
            response_format={"type": "json_object"}
        )
        
        return json.loads(response)
    
    @retry(stop=stop_after_attempt(3), wait=wait_exponential(multiplier=1, min=4, max=10))
    def embed(self, texts: Union[str, List[str]]) -> List[List[float]]:
        """
        텍스트 임베딩 생성
        
        Args:
            texts: 임베딩할 텍스트 또는 텍스트 리스트
        
        Returns:
            임베딩 벡터 리스트
        """
        config = get_config()
        
        if isinstance(texts, str):
            texts = [texts]
        
        response = self.client.embeddings.create(
            model=config.openai.embedding_model,
            input=texts
        )
        
        return [item.embedding for item in response.data]
    
    def analyze_image(
        self,
        image_path: str,
        prompt: str,
        system_prompt: Optional[str] = None
    ) -> str:
        """
        이미지 분석 (GPT-4V)
        
        Args:
            image_path: 이미지 파일 경로
            prompt: 분석 요청 프롬프트
            system_prompt: 시스템 프롬프트
        
        Returns:
            이미지 분석 결과
        """
        import base64
        
        # Read and encode image
        with open(image_path, "rb") as f:
            image_data = base64.b64encode(f.read()).decode("utf-8")
        
        # Determine image type
        if image_path.lower().endswith(".png"):
            image_type = "image/png"
        elif image_path.lower().endswith((".jpg", ".jpeg")):
            image_type = "image/jpeg"
        else:
            image_type = "image/png"
        
        messages = []
        if system_prompt:
            messages.append({"role": "system", "content": system_prompt})
        
        messages.append({
            "role": "user",
            "content": [
                {"type": "text", "text": prompt},
                {
                    "type": "image_url",
                    "image_url": {
                        "url": f"data:{image_type};base64,{image_data}"
                    }
                }
            ]
        })
        
        response = self.client.chat.completions.create(
            model="gpt-4o",  # Vision model
            messages=messages,
            max_tokens=self.max_tokens
        )
        
        return response.choices[0].message.content


# Global LLM client instance
_llm_client: Optional[LLMClient] = None


def get_llm_client() -> LLMClient:
    """Get the global LLM client instance."""
    global _llm_client
    if _llm_client is None:
        _llm_client = LLMClient()
    return _llm_client


def reset_llm_client():
    """Reset the global LLM client instance."""
    global _llm_client
    _llm_client = None
