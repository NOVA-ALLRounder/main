#!/usr/bin/env python3
"""
Virtual Science Lab - CLI Entry Point
Usage: python run.py "가설 또는 질문"
"""

import sys
from pathlib import Path

# 프로젝트 루트 추가
sys.path.insert(0, str(Path(__file__).parent.parent))

from workflow import run_vsl_workflow


def main():
    if len(sys.argv) < 2:
        print("=" * 60)
        print("🔬 Virtual Science Lab (VSL)")
        print("=" * 60)
        print("\nUsage: python run.py '가설 또는 질문'")
        print("\nExamples:")
        print("  python run.py '특정 단백질이 암세포 성장을 억제할 것이다'")
        print("  python run.py '양자 컴퓨팅이 현재 상용화 가능한가?'")
        sys.exit(1)
    
    user_input = " ".join(sys.argv[1:])
    
    print("\n" + "=" * 60)
    print("🔬 Virtual Science Lab - Autonomous Scientific Discovery")
    print("=" * 60)
    print(f"\n📝 Input: {user_input}\n")
    
    try:
        # 워크플로우 실행
        final_state = run_vsl_workflow(user_input)
        
        print("\n" + "=" * 60)
        print("✅ Analysis Complete!")
        print("=" * 60)
        
        # 결과 출력
        if final_state.get("intent"):
            print(f"\n📌 Intent: {final_state['intent']} (confidence: {final_state.get('intent_confidence', 0):.2f})")
        
        if final_state.get("domain"):
            print(f"🏷️  Domain: {final_state['domain']}")
        
        if final_state.get("literature_context"):
            print(f"\n📚 Found {len(final_state['literature_context'])} related papers")
        
        if final_state.get("novelty_score"):
            print(f"🆕 Novelty Score: {final_state['novelty_score']:.2f}")
        
        if final_state.get("proposed_methods"):
            print(f"\n🔧 Proposed {len(final_state['proposed_methods'])} methodologies")
            for m in final_state["proposed_methods"]:
                print(f"   [{m['method_id']}] {m['title']} ({m['approach_type']})")
        
        if final_state.get("final_report_markdown"):
            report_path = Path("output") / "report.md"
            report_path.parent.mkdir(exist_ok=True)
            report_path.write_text(final_state["final_report_markdown"], encoding="utf-8")
            print(f"\n📄 Report saved to: {report_path}")
        
    except Exception as e:
        print(f"\n❌ Error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
