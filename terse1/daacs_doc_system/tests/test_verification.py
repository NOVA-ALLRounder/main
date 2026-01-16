"""
Basic tests for DAACS v3.0 Document System
Run with: pytest tests/ -v
"""

import pytest
from ..verification import DocumentVerificationTemplates, run_document_verification


class TestVerificationTemplates:
    
    def test_verify_no_placeholder_clean(self):
        """Test that clean text passes placeholder check"""
        draft = "This is a complete document with no placeholders."
        result = DocumentVerificationTemplates.verify_no_placeholder(draft)
        assert result["ok"] is True
    
    def test_verify_no_placeholder_with_todo(self):
        """Test that TODO placeholders are detected"""
        draft = "This document has a [TODO: add content] placeholder."
        result = DocumentVerificationTemplates.verify_no_placeholder(draft)
        assert result["ok"] is False
        assert "TODO" in str(result["details"])
    
    def test_verify_word_count_in_range(self):
        """Test word count within tolerance"""
        draft = " ".join(["word"] * 100)  # 100 words
        result = DocumentVerificationTemplates.verify_word_count(draft, target=100)
        assert result["ok"] is True
    
    def test_verify_word_count_out_of_range(self):
        """Test word count outside tolerance"""
        draft = " ".join(["word"] * 50)  # 50 words
        result = DocumentVerificationTemplates.verify_word_count(draft, target=100)
        assert result["ok"] is False
    
    def test_verify_toc_match_success(self):
        """Test that headings match plan"""
        plan = [{"title": "Introduction"}, {"title": "Methods"}]
        draft = "# Introduction\nSome text\n## Methods\nMore text"
        result = DocumentVerificationTemplates.verify_toc_match(plan, draft)
        assert result["ok"] is True
    
    def test_verify_toc_match_missing(self):
        """Test that missing sections are detected"""
        plan = [{"title": "Introduction"}, {"title": "Results"}]
        draft = "# Introduction\nSome text"
        result = DocumentVerificationTemplates.verify_toc_match(plan, draft)
        assert result["ok"] is False
        assert "results" in str(result["details"]["missing"]).lower()


class TestRunVerification:
    
    def test_full_verification_clean(self):
        """Test full verification on clean document"""
        draft = "# Introduction\n\nThis is a well-written document with proper content."
        result = run_document_verification(
            verification_type="content",
            draft=draft
        )
        assert result["ok"] is True
