"""
DAACS v3.0 Document System - Replanning Strategies
Adapted from DAACS V6 replanning.py for document domain
"""

from typing import Dict, List, Any, Optional


class DocumentReplanningStrategies:
    """
    Failure-specific recovery strategies for the document writing workflow.
    """

    STRATEGIES = {
        "content_issue": {
            "stop": False,
            "reason": "Content issues found - routing back to Writer",
            "next_agent": "writer",
            "severity": "medium",
            "prompt_addon": "Please revise the content focusing on the following issues: {issues}"
        },
        
        "fact_issue": {
            "stop": False,
            "reason": "Factual accuracy problems - routing to Researcher",
            "next_agent": "researcher",
            "severity": "high",
            "prompt_addon": "Please verify and correct the following claims: {issues}"
        },
        
        "style_issue": {
            "stop": False,
            "reason": "Style/formatting issues - routing to Formatter",
            "next_agent": "formatter",
            "severity": "low",
            "prompt_addon": "Please fix the following style issues: {issues}"
        },
        
        "structure_issue": {
            "stop": False,
            "reason": "Document structure mismatch - routing to Writer",
            "next_agent": "writer",
            "severity": "medium",
            "prompt_addon": "The document structure doesn't match the plan. Please reorganize: {issues}"
        },
        
        "placeholder_found": {
            "stop": False,
            "reason": "Unfinished placeholders detected - routing to Writer",
            "next_agent": "writer",
            "severity": "high",
            "prompt_addon": "Please replace all placeholders with actual content: {issues}"
        },
        
        "max_revisions_exceeded": {
            "stop": False,
            "reason": "Maximum revision attempts reached - forcing soft approval",
            "next_agent": "publisher",  # Force proceed
            "severity": "critical",
            "prompt_addon": "Proceeding despite unresolved issues due to revision limit."
        },
        
        "user_intervention_required": {
            "stop": True,
            "reason": "Cannot resolve issue automatically - user input needed",
            "next_agent": None,
            "severity": "critical",
            "prompt_addon": None
        }
    }

    @staticmethod
    def get_strategy(failure_type: Optional[str]) -> Dict[str, Any]:
        """Get strategy for a given failure type."""
        if not failure_type:
            return {
                "stop": False,
                "reason": "Unknown issue - generic retry",
                "next_agent": "writer",
                "severity": "low"
            }
        
        return DocumentReplanningStrategies.STRATEGIES.get(
            failure_type,
            {"stop": False, "reason": f"Unknown: {failure_type}", "next_agent": "writer", "severity": "low"}
        )

    @staticmethod
    def should_stop(
        failure_type: Optional[str],
        consecutive_failures: int,
        max_failures: int = 3
    ) -> bool:
        """Determine if we should stop and request user intervention."""
        strategy = DocumentReplanningStrategies.get_strategy(failure_type)
        
        # Explicit stop
        if strategy.get("stop"):
            return True
        
        # Max failures reached
        if consecutive_failures >= max_failures:
            return True
        
        # Critical severity
        if strategy.get("severity") == "critical" and failure_type != "max_revisions_exceeded":
            return True
        
        return False


def detect_failure_type(critique_history: List[Dict]) -> Optional[str]:
    """
    Analyze critique history to determine the dominant failure type.
    
    Args:
        critique_history: List of CritiqueItem dicts
        
    Returns:
        The detected failure type string
    """
    if not critique_history:
        return None
    
    # Count issue types
    type_counts = {"content": 0, "fact": 0, "style": 0, "structure": 0, "placeholder": 0}
    
    for critique in critique_history:
        issue_type = critique.get("type", "").lower()
        comment = critique.get("comment", "").lower()
        
        if issue_type == "content" or "logic" in comment or "missing" in comment:
            type_counts["content"] += 1
        elif issue_type == "fact" or "source" in comment or "evidence" in comment:
            type_counts["fact"] += 1
        elif issue_type == "style" or "format" in comment or "tone" in comment:
            type_counts["style"] += 1
        elif "structure" in comment or "heading" in comment or "section" in comment:
            type_counts["structure"] += 1
        elif "placeholder" in comment or "todo" in comment:
            type_counts["placeholder"] += 1
    
    # Find dominant type
    max_type = max(type_counts, key=type_counts.get)
    max_count = type_counts[max_type]
    
    if max_count == 0:
        return "content_issue"  # Default
    
    type_mapping = {
        "content": "content_issue",
        "fact": "fact_issue",
        "style": "style_issue",
        "structure": "structure_issue",
        "placeholder": "placeholder_found"
    }
    
    return type_mapping.get(max_type, "content_issue")


def create_replan_response(
    failure_type: Optional[str],
    consecutive_failures: int,
    max_failures: int = 3,
    issues: Optional[List[str]] = None
) -> Dict[str, Any]:
    """
    Create a replanning response based on failure analysis.
    
    Returns:
        {
            "stop": bool,
            "next_agent": str or None,
            "reason": str,
            "prompt_addon": str (additional context for the next agent)
        }
    """
    # Check if we've hit the limit
    if consecutive_failures >= max_failures:
        failure_type = "max_revisions_exceeded"
    
    strategy = DocumentReplanningStrategies.get_strategy(failure_type)
    should_stop = DocumentReplanningStrategies.should_stop(
        failure_type, consecutive_failures, max_failures
    )
    
    if should_stop and failure_type != "max_revisions_exceeded":
        return {
            "stop": True,
            "next_agent": None,
            "reason": strategy["reason"],
            "prompt_addon": None
        }
    
    # Format prompt addon with issues
    prompt_addon = strategy.get("prompt_addon", "")
    if prompt_addon and issues:
        prompt_addon = prompt_addon.format(issues="; ".join(issues[:5]))
    
    return {
        "stop": False,
        "next_agent": strategy["next_agent"],
        "reason": strategy["reason"],
        "prompt_addon": prompt_addon
    }
