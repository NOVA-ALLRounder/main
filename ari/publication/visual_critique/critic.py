"""
Visual Critic

GPT-4V를 사용한 도표 품질 분석
"""

from dataclasses import dataclass, field
from typing import List, Dict, Any, Optional

import sys
import os
sys.path.append(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
from core.llm import LLMClient, get_llm_client
from core.logger import get_logger

logger = get_logger("visual_critique")


@dataclass
class PlotFeedback:
    """도표 피드백"""
    image_path: str
    
    # 점수 (1-10)
    overall_score: float = 0.0
    readability_score: float = 0.0
    informativeness_score: float = 0.0
    aesthetics_score: float = 0.0
    
    # 상세 피드백
    strengths: List[str] = field(default_factory=list)
    weaknesses: List[str] = field(default_factory=list)
    suggestions: List[str] = field(default_factory=list)
    
    # 코드 수정 제안
    code_fixes: str = ""
    
    # 출판 가능 여부
    publication_ready: bool = False
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "image_path": self.image_path,
            "overall_score": self.overall_score,
            "readability_score": self.readability_score,
            "informativeness_score": self.informativeness_score,
            "aesthetics_score": self.aesthetics_score,
            "strengths": self.strengths,
            "weaknesses": self.weaknesses,
            "suggestions": self.suggestions,
            "code_fixes": self.code_fixes,
            "publication_ready": self.publication_ready
        }


class VisualCritic:
    """VLM 기반 시각적 비평가"""
    
    CRITIQUE_PROMPT = """You are an expert reviewer of scientific figures and plots.

Analyze this figure for publication quality and provide detailed feedback.

Evaluate the following aspects:
1. **Readability**: Are labels, legends, and text clearly visible and appropriately sized?
2. **Informativeness**: Does the figure clearly convey the intended information?
3. **Aesthetics**: Is the color scheme appropriate? Is the layout clean?
4. **Scientific Standards**: Does it meet publication standards for academic papers?

Expected context/caption:
{caption}

Provide your critique in JSON format:
{{
    "overall_score": 1-10,
    "readability_score": 1-10,
    "informativeness_score": 1-10,
    "aesthetics_score": 1-10,
    "strengths": ["strength1", "strength2"],
    "weaknesses": ["weakness1", "weakness2"],
    "suggestions": ["suggestion1", "suggestion2"],
    "publication_ready": true/false
}}
"""
    
    CODE_FIX_PROMPT = """Based on the following feedback for a matplotlib figure:

Feedback:
{feedback}

Original plotting code:
```python
{code}
```

Generate improved code that addresses the weaknesses and implements the suggestions.
Focus on:
1. Improving readability (font sizes, labels)
2. Better color schemes
3. Cleaner layout
4. Publication-quality output

Return only the improved Python code.
"""
    
    def __init__(self, llm_client: Optional[LLMClient] = None):
        self.llm = llm_client or get_llm_client()
    
    def critique(
        self,
        image_path: str,
        caption: str = "",
        original_code: str = ""
    ) -> PlotFeedback:
        """
        도표 비평
        
        Args:
            image_path: 이미지 파일 경로
            caption: 예상 캡션/설명
            original_code: 원본 플로팅 코드
        
        Returns:
            PlotFeedback 객체
        """
        prompt = self.CRITIQUE_PROMPT.format(caption=caption or "No caption provided")
        
        try:
            # VLM으로 이미지 분석
            response_text = self.llm.analyze_image(
                image_path=image_path,
                prompt=prompt,
                system_prompt="You are an expert scientific figure reviewer."
            )
            
            # JSON 파싱
            import json
            import re
            
            # JSON 추출
            json_match = re.search(r'\{[\s\S]*\}', response_text)
            if json_match:
                response = json.loads(json_match.group())
            else:
                response = {}
            
            feedback = PlotFeedback(
                image_path=image_path,
                overall_score=float(response.get("overall_score", 5)),
                readability_score=float(response.get("readability_score", 5)),
                informativeness_score=float(response.get("informativeness_score", 5)),
                aesthetics_score=float(response.get("aesthetics_score", 5)),
                strengths=response.get("strengths", []),
                weaknesses=response.get("weaknesses", []),
                suggestions=response.get("suggestions", []),
                publication_ready=response.get("publication_ready", False)
            )
            
            # 코드 수정 제안 생성
            if original_code and not feedback.publication_ready:
                feedback.code_fixes = self._generate_code_fixes(
                    feedback, original_code
                )
            
            return feedback
        
        except Exception as e:
            logger.error(f"Visual critique failed: {e}")
            return PlotFeedback(
                image_path=image_path,
                overall_score=0,
                weaknesses=[f"Critique failed: {str(e)}"]
            )
    
    def _generate_code_fixes(
        self,
        feedback: PlotFeedback,
        original_code: str
    ) -> str:
        """코드 수정 제안 생성"""
        feedback_str = f"""
Weaknesses:
{chr(10).join('- ' + w for w in feedback.weaknesses)}

Suggestions:
{chr(10).join('- ' + s for s in feedback.suggestions)}
"""
        
        prompt = self.CODE_FIX_PROMPT.format(
            feedback=feedback_str,
            code=original_code
        )
        
        try:
            return self.llm.complete(
                prompt=prompt,
                system_prompt="You are improving matplotlib code for publication quality.",
                temperature=0.3
            )
        except Exception:
            return ""
    
    def batch_critique(
        self,
        images: List[Dict[str, str]]
    ) -> List[PlotFeedback]:
        """
        여러 도표 일괄 비평
        
        Args:
            images: [{"path": "...", "caption": "...", "code": "..."}]
        
        Returns:
            PlotFeedback 리스트
        """
        results = []
        
        for img_info in images:
            feedback = self.critique(
                image_path=img_info.get("path", ""),
                caption=img_info.get("caption", ""),
                original_code=img_info.get("code", "")
            )
            results.append(feedback)
        
        return results
    
    def is_publication_ready(self, image_path: str, threshold: float = 7.0) -> bool:
        """출판 준비 여부 확인"""
        feedback = self.critique(image_path)
        return feedback.overall_score >= threshold
    
    def get_improvement_priority(
        self,
        feedbacks: List[PlotFeedback]
    ) -> List[PlotFeedback]:
        """개선 우선순위 정렬"""
        # 점수가 낮은 순으로 정렬 (먼저 개선 필요)
        return sorted(feedbacks, key=lambda f: f.overall_score)
