import subprocess
import time
import os
from pathlib import Path

def test_monitoring():
    print("🚀 Starting Monitor Test...")
    
    core_path = "./target/debug/core"
    
    # Ensure Downloads exists
    home = str(Path.home())
    downloads = os.path.join(home, "Downloads")
    test_file = os.path.join(downloads, "agent_monitor_test.txt")
    
    if os.path.exists(test_file):
        os.remove(test_file)

    try:
        process = subprocess.Popen(
            [core_path],
            cwd="./core",
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1
        )
    except FileNotFoundError:
        print(f"❌ Core not found at {core_path}")
        return

    print(f"✅ Core spawned (PID: {process.pid})")
    time.sleep(3) # Wait for init

    # 1. Test Status Command
    print("📊 Testing 'status' command...")
    process.stdin.write("status\n")
    process.stdin.flush()
    time.sleep(1)

    # 2. Test File Watcher
    print(f"👀 Creating test file: {test_file}")
    with open(test_file, "w") as f:
        f.write("Hello Agent")
    
    time.sleep(10) # Give watcher time to react (FSEvents latency)

    print("🛑 Terminating...")
    process.terminate()
    
    stdout, _ = process.communicate()
    
    # Determine Success
    status_ok = "CPU:" in stdout and "RAM:" in stdout
    watcher_ok = "file_created" in stdout and "agent_monitor_test.txt" in stdout

    if status_ok:
        print("✅ SUCCESS: 'status' command returned valid metrics.")
    else:
        print("❌ FAILURE: 'status' output missing or invalid.")

    if watcher_ok:
        print("✅ SUCCESS: File creation detected.")
    else:
        print("❌ FAILURE: File creation NOT detected.")
        print(f"DEBUG STDOUT:\n{stdout}")

    # Cleanup
    if os.path.exists(test_file):
        os.remove(test_file)

if __name__ == "__main__":
    test_monitoring()
