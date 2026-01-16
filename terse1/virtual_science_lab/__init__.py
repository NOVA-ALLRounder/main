"""
Virtual Science Lab (VSL)
자율 과학 발견을 위한 멀티 에이전트 프레임워크
"""

from .state import (
    ScientificState,
    LiteratureItem,
    MethodologyProposal,
    ExperimentResult
)
from .prompts import VSLPrompts
from .llm_adapter import get_llm_for_agent, CLIAssistantSource, MockLLMSource, VSLLLMFactory

__version__ = "1.0.0"
__all__ = [
    "ScientificState",
    "LiteratureItem",
    "MethodologyProposal", 
    "ExperimentResult",
    "VSLPrompts",
    "get_llm_for_agent",
    "CLIAssistantSource",
    "MockLLMSource",
    "VSLLLMFactory",
]

