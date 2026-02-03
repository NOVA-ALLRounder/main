#!/bin/bash
echo "🧪 Running System Verification Script..."
echo "1. [TEST] Opening 'TextEdit' (Visual App)..."
open -a TextEdit
sleep 2
echo "2. [TEST] Opening 'https://www.google.com' (Browser)..."
open "https://www.google.com"
echo "✅ Verification Complete. If you saw TextEdit and Google, your system is fine."
