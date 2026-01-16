import json
from typing import Dict, Any, List
from langchain_core.messages import SystemMessage, HumanMessage
from langchain_core.prompts import ChatPromptTemplate
from langchain_openai import ChatOpenAI
from langchain_core.output_parsers import JsonOutputParser

from .state import AgentState, ChapterPlan, CritiqueItem
from .prompts import SystemPrompts
from .verification import run_document_verification
from .replanning import detect_failure_type, create_replan_response

# LLM 설정 (실제 환경에서는 환경변수에서 API 키 로드 필요)
llm = ChatOpenAI(model="gpt-4-turbo-preview", temperature=0)

class DAACSNodes:
    
    @staticmethod
    def planner_node(state: AgentState) -> Dict[str, Any]:
        """기획 에이전트: 사용자 요청을 분석하여 목차 생성"""
        print("--- [Planner] Planning Document Structure ---")
        prompt = ChatPromptTemplate.from_messages([
            ("system", SystemPrompts.PLANNER),
            ("user", "REQUEST: {task}")
        ])
        chain = prompt | llm | JsonOutputParser()
        
        try:
            result = chain.invoke({"task": state["task"]})
            # JSON 결과를 State 형식에 맞게 변환
            plans = []
            for item in result.get("toc", []):
                plans.append(ChapterPlan(
                    chapter_id=item["chapter_id"],
                    title=item["title"],
                    key_points=item.get("key_points", []),
                    estimated_words=item.get("estimated_words", 500),
                    dependencies=item.get("dependencies", []),
                    status="pending"
                ))
            return {"plan": plans}
        except Exception as e:
            print(f"Planning failed: {e}")
            return {"plan": []}

    @staticmethod
    def supervisor_node(state: AgentState) -> Dict[str, Any]:
        """감독관 에이전트: 다음 단계 결정"""
        print("--- [Supervisor] routing next step ---")
        
        # 1. 기획이 없으면 실패 처리
        if not state.get("plan"):
            return {"next_step": "STOP"}

        # 2. 모든 챕터가 완료되었는지 확인
        all_complete = all(ch["status"] == "complete" for ch in state["plan"])
        if all_complete:
            return {"next_step": "publisher"}

        # 3. 작성 중이거나 리뷰 중인 것이 있는지 확인
        # (단순화된 로직: 하나라도 pending이면 writer 호출)
        pending_chapters = [ch for ch in state["plan"] if ch["status"] == "pending"]
        if pending_chapters:
            return {"next_step": "writer"}
        
        writing_chapters = [ch for ch in state["plan"] if ch["status"] == "writing"]
        if writing_chapters:
             # 실제로는 병렬 실행 대기를 기다려야 함
            return {"next_step": "writer"}

        reviewing_chapters = [ch for ch in state["plan"] if ch["status"] == "reviewing"]
        if reviewing_chapters:
            return {"next_step": "reviewer"}
        
        # 4. Check for replanning based on failure history
        critique_history = state.get("critique_history", [])
        consecutive_failures = state.get("consecutive_failures", 0)
        
        if critique_history and consecutive_failures > 0:
            failure_type = detect_failure_type(critique_history)
            replan = create_replan_response(failure_type, consecutive_failures, max_failures=3)
            
            if replan["stop"]:
                print(f"[Supervisor] Stopping due to: {replan['reason']}")
                return {"next_step": "STOP", "failure_type": failure_type}
            
            if replan["next_agent"]:
                print(f"[Supervisor] Replanning: {replan['reason']} -> {replan['next_agent']}")
                return {"next_step": replan["next_agent"], "failure_type": failure_type}
            
        return {"next_step": "STOP"}

    @staticmethod
    def writer_node(state: AgentState) -> Dict[str, Any]:
        """작가 에이전트: 챕터별 초안 작성"""
        print("--- [Writer] Writing Draft ---")
        # 실제로는 병렬 처리를 위해 map-reduce 패턴이나 서브그래프 사용 필요
        # 여기서는 첫 번째 pending 챕터만 처리하는 단순화된 예시
        target_chapter = next((ch for ch in state["plan"] if ch["status"] == "pending"), None)
        
        if not target_chapter:
            return {}

        prompt = ChatPromptTemplate.from_messages([
            ("system", SystemPrompts.WRITER),
            ("user", "Write Chapter: {title}\nKey Points: {points}")
        ])
        chain = prompt | llm
        
        result = chain.invoke({
            "title": target_chapter["title"],
            "points": ", ".join(target_chapter["key_points"])
        })
        
        # 상태 업데이트
        target_chapter["status"] = "reviewing"
        # draft_refs 업데이트는 불변성을 고려하여 새로운 dict 반환 권장
        new_drafts = state.get("draft_refs", {}).copy()
        new_drafts[target_chapter["chapter_id"]] = result.content
        
        return {"draft_refs": new_drafts, "plan": state["plan"]}

    @staticmethod
    def reviewer_node(state: AgentState) -> Dict[str, Any]:
        """검수 에이전트: 초안 평가"""
        print("--- [Reviewer] Reviewing Draft ---")
        # 리뷰 대기 중인 챕터 찾기
        target_chapter = next((ch for ch in state["plan"] if ch["status"] == "reviewing"), None)
        if not target_chapter:
            return {}
            
        draft_content = state["draft_refs"].get(target_chapter["chapter_id"], "")
        
        prompt = ChatPromptTemplate.from_messages([
            ("system", SystemPrompts.REVIEWER),
            ("user", "Review this draft:\n{draft}")
        ])
        chain = prompt | llm | JsonOutputParser()
        
        try:
            result = chain.invoke({"draft": draft_content[:2000]}) # 길이 제한
            verdict = result.get("verdict", "REJECT")
            
            # Run automated verification (V6 Integration)
            print("--- [Reviewer] Running automated verification ---")
            auto_check = run_document_verification(
                verification_type="content",
                draft=draft_content,
                plan=state["plan"],
                target_words=target_chapter.get("estimated_words")
            )
            
            # Combine LLM verdict with automated checks
            if not auto_check["ok"]:
                print(f"[Verification Failed] {auto_check['summary']}")
                verdict = "REJECT"  # Override if automated checks fail
            
            consecutive_failures = state.get("consecutive_failures", 0)
            
            if verdict == "APPROVE" or verdict == "SOFT_ACCEPT":
                target_chapter["status"] = "complete"
                consecutive_failures = 0  # Reset on success
            else:
                # REJECT 시 다시 pending으로 돌려서 재작성 유도
                target_chapter["status"] = "pending"
                consecutive_failures += 1
            
            # 피드백 저장
            critique = CritiqueItem(
                round=state.get("revision_count", 0),
                type="content",
                location="entire_chapter",
                comment=str(result.get("issues", [])),
                fixed=False
            )
            return {
                "critique_history": [critique], 
                "plan": state["plan"],
                "consecutive_failures": consecutive_failures,
                "revision_count": state.get("revision_count", 0) + 1
            }
            
        except Exception as e:
            print(f"Review failed: {e}")
            target_chapter["status"] = "complete" # 에러 시 강제 통과 (Fail-safe)
            return {"plan": state["plan"], "consecutive_failures": 0}

    @staticmethod
    def publisher_node(state: AgentState) -> Dict[str, Any]:
        """출판 에이전트: 최종 산출물 생성"""
        print("--- [Publisher] Finalizing Artifact ---")
        return {"final_artifact_path": "output/final_document.md"}
