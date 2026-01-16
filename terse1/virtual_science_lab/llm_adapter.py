"""
Virtual Science Lab - LLM Source Adapter
DAACS v2 CLI Provider를 VSL에 통합하기 위한 어댑터 계층
"""

import sys
import subprocess
import json
from pathlib import Path
from abc import ABC, abstractmethod
from typing import Dict, Any, Optional


# DAACS v2 경로 설정
DAACS_V2_PATH = Path(__file__).parent.parent.parent.parent / "Daacs" / "DAACS_v2" / "transformers7-project-feature-backend2"


class LLMSource(ABC):
    """LLM 소스 베이스 클래스"""
    
    @abstractmethod
    def invoke(self, prompt: str, **kwargs) -> str:
        """LLM 호출"""
        pass
    
    @abstractmethod
    def invoke_structured(self, prompt: str, schema: Optional[Dict] = None) -> Dict:
        """구조화된 출력 (JSON)"""
        pass


class CLIAssistantSource(LLMSource):
    """
    CLI Assistant(Claude Code, Codex 등)를 사용하는 LLM 소스
    DAACS v2의 cli_executor.py 로직을 간소화하여 사용
    """
    
    CLI_COMMANDS = {
        "claude_code": ["claude", "-p"],
        "codex": ["codex", "-q", "--no-full-auto"],
        "gemini": ["gemini", "-p"],
        "cursor": ["cursor", "--pipe"]
    }
    
    def __init__(
        self,
        cli_type: str = "claude_code",
        cwd: str = ".",
        timeout_sec: int = 120
    ):
        self.cli_type = cli_type
        self.cwd = cwd
        self.timeout_sec = timeout_sec
        
        if cli_type not in self.CLI_COMMANDS:
            raise ValueError(f"Unsupported CLI type: {cli_type}. Supported: {list(self.CLI_COMMANDS.keys())}")
    
    def invoke(self, prompt: str, **kwargs) -> str:
        """CLI Assistant 호출"""
        cmd = self.CLI_COMMANDS[self.cli_type].copy()
        cmd.append(prompt)
        
        try:
            result = subprocess.run(
                cmd,
                cwd=self.cwd,
                capture_output=True,
                text=True,
                timeout=self.timeout_sec
            )
            
            if result.returncode != 0:
                return f"[CLI Error] {result.stderr}"
            
            return result.stdout.strip()
            
        except subprocess.TimeoutExpired:
            return f"[CLI Timeout] Exceeded {self.timeout_sec}s"
        except FileNotFoundError:
            return f"[CLI Not Found] {self.cli_type} is not installed"
        except Exception as e:
            return f"[CLI Exception] {str(e)}"
    
    def invoke_structured(self, prompt: str, schema: Optional[Dict] = None) -> Dict:
        """JSON 출력 요청"""
        json_prompt = prompt + "\n\n반드시 JSON 형식으로만 응답하세요. 다른 텍스트 없이 순수 JSON만 출력하세요."
        
        response = self.invoke(json_prompt)
        
        # JSON 파싱 시도
        try:
            # 코드 블록 제거
            if "```json" in response:
                response = response.split("```json")[1].split("```")[0]
            elif "```" in response:
                response = response.split("```")[1].split("```")[0]
            
            return json.loads(response.strip())
        except json.JSONDecodeError:
            return {"error": "JSON parse failed", "raw_response": response[:500]}


class MockLLMSource(LLMSource):
    """테스트용 Mock LLM (CLI 없이도 작동)"""
    
    def __init__(self, role: str = "default"):
        self.role = role
    
    def invoke(self, prompt: str, **kwargs) -> str:
        return f"[MockLLM:{self.role}] Processed prompt of {len(prompt)} chars"
    
    def invoke_structured(self, prompt: str, schema: Optional[Dict] = None) -> Dict:
        """역할별 Mock 응답 반환"""
        if self.role == "router":
            return {
                "intent": "hypothesis",
                "confidence": 0.85,
                "domain": "cs",
                "reasoning": "Mock classification"
            }
        elif self.role == "pi":
            return {
                "novelty_score": 0.75,
                "novelty_reasoning": "Mock novelty assessment",
                "already_validated": False,
                "proposed_methods": [
                    {
                        "method_id": 1,
                        "approach_type": "simulation",
                        "title": "Mock Method",
                        "description": "Test methodology",
                        "required_libraries": ["numpy"],
                        "pros": ["Simple"],
                        "cons": ["Mock only"]
                    }
                ]
            }
        elif self.role == "engineer":
            return {
                "main_script": "print('Hello from Mock Engineer')",
                "requirements": ["numpy"],
                "expected_outputs": ["results/test.txt"]
            }
        elif self.role == "critic":
            return {
                "verdict": "minor_revision",
                "overall_score": 7,
                "issues": [],
                "suggestions": ["This is a mock review"]
            }
        else:
            return {"status": "ok", "role": self.role}


class VSLLLMFactory:
    """VSL 에이전트별 LLM 소스 팩토리"""
    
    # 에이전트별 기본 CLI 타입 매핑
    AGENT_CLI_MAP = {
        "router": "claude_code",
        "librarian": None,  # API 직접 호출
        "pi": "claude_code",
        "engineer": "codex",
        "critic": "claude_code",
        "author": "claude_code"
    }
    
    @classmethod
    def create(
        cls,
        agent_name: str,
        use_mock: bool = False,
        cli_type: Optional[str] = None,
        **kwargs
    ) -> LLMSource:
        """
        에이전트용 LLM 소스 생성
        
        Args:
            agent_name: 에이전트 이름 (router, pi, engineer, etc.)
            use_mock: True면 Mock LLM 사용 (테스트용)
            cli_type: CLI 타입 오버라이드
            
        Returns:
            LLMSource 인스턴스
        """
        if use_mock:
            return MockLLMSource(role=agent_name)
        
        # CLI 타입 결정
        target_cli = cli_type or cls.AGENT_CLI_MAP.get(agent_name, "claude_code")
        
        if target_cli is None:
            # Librarian 등 API 직접 사용하는 경우
            return MockLLMSource(role=agent_name)
        
        return CLIAssistantSource(
            cli_type=target_cli,
            **kwargs
        )


# 간편 함수
def get_llm_for_agent(agent_name: str, **kwargs) -> LLMSource:
    """에이전트용 LLM 소스 가져오기"""
    return VSLLLMFactory.create(agent_name, **kwargs)
