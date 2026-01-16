#!/usr/bin/env python3
"""
DAACS v3.0 Document System - CLI Entry Point
Usage: python run.py "문서 주제 또는 요청"
"""

import sys
import os
from dotenv import load_dotenv

# Load environment variables
load_dotenv()

from workflow import create_workflow
from state import AgentState


def main():
    if len(sys.argv) < 2:
        print("Usage: python run.py '문서 주제 또는 요청'")
        print("Example: python run.py 'AI 스타트업 사업계획서 작성'")
        sys.exit(1)
    
    task = " ".join(sys.argv[1:])
    print(f"\n{'='*60}")
    print(f"📝 DAACS v3.0 Intelligent Document System")
    print(f"{'='*60}")
    print(f"Task: {task}\n")
    
    # Initialize state
    initial_state: AgentState = {
        "task": task,
        "plan": [],
        "draft_refs": {},
        "reference_data": [],
        "critique_history": [],
        "revision_count": 0,
        "next_step": "",
        "final_artifact_path": None,
        "failure_type": None,
        "consecutive_failures": 0
    }
    
    # Create and run workflow
    workflow = create_workflow()
    
    print("🚀 Starting workflow...\n")
    
    try:
        # Stream execution for visibility
        for step in workflow.stream(initial_state):
            node_name = list(step.keys())[0]
            print(f"✓ Completed: {node_name}")
        
        print(f"\n{'='*60}")
        print("✅ Document generation complete!")
        
        # Get final state
        final_state = workflow.invoke(initial_state)
        if final_state.get("final_artifact_path"):
            print(f"📄 Output: {final_state['final_artifact_path']}")
            
    except KeyboardInterrupt:
        print("\n⚠️ Workflow interrupted by user")
        sys.exit(130)
    except Exception as e:
        print(f"\n❌ Error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
