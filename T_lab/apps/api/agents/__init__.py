# =============================================================================
# T_lab Agents - Package Exports
# =============================================================================

from agents.router import classify_intent
from agents.librarian import search_literature
from agents.pi import evaluate_novelty, propose_methods
from agents.engineer import execute_experiment
from agents.experiment_runner import run_simulation
from agents.author import write_report
from agents.critic import review_report
from agents.fact_checker import verify_citations
from agents.paper_synthesizer import synthesize_paper

__all__ = [
    "classify_intent",
    "search_literature",
    "evaluate_novelty",
    "propose_methods",
    "execute_experiment",
    "run_simulation",
    "write_report",
    "review_report",
    "verify_citations",
    "synthesize_paper"
]
