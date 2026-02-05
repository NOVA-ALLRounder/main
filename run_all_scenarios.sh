#!/bin/bash
set -e

echo "🚀 Starting Advanced Scenarios 1-5 Execution..."
echo "⚠️  PLEASE DO NOT TOUCH THE MOUSE/KEYBOARD DURING EXECUTION"
echo ""

# Scenario 1: Calendar (Read)
echo "---------------------------------------------------"
echo "📅 Scenario 1: Calendar Check"
echo "Goal: 'Check my calendar for today and tell me the first event.'"
cargo run --manifest-path core/Cargo.toml --bin local_os_agent -- surf "Check my calendar for today and tell me the first event."
echo "✅ Scenario 1 Complete."
sleep 5

# Scenario 2: Meeting Summary (Write)
echo "---------------------------------------------------"
echo "📝 Scenario 2: Meeting Summary"
echo "Goal: 'Open Notes app, create a new note titled 'Daily Standup', and write 'All systems go'.'"
cargo run --manifest-path core/Cargo.toml --bin local_os_agent -- surf "Open Notes app, create a new note titled 'Daily Standup', and write 'All systems go'."
echo "✅ Scenario 2 Complete."
sleep 5

# Scenario 3: Research (Browser)
echo "---------------------------------------------------"
echo "🌐 Scenario 3: Web Research"
echo "Goal: 'Open Safari, search for 'DeepSeek R1', and tell me what it is.'"
cargo run --manifest-path core/Cargo.toml --bin local_os_agent -- surf "Open Safari, search for 'DeepSeek R1', and tell me what it is."
echo "✅ Scenario 3 Complete."
sleep 5

# Scenario 4: Finder/System (Navigation)
echo "---------------------------------------------------"
echo "📂 Scenario 4: Finder Navigation"
echo "Goal: 'Open Finder, list the files in the Downloads folder, and read the first filename.'"
cargo run --manifest-path core/Cargo.toml --bin local_os_agent -- surf "Open Finder, list the files in the Downloads folder, and read the first filename."
echo "✅ Scenario 4 Complete."
sleep 5

# Scenario 5: Complex (Multi-App)
echo "---------------------------------------------------"
echo "🔗 Scenario 5: Complex Workflow"
echo "Goal: 'Open Safari, check the current stock price of Apple (AAPL), then open Notes and record it.'"
cargo run --manifest-path core/Cargo.toml --bin local_os_agent -- surf "Open Safari, check the current stock price of Apple (AAPL), then open Notes and record it."
echo "✅ Scenario 5 Complete."

echo ""
echo "🎉 All 5 Scenarios Executed Successfully."
