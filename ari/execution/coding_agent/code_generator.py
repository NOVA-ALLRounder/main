"""
Code Generator

LLM 기반 실험 코드 생성
"""

from typing import Dict, Any, Optional, List
from dataclasses import dataclass

import sys
import os
sys.path.append(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
from core.llm import LLMClient, get_llm_client


@dataclass 
class GeneratedCode:
    """생성된 코드"""
    code: str
    language: str = "python"
    filename: str = "experiment.py"
    description: str = ""
    dependencies: List[str] = None
    
    def __post_init__(self):
        if self.dependencies is None:
            self.dependencies = []


class CodeGenerator:
    """LLM 기반 코드 생성기"""
    
    GENERATION_PROMPT = """You are an expert Python developer specializing in machine learning and scientific computing.

Generate Python code for the following experiment:

Experiment Description:
{description}

Requirements:
{requirements}

The code should:
1. Be complete and runnable
2. Include proper imports
3. Include error handling
4. Print clear progress and results
5. Save results to files when appropriate

Return the code in the following JSON format:
{{
    "code": "complete Python code here",
    "filename": "suggested filename",
    "dependencies": ["list", "of", "required", "packages"],
    "description": "brief description of what the code does"
}}
"""
    
    MODIFICATION_PROMPT = """You are an expert Python developer. 

Modify the following code according to the instructions:

Original Code:
```python
{original_code}
```

Modification Instructions:
{instructions}

Return the modified code in JSON format:
{{
    "code": "modified Python code",
    "changes_summary": "summary of changes made"
}}
"""
    
    def __init__(self, llm_client: Optional[LLMClient] = None):
        self.llm = llm_client or get_llm_client()
    
    def generate(
        self,
        description: str,
        requirements: List[str] = None,
        template: str = None
    ) -> GeneratedCode:
        """
        실험 코드 생성
        
        Args:
            description: 실험 설명
            requirements: 요구사항 리스트
            template: 코드 템플릿 (선택)
        
        Returns:
            GeneratedCode 객체
        """
        requirements_str = "\n".join(f"- {r}" for r in (requirements or []))
        
        prompt = self.GENERATION_PROMPT.format(
            description=description,
            requirements=requirements_str or "No specific requirements"
        )
        
        if template:
            prompt += f"\n\nUse this as a starting template:\n```python\n{template}\n```"
        
        try:
            response = self.llm.generate_json(
                prompt=prompt,
                system_prompt="You are an expert Python developer. Generate clean, efficient, and well-documented code.",
                temperature=0.5
            )
            
            return GeneratedCode(
                code=response.get("code", ""),
                filename=response.get("filename", "experiment.py"),
                description=response.get("description", ""),
                dependencies=response.get("dependencies", [])
            )
        
        except Exception as e:
            return GeneratedCode(
                code=f"# Error generating code: {str(e)}\nraise NotImplementedError('Code generation failed')",
                description=f"Error: {str(e)}"
            )
    
    def modify(
        self,
        original_code: str,
        instructions: str
    ) -> GeneratedCode:
        """
        코드 수정
        
        Args:
            original_code: 원본 코드
            instructions: 수정 지시사항
        
        Returns:
            수정된 GeneratedCode 객체
        """
        prompt = self.MODIFICATION_PROMPT.format(
            original_code=original_code,
            instructions=instructions
        )
        
        try:
            response = self.llm.generate_json(
                prompt=prompt,
                system_prompt="You are modifying Python code. Make minimal, targeted changes.",
                temperature=0.3
            )
            
            return GeneratedCode(
                code=response.get("code", original_code),
                description=response.get("changes_summary", "")
            )
        
        except Exception as e:
            return GeneratedCode(
                code=original_code,
                description=f"Error modifying code: {str(e)}"
            )
    
    def generate_experiment_variants(
        self,
        base_code: str,
        num_variants: int = 3
    ) -> List[GeneratedCode]:
        """
        실험 변형 생성 (트리 탐색용)
        
        Args:
            base_code: 기본 코드
            num_variants: 생성할 변형 수
        
        Returns:
            변형 코드 리스트
        """
        prompt = f"""Given this base experiment code:

```python
{base_code}
```

Generate {num_variants} different variations that could improve the results.
Each variation should try a different approach such as:
- Different hyperparameters
- Different model architecture
- Different data preprocessing
- Different optimization technique

Return as JSON:
{{
    "variants": [
        {{
            "code": "variant code",
            "description": "what this variant changes"
        }}
    ]
}}
"""
        
        try:
            response = self.llm.generate_json(
                prompt=prompt,
                system_prompt="You are generating experiment variations for hyperparameter search and ablation studies.",
                temperature=0.7
            )
            
            variants = []
            for item in response.get("variants", [])[:num_variants]:
                variants.append(GeneratedCode(
                    code=item.get("code", ""),
                    description=item.get("description", "")
                ))
            
            return variants
        
        except Exception as e:
            return []
    
    def generate_plotting_code(
        self,
        data_description: str,
        plot_type: str = "line"
    ) -> GeneratedCode:
        """플로팅 코드 생성"""
        prompt = f"""Generate Python matplotlib code to create a {plot_type} plot.

Data description:
{data_description}

The code should:
1. Use matplotlib with a clean, publication-quality style
2. Include proper labels, title, and legend
3. Save the plot to a file
4. Be complete and runnable

Return as JSON:
{{
    "code": "plotting code",
    "filename": "plot.py"
}}
"""
        
        try:
            response = self.llm.generate_json(prompt=prompt, temperature=0.3)
            
            return GeneratedCode(
                code=response.get("code", ""),
                filename=response.get("filename", "plot.py"),
                description=f"{plot_type} plot generation"
            )
        
        except Exception:
            return GeneratedCode(
                code="import matplotlib.pyplot as plt\nprint('Plot generation failed')",
                filename="plot.py"
            )
