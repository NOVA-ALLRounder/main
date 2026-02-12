#!/bin/bash

echo "🔪 Killing processes..."

# 1. Kill Backend (Port 5680)
# Check if lsof exists
if command -v lsof >/dev/null 2>&1; then
    PID_5680=$(lsof -t -i:5680)
    if [ -n "$PID_5680" ]; then
        echo "   - Found Backend on port 5680 (PID: $PID_5680). Killing..."
        kill -9 $PID_5680 2>/dev/null
    else
        echo "   - Port 5680 is free."
    fi

    # 2. Kill Frontend (Port 5174)
    PID_5174=$(lsof -t -i:5174)
    if [ -n "$PID_5174" ]; then
        echo "   - Found Frontend on port 5174 (PID: $PID_5174). Killing..."
        kill -9 $PID_5174 2>/dev/null
    else
        echo "   - Port 5174 is free."
    fi
    
    # Check default Vite port 5173 just in case
    PID_5173=$(lsof -t -i:5173)
    if [ -n "$PID_5173" ]; then
        echo "   - Found Frontend on port 5173 (PID: $PID_5173). Killing..."
        kill -9 $PID_5173 2>/dev/null
    fi
else
    echo "⚠️  'lsof' not found. Trying pkill..."
fi

# 3. Force Kill by Name (Backup)
echo "   - Cleaning up 'local_os_agent' process..."
pkill -f "local_os_agent" 2>/dev/null

echo "✅ All clean."
