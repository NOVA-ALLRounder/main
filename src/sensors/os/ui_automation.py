import argparse
import logging
import time
from typing import Optional, Dict, Any

import uiautomation as auto

# Local imports (assuming running as module)
try:
    from .emit import EmitConfig, HttpEmitter, build_event
except ImportError:
    # Fallback for direct execution testing
    from emit import EmitConfig, HttpEmitter, build_event

logger = logging.getLogger(__name__)

class UIAutomationSensor:
    def __init__(self, emitter: HttpEmitter, interval: float = 1.0):
        self._emitter = emitter
        self.interval = interval
        self.last_data_hash = None
        
    def run(self):
        logger.info(f"UI Automation Sensor started. Interval: {self.interval}s")
        while True:
            try:
                self.capture_and_send()
            except Exception as e:
                logger.error(f"Error in UI Automation Sensor: {e}")
            finally:
                time.sleep(self.interval)

    def capture_and_send(self):
        try:
            # Get the focused element
            element = auto.GetFocusedControl()
            if not element:
                return

            # Quick check to avoid heavy processing if nothing changed (optional optimization)
            # For now, we rely on semantic deduplication.

            # 1. Process & Window Info
            process_id = element.ProcessId
            process_name = "unknown"
            try:
                # auto.GetProcessName is not standard, use psutil if needed or uiautomation helper
                # generic approach:
                pass 
            except:
                pass
                
            # auto.Control has ProcessId. 
            # We can try to get process name via simple lookup if needed, 
            # but element.Name and ControlType are most important.
            
            # 2. Element Info
            control_type = element.ControlTypeName
            name = element.Name
            try:
                # Some controls don't support ValuePattern at all - silently handle
                value = element.GetValuePattern().Value
            except:
                value = ""

            automation_id = element.AutomationId
            
            # Top level window
            window = element.GetTopLevelControl()
            window_title = window.Name if window else "Unknown"
            window_handle = window.NativeWindowHandle if window else 0

            # 3. Construct Payload
            payload = {
                "window_title": window_title,
                "control_type": control_type,
                "element_name": name,
                "element_value": value,
                "automation_id": automation_id,
                "bounding_rect": str(element.BoundingRectangle),
            }
            
            # 4. Deduplication
            # Create a simple hash/signature of the current state
            current_hash = hash(f"{window_title}|{control_type}|{name}|{value}")
            
            if current_hash == self.last_data_hash:
                return

            self.last_data_hash = current_hash
            
            # 5. Build and Send Event
            # Note: We don't have easy access to app name here without psutil, 
            # but window_title is often enough for "app".
            # We can leave 'app' as 'unknown' or try to derive it.
            
            event = build_event(
                source="ui_automation",
                app=window_title, # Fallback to window title as app identifier
                event_type="user.interaction",
                resource_type="ui_element",
                resource_id=automation_id or name or "unknown",
                payload=payload,
                priority="P2", # P2 = High priority, but not critical system event
                window_id=str(window_handle),
                pid=process_id
            )
            
            sent = self._emitter.send_event(event)
            if sent:
                # Optimized console output for user visibility
                action_desc = f"{control_type} '{name}'"
                if value:
                    action_desc += f" = '{value}'"
                print(f"[{window_title}] {action_desc}", flush=True)
            
        except Exception as e:
            logger.warning(f"Failed to capture UI state: {e}")

def parse_args():
    parser = argparse.ArgumentParser(description="UI Automation Sensor")
    parser.add_argument(
        "--ingest-url",
        default="http://127.0.0.1:8080/events",
        help="Collector ingest URL"
    )
    parser.add_argument(
        "--interval",
        type=float,
        default=1.0,
        help="Polling interval in seconds"
    )
    return parser.parse_args()

def main():
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")
    args = parse_args()
    
    config = EmitConfig(ingest_url=args.ingest_url)
    emitter = HttpEmitter(config)
    
    sensor = UIAutomationSensor(emitter, interval=args.interval)
    sensor.run()

if __name__ == "__main__":
    main()
