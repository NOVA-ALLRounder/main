"""
Coding Agent

코드 생성, 수정, 디버깅을 총괄하는 에이전트
"""

from typing import Dict, Any, Optional, List
from pathlib import Path
from dataclasses import dataclass
import json

import sys
import os
sys.path.append(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
from core.llm import LLMClient, get_llm_client
from core.logger import get_logger

from .code_generator import CodeGenerator, GeneratedCode
from .debugger import AutoDebugger, DebugResult


logger = get_logger("coding_agent")


@dataclass
class ExperimentCode:
    """실험 코드 패키지"""
    main_code: GeneratedCode
    plot_code: Optional[GeneratedCode] = None
    requirements: List[str] = None
    working_dir: str = ""
    
    def __post_init__(self):
        if self.requirements is None:
            self.requirements = self.main_code.dependencies or []


class CodingAgent:
    """코딩 에이전트"""
    
    def __init__(
        self,
        llm_client: Optional[LLMClient] = None,
        working_dir: str = "./experiments"
    ):
        self.llm = llm_client or get_llm_client()
        self.generator = CodeGenerator(self.llm)
        self.debugger = AutoDebugger(self.llm)
        self.working_dir = Path(working_dir)
        self.working_dir.mkdir(parents=True, exist_ok=True)
    
    def create_experiment(
        self,
        hypothesis: Dict[str, Any],
        experiment_plan: List[str]
    ) -> ExperimentCode:
        """
        가설과 실험 계획에서 실험 코드 생성
        
        Args:
            hypothesis: 가설 정보
            experiment_plan: 실험 계획 단계 리스트
        
        Returns:
            ExperimentCode 객체
        """
        description = f"""
Research Question: {hypothesis.get('research_question', '')}
Methodology: {hypothesis.get('methodology', '')}
Expected Results: {hypothesis.get('expected_results', '')}

Experiment Plan:
{chr(10).join(f'{i+1}. {step}' for i, step in enumerate(experiment_plan))}
"""
        
        # 메인 실험 코드 생성
        main_code = self.generator.generate(
            description=description,
            requirements=[
                "Complete implementation of all experiment steps",
                "Clear logging of progress and metrics",
                "Save results to JSON file",
                "Handle errors gracefully"
            ]
        )
        
        # 시각화 코드 생성
        plot_code = self.generator.generate_plotting_code(
            data_description="Experiment results including metrics over time",
            plot_type="multi-panel"
        )
        
        # 작업 디렉토리 설정
        exp_dir = self.working_dir / hypothesis.get('hypothesis_id', 'exp')
        exp_dir.mkdir(parents=True, exist_ok=True)
        
        return ExperimentCode(
            main_code=main_code,
            plot_code=plot_code,
            working_dir=str(exp_dir)
        )
    
    def modify_for_node(
        self,
        base_code: str,
        node_changes: Dict[str, Any]
    ) -> GeneratedCode:
        """
        트리 노드의 변경사항에 따라 코드 수정
        
        Args:
            base_code: 기본 코드
            node_changes: 노드의 변경사항
        
        Returns:
            수정된 코드
        """
        instructions = node_changes.get("description", "")
        
        if node_changes.get("config_changes"):
            config_str = json.dumps(node_changes["config_changes"], indent=2)
            instructions += f"\n\nApply these configuration changes:\n{config_str}"
        
        if node_changes.get("code_changes"):
            instructions += f"\n\nSpecific code changes:\n{node_changes['code_changes']}"
        
        return self.generator.modify(base_code, instructions)
    
    def debug_code(
        self,
        code: str,
        error: str
    ) -> DebugResult:
        """
        코드 디버깅
        
        Args:
            code: 에러가 발생한 코드
            error: 에러 메시지
        
        Returns:
            DebugResult
        """
        return self.debugger.debug(code, error)
    
    def get_missing_packages(self, error: str) -> List[str]:
        """누락 패키지 확인"""
        return self.debugger.get_missing_packages(error)
    
    def save_experiment(
        self,
        experiment: ExperimentCode,
        experiment_id: str = None
    ) -> str:
        """
        실험 코드 저장
        
        Args:
            experiment: ExperimentCode 객체
            experiment_id: 실험 ID
        
        Returns:
            저장된 디렉토리 경로
        """
        exp_dir = Path(experiment.working_dir) if experiment.working_dir else \
                  self.working_dir / (experiment_id or "experiment")
        exp_dir.mkdir(parents=True, exist_ok=True)
        
        # 메인 코드 저장
        main_file = exp_dir / experiment.main_code.filename
        with open(main_file, 'w', encoding='utf-8') as f:
            f.write(experiment.main_code.code)
        
        # 플롯 코드 저장
        if experiment.plot_code:
            plot_file = exp_dir / experiment.plot_code.filename
            with open(plot_file, 'w', encoding='utf-8') as f:
                f.write(experiment.plot_code.code)
        
        # requirements.txt 저장
        if experiment.requirements:
            req_file = exp_dir / "requirements.txt"
            with open(req_file, 'w', encoding='utf-8') as f:
                f.write("\n".join(experiment.requirements))
        
        logger.info(f"Experiment saved to {exp_dir}")
        return str(exp_dir)
    
    def generate_variants(
        self,
        base_code: str,
        num_variants: int = 3
    ) -> List[GeneratedCode]:
        """
        코드 변형 생성 (트리 검색용)
        """
        return self.generator.generate_experiment_variants(base_code, num_variants)
    
    def explain_code(self, code: str) -> str:
        """
        코드 설명 생성
        
        Args:
            code: 설명할 코드
        
        Returns:
            코드 설명
        """
        prompt = f"""Explain what this Python code does in simple terms:

```python
{code[:3000]}
```

Provide:
1. A brief summary
2. Key components and their purposes
3. Expected inputs and outputs
"""
        
        return self.llm.complete(
            prompt=prompt,
            system_prompt="You are a code documentation expert.",
            temperature=0.3
        )
