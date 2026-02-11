#!/bin/bash

# Configuration
STEER_BIN="../apps/core/target/release/steer-core"
LOG_DIR="$HOME/.steer/logs"
GUARDIAN_LOG="$LOG_DIR/guardian.log"
CRASH_LOG="$LOG_DIR/crash.log"

mkdir -p "$LOG_DIR"

echo "🛡️  Steer Guardian Active. Monitoring steer-core..."
echo "------------------------------------------------"

while true; do
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] Starting steer-core..." >> "$GUARDIAN_LOG"
    
    # Run the core process
    $STEER_BIN
    EXIT_CODE=$?
    
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] Process exited with code: $EXIT_CODE" >> "$GUARDIAN_LOG"
    
    if [ $EXIT_CODE -eq 0 ]; then
        echo "✅ Steer Core exited normally (user quit). Guardian shutting down."
        break
    else
        echo "⚠️  CRASH DETECTED (Code $EXIT_CODE). Restarting in 3 seconds..."
        echo "⚠️  CRASH DETECTED (Code $EXIT_CODE). See $CRASH_LOG for details." >> "$GUARDIAN_LOG"
        sleep 3
    fi
done

echo "🛡️  Guardian Terminated."
