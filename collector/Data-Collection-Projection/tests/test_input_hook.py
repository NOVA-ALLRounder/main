"""
Test for Input Hook Sensor.

Run this to verify the input hook sensor is working correctly.
Press Ctrl+C to stop.
"""

import logging
import sys
from pathlib import Path
from unittest.mock import MagicMock

# Add project root and src to sys.path
project_root = Path(__file__).resolve().parents[1]
sys.path.append(str(project_root))
sys.path.append(str(project_root / "src"))

from sensors.os.input_hook import InputHookSensor


def test_sensor():
    print("=" * 60)
    print("Input Hook Sensor Test")
    print("=" * 60)
    print()
    print("This test will capture:")
    print("  - Mouse clicks (left, right, middle)")
    print("  - Special keys (Enter, Tab, Escape, F1-F12, etc.)")
    print("  - Keyboard shortcuts (Ctrl+S, Ctrl+C, etc.)")
    print()
    print("Regular character keys are NOT captured (for privacy).")
    print()
    print("Press Ctrl+C to stop.")
    print("-" * 60)
    
    # Mock Emitter
    mock_emitter = MagicMock()
    
    def side_effect(event):
        event_type = event.get("event_type", "unknown")
        payload = event.get("payload", {})
        
        if event_type == "mouse.click":
            x = payload.get("x", 0)
            y = payload.get("y", 0)
            button = payload.get("button", "unknown")
            print(f"[MOCK] Click: ({x}, {y}) {button}")
        elif event_type == "keyboard.special_key":
            key = payload.get("key", "unknown")
            print(f"[MOCK] Special Key: {key}")
        elif event_type == "keyboard.shortcut":
            shortcut = payload.get("shortcut", "unknown")
            print(f"[MOCK] Shortcut: {shortcut}")
        elif event_type == "mouse.scroll":
            direction = payload.get("direction", "unknown")
            print(f"[MOCK] Scroll: {direction}")
        else:
            print(f"[MOCK] Event: {event_type}")
        
        return True
    
    mock_emitter.send_event.side_effect = side_effect
    
    sensor = InputHookSensor(mock_emitter, debounce_ms=100)
    
    try:
        sensor.run()
    except KeyboardInterrupt:
        print()
        print("-" * 60)
        print("Test stopped.")


if __name__ == "__main__":
    logging.basicConfig(level=logging.CRITICAL)  # Suppress internal logs
    test_sensor()
