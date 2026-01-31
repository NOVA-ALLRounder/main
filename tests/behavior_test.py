import subprocess
import json
import time
import sys

def test_behavior_analysis():
    print("🚀 Starting Behavior Analysis Test (Triggering Shadow Analyzer)...")
    # We need to run the RUST CORE, not just the adapter, because the logic is in Core.
    # But Core is interactive CLI.
    # We will spawn Core, then pipe "type" commands to it.
    
    core_path = "./target/debug/core"
    
    try:
        process = subprocess.Popen(
            [core_path],
            cwd="./core",  # Run inside core/ so it finds .env
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1
        )
    except FileNotFoundError:
        print(f"❌ Core not found at {core_path}. Did you build it?")
        return

    print(f"✅ Core spawned (PID: {process.pid})")
    time.sleep(2) # Wait for init

    # 0. Unlock Policy
    print("🔓 Unlocking Write Policy...")
    process.stdin.write("unlock\n")
    process.stdin.flush()
    time.sleep(1)

    # Spam 25 actions to trigger the batch size of 20
    print("⚡️ Spamming 25 'type' commands to generate logs...")
    
    for i in range(25):
        cmd = "fake_log\n"
        process.stdin.write(cmd)
        process.stdin.flush()
        time.sleep(0.1)
        
        # Read a bit of output to keep buffer clear
        # But core output is async.
    
    print("⏳ Waiting for Analyzer to trigger (Check logs)...")
    time.sleep(10) # Give LLM time to reply
    
    print("🛑 Terminating...")
    process.terminate()
    
    # Analyze Output
    stdout, stderr = process.communicate()
    
    if "Shadow Analyzer" in stdout:
        print("✅ SUCCESS: Found '[Shadow Analyzer]' in output!")
    else:
        print("❌ FAILURE: Analyzer did not trigger.")
        
    if "TENDENCY:" in stdout:
        print("✅ SUCCESS: Found 'TENDENCY:' decision!")
    else:
        print("❌ FAILURE: No Tendency output.")

    print("\n--- STDOUT DUMP ---")
    # print(stdout[-2000:]) # Last 2000 chars

    with open("behavior_test_output.txt", "w") as f:
        f.write(stdout)
        f.write("\n\n--- STDERR ---\n")
        f.write(stderr)
        
    print("Results saved to behavior_test_output.txt")

if __name__ == "__main__":
    test_behavior_analysis()
