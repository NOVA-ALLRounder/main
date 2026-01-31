# =============================================================================
# T_lab - Dynamic Domain Knowledge Store
# Auto-learning domain-specific mathematical models
# =============================================================================

import json
import os
from typing import Dict, Any, Optional
from datetime import datetime
from pathlib import Path

try:
    from langchain_openai import ChatOpenAI
    from langchain_core.messages import SystemMessage, HumanMessage
    LANGCHAIN_AVAILABLE = True
except ImportError:
    LANGCHAIN_AVAILABLE = False

from core import get_settings, get_logger

settings = get_settings()
logger = get_logger("domain_knowledge")

# Path to knowledge base
KNOWLEDGE_FILE = Path(__file__).parent / "data" / "domain_knowledge.json"


DOMAIN_EXTRACTION_PROMPT = """당신은 과학 연구 방법론 전문가입니다.

주어진 도메인과 가설에 대해, 이 분야에서 사용되는 수학적/통계적 모델을 분석하세요.

반드시 다음 JSON 형식으로 응답하세요:
{
    "models": [
        {
            "name": "모델 이름",
            "formula": "수학적 공식",
            "description": "모델 설명"
        }
    ],
    "code_template": "Python 코드 템플릿 (import문과 기본 설정)",
    "variables": {
        "independent": ["독립변수1", "독립변수2"],
        "dependent": "종속변수"
    },
    "simulatable": true,
    "simulation_reason": "시뮬레이션 가능/불가능 이유"
}

시뮬레이션이 불가능한 경우 (질적 연구, 실제 데이터 필요 등):
{
    "simulatable": false,
    "simulation_reason": "이유 설명",
    "alternative_method": "대안적 연구 방법"
}
"""


