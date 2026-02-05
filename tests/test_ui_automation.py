import logging
import sys
import time
from pathlib import Path
from unittest.mock import MagicMock

# Add project root and src to sys.path
project_root = Path(__file__).resolve().parents[1]
sys.path.append(str(project_root))
sys.path.append(str(project_root / "src"))

from sensors.os.ui_automation import UIAutomationSensor

def test_sensor():
    print("Starting UI Automation Sensor Test...")
    print("Please click on different windows and elements to see the output.")
    print("Press Ctrl+C to stop.")
    
    # Mock Emitter
    mock_emitter = MagicMock()
    
    def side_effect(event):
        print(f"[EMIT] Event: {event['payload']['element_name']} (Type: {event['payload']['control_type']}) in Window: {event['payload']['window_title']}")
        return True
        
    mock_emitter.send_event.side_effect = side_effect
    
    sensor = UIAutomationSensor(mock_emitter, interval=1.0)
    
    try:
        sensor.run()
    except KeyboardInterrupt:
        print("\nTest stopped.")

if __name__ == "__main__":
    logging.basicConfig(level=logging.CRITICAL) # Suppress internal logs
    test_sensor()
