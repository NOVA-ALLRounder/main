"""
DAACS v3.0 Document System - Verification Templates
Adapted from DAACS V6 verification.py for document domain
"""

import re
import urllib.request
import urllib.error
from typing import Dict, List, Any, Optional


class DocumentVerificationTemplates:
    """Document-specific verification templates."""

    @staticmethod
    def verify_toc_match(plan: List[Dict], draft: str) -> Dict[str, Any]:
        """
        Verify that drafted headings match planned ToC.
        
        Args:
            plan: List of chapter plans with 'title' field
            draft: The markdown draft content
        """
        # Extract headings from draft (## or ### level)
        heading_pattern = r'^#{1,3}\s+(.+)$'
        draft_headings = re.findall(heading_pattern, draft, re.MULTILINE)
        draft_headings_normalized = [h.strip().lower() for h in draft_headings]
        
        # Extract planned titles
        planned_titles = [ch.get("title", "").strip().lower() for ch in plan]
        
        # Find mismatches
        missing_in_draft = [t for t in planned_titles if t and t not in draft_headings_normalized]
        extra_in_draft = [h for h in draft_headings_normalized if h not in planned_titles]
        
        ok = len(missing_in_draft) == 0
        
        return {
            "ok": ok,
            "reason": f"Missing sections: {missing_in_draft}" if missing_in_draft else "All planned sections present",
            "template": "verify_toc_match",
            "details": {
                "missing": missing_in_draft,
                "extra": extra_in_draft[:5]  # Limit output
            }
        }

    @staticmethod
    def verify_word_count(draft: str, target: int, tolerance: float = 0.2) -> Dict[str, Any]:
        """
        Verify word count is within tolerance of target.
        
        Args:
            draft: The draft content
            target: Target word count
            tolerance: Allowed deviation (default 20%)
        """
        words = len(draft.split())
        min_words = int(target * (1 - tolerance))
        max_words = int(target * (1 + tolerance))
        
        ok = min_words <= words <= max_words
        
        return {
            "ok": ok,
            "reason": f"Word count {words} (target: {target}, range: {min_words}-{max_words})",
            "template": "verify_word_count",
            "details": {"actual": words, "target": target}
        }

    @staticmethod
    def verify_no_placeholder(draft: str) -> Dict[str, Any]:
        """
        Detect leftover placeholders like [TODO], {{...}}, [INSERT], etc.
        """
        placeholder_patterns = [
            r'\[TODO[^\]]*\]',
            r'\{\{[^}]+\}\}',
            r'\[INSERT[^\]]*\]',
            r'\[PLACEHOLDER[^\]]*\]',
            r'<FILL_IN>',
            r'___+',  # Multiple underscores
        ]
        
        found = []
        for pattern in placeholder_patterns:
            matches = re.findall(pattern, draft, re.IGNORECASE)
            found.extend(matches[:3])  # Limit per pattern
        
        ok = len(found) == 0
        
        return {
            "ok": ok,
            "reason": f"Found placeholders: {found}" if found else "No placeholders found",
            "template": "verify_no_placeholder",
            "details": {"found": found[:10]}
        }

    @staticmethod
    def verify_reference_links(urls: List[str], timeout: int = 5) -> Dict[str, Any]:
        """
        Verify that reference URLs are not 404.
        Uses HEAD request to minimize bandwidth.
        """
        broken = []
        checked = 0
        
        for url in urls[:10]:  # Limit to 10 URLs
            if not url.startswith(('http://', 'https://')):
                continue
            checked += 1
            try:
                req = urllib.request.Request(url, method='HEAD')
                req.add_header('User-Agent', 'Mozilla/5.0 (compatible; LinkChecker/1.0)')
                urllib.request.urlopen(req, timeout=timeout)
            except urllib.error.HTTPError as e:
                if e.code >= 400:
                    broken.append(f"{url} ({e.code})")
            except Exception as e:
                broken.append(f"{url} (Error: {str(e)[:30]})")
        
        ok = len(broken) == 0
        
        return {
            "ok": ok,
            "reason": f"Broken links: {broken}" if broken else f"All {checked} links valid",
            "template": "verify_reference_links",
            "details": {"broken": broken, "checked": checked}
        }

    @staticmethod
    def verify_sentence_quality(draft: str, max_sentence_length: int = 50) -> Dict[str, Any]:
        """
        Check for overly long sentences that hurt readability.
        """
        # Split by sentence-ending punctuation
        sentences = re.split(r'[.!?]+', draft)
        long_sentences = []
        
        for s in sentences:
            words = len(s.split())
            if words > max_sentence_length:
                preview = s.strip()[:80] + "..."
                long_sentences.append(f"({words} words) {preview}")
        
        ok = len(long_sentences) == 0
        
        return {
            "ok": ok,
            "reason": f"Found {len(long_sentences)} overly long sentences" if long_sentences else "Sentence lengths OK",
            "template": "verify_sentence_quality",
            "details": {"long_sentences": long_sentences[:3]}
        }


# Template type mapping (similar to V6)
TYPE_TO_TEMPLATES = {
    "structure": ["verify_toc_match"],
    "content": ["verify_no_placeholder", "verify_sentence_quality"],
    "references": ["verify_reference_links"],
    "full": ["verify_toc_match", "verify_no_placeholder", "verify_sentence_quality", "verify_reference_links"],
}


def run_document_verification(
    verification_type: str,
    draft: str,
    plan: Optional[List[Dict]] = None,
    target_words: Optional[int] = None,
    reference_urls: Optional[List[str]] = None
) -> Dict[str, Any]:
    """
    Run document verification based on type.
    
    Args:
        verification_type: Type of verification (structure, content, references, full)
        draft: The document draft
        plan: Chapter plans (for structure check)
        target_words: Target word count
        reference_urls: List of URLs to check
        
    Returns:
        {
            "ok": bool,
            "verdicts": List[Dict],
            "summary": str
        }
    """
    templates = TYPE_TO_TEMPLATES.get(verification_type, ["verify_no_placeholder"])
    vt = DocumentVerificationTemplates()
    verdicts = []
    
    if "verify_toc_match" in templates and plan:
        verdicts.append(vt.verify_toc_match(plan, draft))
    
    if "verify_no_placeholder" in templates:
        verdicts.append(vt.verify_no_placeholder(draft))
    
    if "verify_sentence_quality" in templates:
        verdicts.append(vt.verify_sentence_quality(draft))
    
    if "verify_reference_links" in templates and reference_urls:
        verdicts.append(vt.verify_reference_links(reference_urls))
    
    if target_words:
        verdicts.append(vt.verify_word_count(draft, target_words))
    
    all_passed = all(v["ok"] for v in verdicts) if verdicts else True
    failed_reasons = [v["reason"] for v in verdicts if not v["ok"]]
    
    return {
        "ok": all_passed,
        "verdicts": verdicts,
        "summary": "All checks passed" if all_passed else f"Issues: {'; '.join(failed_reasons)}"
    }