class DomainKnowledgeStore:
    """
    Dynamic domain knowledge store with auto-learning capability.
    
    - Caches domain-specific mathematical models
    - Auto-learns new domains via LLM
    - Persists knowledge to JSON file
    """
    
    def __init__(self):
        self.knowledge: Dict[str, Any] = {}
        self.llm = None
        
        if LANGCHAIN_AVAILABLE and settings.openai_api_key:
            self.llm = ChatOpenAI(
                model=settings.default_model,
                temperature=0.3,
                api_key=settings.openai_api_key
            )
        
        self._load_knowledge()
    
    def _load_knowledge(self):
        """Load knowledge from JSON file."""
        if KNOWLEDGE_FILE.exists():
            try:
                with open(KNOWLEDGE_FILE, 'r', encoding='utf-8') as f:
                    self.knowledge = json.load(f)
                logger.info(f"Loaded {len(self.knowledge)} domains from knowledge base")
            except Exception as e:
                logger.error(f"Failed to load knowledge: {e}")
                self.knowledge = {}
        else:
            self.knowledge = {}
    
    def _save_knowledge(self):
        """Save knowledge to JSON file."""
        try:
            KNOWLEDGE_FILE.parent.mkdir(parents=True, exist_ok=True)
            with open(KNOWLEDGE_FILE, 'w', encoding='utf-8') as f:
                json.dump(self.knowledge, f, ensure_ascii=False, indent=4)
            logger.info("Knowledge base saved")
        except Exception as e:
            logger.error(f"Failed to save knowledge: {e}")
    
    def get_domain_models(self, domain: str) -> Optional[Dict[str, Any]]:
        """
        Get mathematical models for a domain.
        
        Returns cached knowledge if available.
        """
        # Normalize domain name
        domain_lower = domain.lower().strip()
        
        # Try exact match first
        if domain_lower in self.knowledge:
            logger.info(f"Cache hit for domain: {domain_lower}")
            return self.knowledge[domain_lower]
        
        # Try partial match
        for key in self.knowledge:
            if domain_lower in key.lower() or key.lower() in domain_lower:
                logger.info(f"Partial match: {domain_lower} -> {key}")
                return self.knowledge[key]
        
        return None
    
    def learn_domain(self, domain: str, hypothesis: str) -> Dict[str, Any]:
        """
        Learn a new domain's mathematical models using LLM.
        
        Args:
            domain: Domain name
            hypothesis: Example hypothesis for context
            
        Returns:
            Learned domain knowledge
        """
        if not self.llm:
            logger.warning("LLM not available, returning default knowledge")
            return self._get_default_knowledge(domain)
        
        logger.info(f"Learning new domain: {domain}")
        
        try:
            messages = [
                SystemMessage(content=DOMAIN_EXTRACTION_PROMPT),
                HumanMessage(content=f"도메인: {domain}\n가설 예시: {hypothesis}")
            ]
            
            response = self.llm.invoke(messages)
            content = response.content
            
            # Parse JSON from response
            knowledge = self._parse_llm_response(content)
            
            if knowledge:
                knowledge["learned_at"] = datetime.now().isoformat()
                knowledge["source"] = "llm_generated"
                
                # Save to cache
                self.knowledge[domain.lower().strip()] = knowledge
                self._save_knowledge()
                
                logger.info(f"Learned and saved domain: {domain}")
                return knowledge
            
        except Exception as e:
            logger.error(f"Failed to learn domain: {e}")
        
        return self._get_default_knowledge(domain)
    
    def _parse_llm_response(self, content: str) -> Optional[Dict[str, Any]]:
        """Parse LLM response to extract JSON."""
        try:
            # Try direct JSON parse
            return json.loads(content)
        except:
            pass
        
        # Try to find JSON in markdown code block
        import re
        json_match = re.search(r'```(?:json)?\s*(\{.*?\})\s*```', content, re.DOTALL)
        if json_match:
            try:
                return json.loads(json_match.group(1))
            except:
                pass
        
        # Try to find raw JSON
        json_match = re.search(r'\{[^{}]*\}', content, re.DOTALL)
        if json_match:
            try:
                return json.loads(json_match.group(0))
            except:
                pass
        
        return None
    
    def _get_default_knowledge(self, domain: str) -> Dict[str, Any]:
        """Return default knowledge for unknown domains."""
        return {
            "models": [
                {"name": "t-검정", "formula": "t = (x̄₁ - x̄₂) / SE", "description": "두 집단 평균 비교"},
                {"name": "회귀분석", "formula": "Y = β₀ + β₁X + ε", "description": "변수 간 관계 분석"}
            ],
            "code_template": "import numpy as np\nfrom scipy import stats\n# Statistical analysis",
            "variables": {
                "independent": ["처리 조건"],
                "dependent": "측정값"
            },
            "simulatable": True,
            "simulation_reason": "일반적인 통계 분석 적용",
            "source": "default"
        }
    
    def check_simulatable(self, domain: str, hypothesis: str) -> Dict[str, Any]:
        """
        Check if a hypothesis is suitable for simulation.
        
        Returns:
            {
                "simulatable": bool,
                "reason": str,
                "alternative": str | None
            }
        """
        knowledge = self.get_domain_models(domain)
        
        if knowledge is None:
            # Learn the domain first
            knowledge = self.learn_domain(domain, hypothesis)
        
        return {
            "simulatable": knowledge.get("simulatable", True),
            "reason": knowledge.get("simulation_reason", "시뮬레이션 가능"),
            "alternative": knowledge.get("alternative_method")
        }
    
    def get_prompt_injection(self, domain: str, hypothesis: str) -> str:
        """
        Get domain-specific content to inject into Engineer prompt.
        
        Returns formatted string with models and code template.
        """
        knowledge = self.get_domain_models(domain)
        
        if knowledge is None:
            knowledge = self.learn_domain(domain, hypothesis)
        
        if not knowledge:
            return ""
        
        models = knowledge.get("models", [])
        template = knowledge.get("code_template", "")
        variables = knowledge.get("variables", {})
        
        injection = f"""
## 도메인 특화 정보 ({domain})

### 적용 가능한 수학 모델:
"""
        for model in models:
            injection += f"- **{model.get('name')}**: `{model.get('formula')}`\n"
            injection += f"  {model.get('description')}\n"
        
        if variables:
            injection += f"""
### 변수:
- 독립변수: {', '.join(variables.get('independent', []))}
- 종속변수: {variables.get('dependent', '측정값')}
"""
        
        if template:
            injection += f"""
### 코드 템플릿:
```python
{template}
```
"""
        
        return injection


# Global singleton
domain_store = DomainKnowledgeStore()
