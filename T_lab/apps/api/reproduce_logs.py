
import sys
import os
import time
import threading
from datetime import datetime

# Setup environment
sys.path.append(os.path.dirname(os.path.abspath(__file__)))

from models import SessionModel, SessionLocal, init_db
from state import create_initial_state

def test_persistence():
    print("Initializing DB...")
    init_db()
    
    # Setup persistent store (mimic main.py)
    class PersistentStore:
        def create_session(self, user_input, domain=""):
            state = create_initial_state(user_input, domain)
            if "created_at" in state: del state["created_at"]
            db = SessionLocal()
            try:
                db_session = SessionModel(**state)
                db.add(db_session)
                db.commit()
                db.refresh(db_session)
                return {c.name: getattr(db_session, c.name) for c in db_session.__table__.columns}
            finally:
                db.close()

        def get_session(self, sid):
            db = SessionLocal()
            try:
                s = db.query(SessionModel).filter(SessionModel.session_id == sid).first()
                if s: return {c.name: getattr(s, c.name) for c in s.__table__.columns}
                return None
            finally:
                db.close()

        def add_log(self, sid, message, agent="System"):
            db = SessionLocal()
            try:
                s = db.query(SessionModel).filter(SessionModel.session_id == sid).first()
                if s:
                    logs = list(s.activity_log or [])
                    logs.append({"time": "00:00:00", "agent": agent, "message": message})
                    s.activity_log = logs
                    db.commit()
                    print(f"Added log: {message} | Total: {len(logs)}")
            finally:
                db.close()

        def update_session(self, sid, **updates):
            db = SessionLocal()
            try:
                s = db.query(SessionModel).filter(SessionModel.session_id == sid).first()
                if s:
                    for key, value in updates.items():
                        # The Fix
                        if key in ["activity_log", "session_id", "created_at"]:
                            continue
                        if hasattr(s, key):
                            setattr(s, key, value)
                    db.commit()
                    db.refresh(s)
                    return {c.name: getattr(s, c.name) for c in s.__table__.columns}
            finally:
                db.close()

    store = PersistentStore()
    
    # 1. Create Session
    print("\n--- Step 1: Create Session ---")
    state = store.create_session("Test Input")
    sid = state["session_id"]
    print(f"Session {sid} created. Logs: {state.get('activity_log')}")

    # 2. Simulate Background Task Start
    print("\n--- Step 2: Background Task Start ---")
    # Fetch state (logs=[])
    current_state = store.get_session(sid) 
    print(f"Fetched State Logs: {current_state.get('activity_log')}")

    # 3. Add Log 1
    print("\n--- Step 3: Add Log 1 ---")
    store.add_log(sid, "Log 1", "Engineer")
    
    # 4. Simulate Agent Execution (modifies state locally, but doesn't know about Log 1)
    print("\n--- Step 4: Agent Execution ---")
    # current_state still has logs=[] (stale)
    current_state["status"] = "running"
    current_state["current_step"] = "engineer"
    # Agent might return the state as is, or modified
    processed_state = current_state.copy()
    
    # 5. Update Session with Stale State
    print("\n--- Step 5: Update Session ---")
    # This passes processed_state which has logs=[] to update_session
    # If the fix works, it should IGNORE logs=[] and keep Log 1 in DB
    upt_state = store.update_session(sid, **processed_state)
    print(f"Updated State Logs in Return: {upt_state.get('activity_log')}")

    # 6. Verify DB
    print("\n--- Step 6: Verify Final DB ---")
    final_state = store.get_session(sid)
    print(f"Final DB Logs: {final_state.get('activity_log')}")
    
    if len(final_state.get('activity_log')) == 1:
        print("SUCCESS: Log preserved.")
    else:
        print("FAILURE: Log lost.")

if __name__ == "__main__":
    test_persistence()
