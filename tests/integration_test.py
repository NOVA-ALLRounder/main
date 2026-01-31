import subprocess
import json
import time
import sys

def send_request(process, req_id, action, payload=None):
    req = {
        "id": req_id,
        "action": action,
        "payload": payload or {}
    }
    process.stdin.write(json.dumps(req) + "\n")
    process.stdin.flush()
    line = process.stdout.readline()
    if not line:
        return {"status": "error", "error": "EOF"}
    try:
        return json.loads(line)
    except json.JSONDecodeError:
        return {"status": "error", "error": f"Invalid JSON: {line}"}

def test_integration():
    print("🚀 Starting Integration Test (Python -> Swift Adapter)...")
    # Correct path relative to project root (local-os-agent/)
    adapter_path = "./adapter/.build/debug/adapter"
    
    try:
        process = subprocess.Popen(
            [adapter_path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1
        )
    except FileNotFoundError:
        print(f"❌ Adapter not found at {adapter_path}")
        return

    print(f"✅ Adapter spawned (PID: {process.pid})")
    time.sleep(1)

    # Test 1: Snapshot
    print("\n[Test 1] UiSnapshot...")
    resp = send_request(process, "t1", "ui.snapshot")
    if resp.get("status") == "success":
        print("✅ Snapshot Success")
    else:
        print(f"❌ Snapshot Failed: {resp}")

    # Test 2: Open Calculator
    print("\n[Test 2] Open Calculator...")
    resp = send_request(process, "t2", "system.open", {"app": "Calculator"})
    if resp.get("status") == "success":
        print("✅ Open Success")
    else:
        print(f"❌ Open Failed: {resp}")
    
    # Wait for Calculator to actually open and focus
    print("   Waiting 2s for app launch...")
    time.sleep(2)

    # Test 3: Type 123
    print("\n[Test 3] Type '123'...")
    resp = send_request(process, "t3", "ui.type", {"text": "123"})
    if resp.get("status") == "success":
        print("✅ Type Success")
    else:
        print(f"❌ Type Failed: {resp}")

    print("\n🛑 Terminating Adapter...")
    process.terminate()
    print("Done.")

if __name__ == "__main__":
    test_integration()
