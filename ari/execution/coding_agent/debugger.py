"""
Auto Debugger

LLM 기반 자동 디버깅
"""

from typing import Dict, Any, Optional, List, Tuple
from dataclasses import dataclass

import sys
import os
sys.path.append(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
from core.llm import LLMClient, get_llm_client


@dataclass
class DebugResult:
    """디버깅 결과"""
    fixed_code: str
    was_fixed: bool
    error_analysis: str
    fix_description: str
    confidence: float


class AutoDebugger:
    """LLM 기반 자동 디버거"""
    
    DEBUG_PROMPT = """You are an expert Python debugger.

The following code produced an error:

Code:
```python
{code}
```

Error:
```
{error}
```

Analyze the error and fix the code.

Return your analysis in JSON format:
{{
    "error_type": "type of error (syntax, runtime, logic, import, etc.)",
    "error_analysis": "detailed analysis of what went wrong",
    "fixed_code": "the corrected Python code",
    "fix_description": "description of the fix applied",
    "confidence": 0.0-1.0
}}
"""
    
    IMPORT_FIX_PROMPT = """The following Python code has an import error:

Error:
```
{error}
```

What packages need to be installed? Return as JSON:
{{
    "packages": ["package1", "package2"],
    "install_commands": ["pip install package1", "pip install package2"]
}}
"""
    
    def __init__(
        self,
        llm_client: Optional[LLMClient] = None,
        max_attempts: int = 5
    ):
        self.llm = llm_client or get_llm_client()
        self.max_attempts = max_attempts
    
    def debug(
        self,
        code: str,
        error: str,
        context: str = ""
    ) -> DebugResult:
        """
        코드 디버깅
        
        Args:
            code: 에러가 발생한 코드
            error: 에러 메시지
            context: 추가 컨텍스트
        
        Returns:
            DebugResult 객체
        """
        prompt = self.DEBUG_PROMPT.format(
            code=code,
            error=error
        )
        
        if context:
            prompt += f"\n\nAdditional context:\n{context}"
        
        try:
            response = self.llm.generate_json(
                prompt=prompt,
                system_prompt="You are an expert Python debugger. Fix the code with minimal changes.",
                temperature=0.2
            )
            
            return DebugResult(
                fixed_code=response.get("fixed_code", code),
                was_fixed=True,
                error_analysis=response.get("error_analysis", ""),
                fix_description=response.get("fix_description", ""),
                confidence=float(response.get("confidence", 0.5))
            )
        
        except Exception as e:
            return DebugResult(
                fixed_code=code,
                was_fixed=False,
                error_analysis=f"Debug failed: {str(e)}",
                fix_description="",
                confidence=0.0
            )
    
    def get_missing_packages(self, error: str) -> List[str]:
        """
        누락된 패키지 식별
        
        Args:
            error: 에러 메시지
        
        Returns:
            설치해야 할 패키지 리스트
        """
        # 일반적인 import 에러 패턴
        import re
        
        packages = []
        
        # ModuleNotFoundError: No module named 'xxx'
        match = re.search(r"No module named ['\"](\w+)['\"]", error)
        if match:
            packages.append(match.group(1))
        
        # ImportError: cannot import name 'xxx' from 'yyy'
        match = re.search(r"cannot import name .+ from ['\"](\w+)['\"]", error)
        if match:
            packages.append(match.group(1))
        
        # LLM으로 추가 분석
        if not packages and "import" in error.lower():
            try:
                response = self.llm.generate_json(
                    prompt=self.IMPORT_FIX_PROMPT.format(error=error),
                    temperature=0.1
                )
                packages = response.get("packages", [])
            except Exception:
                pass
        
        return packages
    
    def debug_loop(
        self,
        code: str,
        executor: callable,
        max_attempts: int = None
    ) -> Tuple[str, bool, List[str]]:
        """
        디버깅 루프 실행
        
        Args:
            code: 초기 코드
            executor: 코드 실행 함수 (code -> (success, output, error))
            max_attempts: 최대 시도 횟수
        
        Returns:
            (최종 코드, 성공 여부, 에러 히스토리)
        """
        max_attempts = max_attempts or self.max_attempts
        error_history = []
        current_code = code
        
        for attempt in range(max_attempts):
            success, output, error = executor(current_code)
            
            if success:
                return current_code, True, error_history
            
            error_history.append(f"Attempt {attempt + 1}: {error[:200]}")
            
            # 누락 패키지 처리
            missing = self.get_missing_packages(error)
            if missing:
                # 패키지 설치 코드 추가
                install_code = "\n".join([
                    f"# pip install {pkg}" for pkg in missing
                ])
                current_code = install_code + "\n" + current_code
            
            # 코드 수정
            result = self.debug(current_code, error, context=f"Previous errors: {error_history}")
            
            if result.was_fixed and result.confidence > 0.3:
                current_code = result.fixed_code
            else:
                # 수정 실패
                break
        
        return current_code, False, error_history
    
    def analyze_error(self, error: str) -> Dict[str, Any]:
        """
        에러 분석
        
        Args:
            error: 에러 메시지
        
        Returns:
            분석 결과 딕셔너리
        """
        prompt = f"""Analyze this Python error and categorize it:

Error:
```
{error}
```

Return as JSON:
{{
    "error_type": "syntax|runtime|logic|import|memory|timeout|other",
    "severity": "low|medium|high|critical",
    "likely_cause": "description of likely cause",
    "suggested_fixes": ["fix1", "fix2"],
    "is_recoverable": true/false
}}
"""
        
        try:
            return self.llm.generate_json(prompt=prompt, temperature=0.1)
        except Exception:
            return {
                "error_type": "unknown",
                "severity": "medium",
                "likely_cause": error[:200],
                "suggested_fixes": [],
                "is_recoverable": True
            }
