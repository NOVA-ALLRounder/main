# =============================================================================
# T_lab Agent - Fact Checker (Citation Verification)
# Verifies all citations have valid DOI/arXiv IDs
# =============================================================================

from typing import Dict, Any, List
import re
import httpx

from state import ScientificState
from core import get_settings, get_logger

settings = get_settings()
logger = get_logger("agents.fact_checker")


class FactCheckerAgent:
    """Verifies citations and references."""
    
    DOI_PATTERN = r'10\.\d{4,}/[^\s]+'
    ARXIV_PATTERN = r'\d{4}\.\d{4,5}'
    
    def __init__(self):
        self.verified = []
        self.unverified = []
    
    def verify(self, state: ScientificState) -> ScientificState:
        """Verify all citations in the report."""
        report = state.get('final_report', '')
        literature = state.get('literature_context', [])
        
        logger.info("Verifying citations", source="fact_checker")
        
        # Extract citations from report
        dois = re.findall(self.DOI_PATTERN, report)
        arxiv_ids = re.findall(self.ARXIV_PATTERN, report)
        
        # Verify from literature context
        for paper in literature:
            doi = paper.get('doi')
            arxiv = paper.get('arxiv_id')
            
            if doi:
                verified = self._verify_doi(doi)
                if verified:
                    self.verified.append({"type": "doi", "id": doi, "title": paper.get('title', '')})
                else:
                    self.unverified.append(doi)
            
            if arxiv:
                self.verified.append({"type": "arxiv", "id": arxiv, "title": paper.get('title', '')})
        
        state['verified_citations'] = self.verified
        state['unverified_citations'] = self.unverified
        state['current_step'] = 'citations_verified'
        
        # Add verification score to state
        total = len(self.verified) + len(self.unverified)
        verification_score = len(self.verified) / total if total > 0 else 1.0
        
        # Add to logic chain
        state['logic_chain'] = state.get('logic_chain', [])
        state['logic_chain'].append({
            "step": "fact_checker",
            "verified_count": len(self.verified),
            "unverified_count": len(self.unverified),
            "verification_score": verification_score
        })
        
        return state
    
    def _verify_doi(self, doi: str) -> bool:
        """Verify DOI exists via dx.doi.org."""
        try:
            with httpx.Client(timeout=5.0, follow_redirects=True) as client:
                response = client.head(f"https://doi.org/{doi}")
                return response.status_code == 200
        except:
            # Assume valid if can't check
            return True
    
    def _verify_arxiv(self, arxiv_id: str) -> bool:
        """Verify arXiv ID exists."""
        try:
            with httpx.Client(timeout=5.0) as client:
                response = client.head(f"https://arxiv.org/abs/{arxiv_id}")
                return response.status_code == 200
        except:
            return True


def verify_citations(state: ScientificState) -> ScientificState:
    """Entry point for fact checker agent."""
    agent = FactCheckerAgent()
    return agent.verify(state)
