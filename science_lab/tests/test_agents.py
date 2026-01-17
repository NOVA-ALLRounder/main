"""
Unit tests for Science Lab agents and workflow
"""
import sys
import os
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import unittest
from state import create_initial_state, ScientificState


class TestState(unittest.TestCase):
    """상태 관리 테스트"""
    
    def test_create_initial_state(self):
        """초기 상태 생성 테스트"""
        state = create_initial_state("테스트 가설입니다", "물리학")
        
        self.assertIn("session_id", state)
        self.assertEqual(state["user_input"], "테스트 가설입니다")
        self.assertEqual(state["domain"], "물리학")
        self.assertEqual(state["status"], "processing")
        self.assertEqual(state["intent"], "")
    
    def test_state_has_required_fields(self):
        """필수 필드 존재 확인"""
        state = create_initial_state("test")
        
        required_fields = [
            "session_id", "user_input", "domain", "created_at",
            "status", "intent", "literature_context", "proposed_methods"
        ]
        for field in required_fields:
            self.assertIn(field, state)


class TestRouterAgent(unittest.TestCase):
    """라우터 에이전트 테스트"""
    
    def test_classify_hypothesis(self):
        """가설 분류 테스트"""
        from agents.router import RouterAgent
        
        agent = RouterAgent()
        state = create_initial_state("카페인이 집중력을 향상시킬 것이다")
        result = agent.classify(state)
        
        self.assertIn("intent", result)
        self.assertIn(result["intent"], ["hypothesis", "question"])
    
    def test_classify_question(self):
        """질문 분류 테스트"""
        from agents.router import RouterAgent
        
        agent = RouterAgent()
        state = create_initial_state("상온 초전도체의 실용화가 가능한가?")
        result = agent.classify(state)
        
        self.assertIn("intent", result)


class TestPIAgent(unittest.TestCase):
    """PI 에이전트 테스트"""
    
    def test_propose_methods(self):
        """방법론 제안 테스트"""
        from agents.pi import PIAgent
        
        agent = PIAgent()
        state = create_initial_state("테스트 가설")
        state["literature_context"] = []
        
        result = agent.propose_methods(state)
        
        self.assertIn("proposed_methods", result)
        self.assertEqual(len(result["proposed_methods"]), 3)
        
        # 각 방법론 구조 확인
        for method in result["proposed_methods"]:
            self.assertIn("type", method)
            self.assertIn("title", method)
            self.assertIn("description", method)


class TestEngineerAgent(unittest.TestCase):
    """엔지니어 에이전트 테스트"""
    
    def test_generate_mock_code(self):
        """Mock 코드 생성 테스트"""
        from agents.engineer import EngineerAgent
        
        agent = EngineerAgent()
        state = create_initial_state("테스트")
        state["proposed_methods"] = [{
            "type": "simulation",
            "title": "시뮬레이션 테스트",
            "description": "테스트"
        }]
        state["selected_method_index"] = 0
        
        # 코드 생성만 테스트 (실행은 하지 않음)
        code = agent._generate_code_mock(state["proposed_methods"][0])
        
        self.assertIn("import", code)
        self.assertIn("matplotlib", code)


class TestTools(unittest.TestCase):
    """도구 모듈 테스트"""
    
    def test_arxiv_search_structure(self):
        """ArXiv 검색 구조 테스트"""
        from tools.arxiv_search import search_arxiv
        
        # 빈 검색어로 테스트 (API 호출 최소화)
        results = search_arxiv("quantum computing", max_results=1)
        
        # 결과가 있으면 구조 확인
        if results:
            self.assertIn("title", results[0])
            self.assertIn("source", results[0])
            self.assertEqual(results[0]["source"], "arxiv")
    
    def test_code_safety_validation(self):
        """코드 안전성 검사 테스트"""
        from tools.code_executor import validate_code_safety
        
        # 안전한 코드
        safe_code = "import numpy as np\nprint(np.array([1,2,3]))"
        is_safe, msg = validate_code_safety(safe_code)
        self.assertTrue(is_safe)
        
        # 위험한 코드
        dangerous_code = "import os\nos.system('rm -rf /')"
        is_safe, msg = validate_code_safety(dangerous_code)
        self.assertFalse(is_safe)


class TestDatabase(unittest.TestCase):
    """데이터베이스 테스트"""
    
    def test_session_save_and_load(self):
        """세션 저장/로드 테스트"""
        from database import save_session, load_session
        
        state = create_initial_state("테스트 입력", "테스트 도메인")
        session_id = state["session_id"]
        
        # 저장
        save_session(session_id, dict(state))
        
        # 로드
        loaded = load_session(session_id)
        
        self.assertIsNotNone(loaded)
        self.assertEqual(loaded["user_input"], "테스트 입력")


class TestWorkflow(unittest.TestCase):
    """워크플로우 테스트"""
    
    def test_simple_workflow(self):
        """간단한 워크플로우 실행 테스트"""
        from workflow import run_workflow_simple
        
        result = run_workflow_simple(
            "인공지능이 창의성을 가질 수 있을 것이다",
            "컴퓨터과학"
        )
        
        self.assertIn("session_id", result)
        self.assertIn("intent", result)
        self.assertIn("current_step", result)


if __name__ == "__main__":
    unittest.main()
