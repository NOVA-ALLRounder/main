import subprocess
import json
import time

def test_integration():
    print("🚀 Starting Integration Test (Python -> Swift Adapter)...")
    
    # 1. Spawn Swift Adapter
    adapter_path = "./local-os-agent/adapter/.build/debug/adapter"
    process = subprocess.Popen(
        [adapter_path],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1  # Line buffered
    )
    
    print(f"✅ Adapter spawned (PID: {process.pid})")
    time.sleep(1) # Wait for startup
    
    # 2. Test Case 1: UI Snapshot (Observe)
    print("\n[Test 1] Sending 'ui.snapshot'...")
    req_1 = {
        "id": "test-1",
        "action": "ui.snapshot",
        "payload": {}
    }
    
    process.stdin.write(json.dumps(req_1) + "\n")
    process.stdin.flush()
    
    resp_1_line = process.stdout.readline()
    print(f"📥 Received: {resp_1_line.strip()}")
    
    try:
        resp_1 = json.loads(resp_1_line)
        if resp_1["request_id"] == "test-1" and resp_1["status"] == "success":
            print("✅ Test 1 PASSED: Snapshot success.")
        else:
            print(f"❌ Test 1 FAILED: Invalid response {resp_1}")
    except json.JSONDecodeError:
         print(f"❌ Test 1 FAILED: JSON Decode Error")

    # 3. Test Case 2: UI Click with Fake ID (Act & Error Handling)
    print("\n[Test 2] Sending 'ui.click' (Fake ID)...")
    req_2 = {
        "id": "test-2",
        "action": "ui.click",
        "payload": {"element_id": "fake_id_12345"}
    }
    
    process.stdin.write(json.dumps(req_2) + "\n")
    process.stdin.flush()
    
    resp_2_line = process.stdout.readline()
    print(f"📥 Received: {resp_2_line.strip()}")
    
    try:
        resp_2 = json.loads(resp_2_line)
        # We expect a failure because the element ID doesn't exist, but the IPC should succeed.
        if resp_2["request_id"] == "test-2" and resp_2["status"] == "failed":
            print(f"✅ Test 2 PASSED: Expected failure handled correctly. Error: {resp_2.get('error')}")
        else:
             print(f"❌ Test 2 FAILED: Unexpected status {resp_2}")
    except json.JSONDecodeError:
         print(f"❌ Test 2 FAILED: JSON Decode Error")

    # Cleanup
    print("\n🛑 Terminating Adapter...")
    process.terminate()
    process.wait()

if __name__ == "__main__":
    test_integration()
