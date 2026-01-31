
import subprocess
import time

def run_agent_test(goal):
    print(f"\n🚀 Testing Goal: '{goal}'")
    
    # Run cargo run with input
    process = subprocess.Popen(
        ["cargo", "run"],
        cwd="/Users/david/Desktop/python/github/Allrounder/Steer/local-os-agent/core",
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )
    
    input_str = f"unlock\nthink {goal}\n"
    
    try:
        stdout, stderr = process.communicate(input=input_str, timeout=30)
        
        # Check output for keywords
        output = stdout
        print("--- Output Summary ---")
        if "✅ Goal Achieved!" in output:
            print("✅ Status: SUCCESS")
        elif "❌" in output:
            print("❌ Status: FAILED")
        else:
            print("⚠️ Status: UNKNOWN/TIMEOUT")
            
        # Extract report if exists
        if "AGENT REPORT:" in output:
            report = output.split("AGENT REPORT:")[1].split("════")[0].strip()
            print(f"📋 Report: {report}")
            
    except subprocess.TimeoutExpired:
        process.kill()
        print("⏱️ Status: TIMEOUT (Agent took too long)")
    except Exception as e:
        print(f"❌ Error: {e}")

# Scenarios
scenarios = [
    "what is my os version",
    "what time is it now",
    "check internet connection with ping",
    "show top 3 processes",
    "check disk space usage"
]

for s in scenarios:
    run_agent_test(s)
    time.sleep(2)
