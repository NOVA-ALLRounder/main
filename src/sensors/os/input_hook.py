"""
Input Hook Sensor - Real-time keyboard and mouse event capture.

Captures:
- Mouse clicks (left, right, middle button with position)
- Special keys (Enter, Tab, Escape, function keys, shortcuts)
- Does NOT capture regular character keys for privacy
"""

import argparse
import logging
import threading
import time
from typing import Optional

from pynput import mouse, keyboard

try:
    from .emit import EmitConfig, HttpEmitter, build_event
except ImportError:
    from emit import EmitConfig, HttpEmitter, build_event

logger = logging.getLogger(__name__)

# Keys that are safe to log (no privacy concern)
LOGGABLE_SPECIAL_KEYS = {
    keyboard.Key.enter, keyboard.Key.tab, keyboard.Key.space,
    keyboard.Key.backspace, keyboard.Key.delete,
    keyboard.Key.up, keyboard.Key.down, keyboard.Key.left, keyboard.Key.right,
    keyboard.Key.home, keyboard.Key.end, keyboard.Key.page_up, keyboard.Key.page_down,
    keyboard.Key.esc, keyboard.Key.insert,
    keyboard.Key.f1, keyboard.Key.f2, keyboard.Key.f3, keyboard.Key.f4,
    keyboard.Key.f5, keyboard.Key.f6, keyboard.Key.f7, keyboard.Key.f8,
    keyboard.Key.f9, keyboard.Key.f10, keyboard.Key.f11, keyboard.Key.f12,
}

# Modifier keys to track for shortcuts
MODIFIER_KEYS = {
    keyboard.Key.ctrl, keyboard.Key.ctrl_l, keyboard.Key.ctrl_r,
    keyboard.Key.alt, keyboard.Key.alt_l, keyboard.Key.alt_r, keyboard.Key.alt_gr,
    keyboard.Key.shift, keyboard.Key.shift_l, keyboard.Key.shift_r,
    keyboard.Key.cmd, keyboard.Key.cmd_l, keyboard.Key.cmd_r,
}


class InputHookSensor:
    def __init__(self, emitter: HttpEmitter, debounce_ms: int = 100):
        self._emitter = emitter
        self._debounce_ms = debounce_ms
        self._last_click_time = 0.0
        self._last_key_time = 0.0
        self._active_modifiers = set()
        self._running = False
        
    def run(self):
        logger.info("Input Hook Sensor started")
        self._running = True
        
        mouse_listener = mouse.Listener(
            on_click=self._on_click,
            on_scroll=self._on_scroll
        )
        keyboard_listener = keyboard.Listener(
            on_press=self._on_key_press,
            on_release=self._on_key_release
        )
        
        mouse_listener.start()
        keyboard_listener.start()
        
        try:
            while self._running:
                time.sleep(0.1)
        except KeyboardInterrupt:
            pass
        finally:
            mouse_listener.stop()
            keyboard_listener.stop()
            logger.info("Input Hook Sensor stopped")
    
    def stop(self):
        self._running = False
    
    def _should_debounce(self, last_time: float) -> bool:
        now = time.time() * 1000  # ms
        return (now - last_time) < self._debounce_ms
    
    def _on_click(self, x: int, y: int, button: mouse.Button, pressed: bool):
        if not pressed:
            return
            
        if self._should_debounce(self._last_click_time):
            return
        self._last_click_time = time.time() * 1000
        
        button_name = button.name if hasattr(button, 'name') else str(button)
        
        payload = {
            "x": x,
            "y": y,
            "button": button_name,
            "action": "click"
        }
        
        event = build_event(
            source="input_hook",
            app="system",
            event_type="mouse.click",
            resource_type="input",
            resource_id=f"mouse_{button_name}",
            payload=payload,
            priority="P2"
        )
        
        sent = self._emitter.send_event(event)
        if sent:
            print(f"[CLICK] ({x}, {y}) {button_name}", flush=True)
    
    def _on_scroll(self, x: int, y: int, dx: int, dy: int):
        # Only log significant scrolls to reduce noise
        if abs(dy) < 3:
            return
            
        direction = "down" if dy < 0 else "up"
        
        payload = {
            "x": x,
            "y": y,
            "direction": direction,
            "delta": abs(dy)
        }
        
        event = build_event(
            source="input_hook",
            app="system",
            event_type="mouse.scroll",
            resource_type="input",
            resource_id=f"scroll_{direction}",
            payload=payload,
            priority="P2"
        )
        
        self._emitter.send_event(event)
    
    def _on_key_press(self, key):
        # Track modifier keys
        if key in MODIFIER_KEYS:
            self._active_modifiers.add(key)
            return
        
        # Check for shortcuts (modifier + key)
        if self._active_modifiers:
            self._handle_shortcut(key)
            return
        
        # Only log special keys
        if key not in LOGGABLE_SPECIAL_KEYS:
            return
            
        if self._should_debounce(self._last_key_time):
            return
        self._last_key_time = time.time() * 1000
        
        key_name = key.name if hasattr(key, 'name') else str(key)
        
        payload = {
            "key": key_name,
            "action": "press"
        }
        
        event = build_event(
            source="input_hook",
            app="system",
            event_type="keyboard.special_key",
            resource_type="input",
            resource_id=f"key_{key_name}",
            payload=payload,
            priority="P2"
        )
        
        sent = self._emitter.send_event(event)
        if sent:
            print(f"[KEY] {key_name}", flush=True)
    
    def _on_key_release(self, key):
        if key in MODIFIER_KEYS:
            self._active_modifiers.discard(key)
    
    def _handle_shortcut(self, key):
        """Handle keyboard shortcuts like Ctrl+S, Ctrl+C, etc."""
        if self._should_debounce(self._last_key_time):
            return
        self._last_key_time = time.time() * 1000
        
        # Build modifier string
        modifiers = []
        for mod in self._active_modifiers:
            mod_name = mod.name if hasattr(mod, 'name') else str(mod)
            # Normalize modifier names
            if 'ctrl' in mod_name:
                if 'Ctrl' not in modifiers:
                    modifiers.append('Ctrl')
            elif 'alt' in mod_name:
                if 'Alt' not in modifiers:
                    modifiers.append('Alt')
            elif 'shift' in mod_name:
                if 'Shift' not in modifiers:
                    modifiers.append('Shift')
            elif 'cmd' in mod_name:
                if 'Win' not in modifiers:
                    modifiers.append('Win')
        
        # Get key name
        if hasattr(key, 'name'):
            key_name = key.name
        elif hasattr(key, 'char') and key.char:
            key_name = key.char.upper()
        else:
            key_name = str(key)
        
        shortcut = '+'.join(modifiers + [key_name])
        
        payload = {
            "shortcut": shortcut,
            "modifiers": modifiers,
            "key": key_name,
            "action": "shortcut"
        }
        
        event = build_event(
            source="input_hook",
            app="system",
            event_type="keyboard.shortcut",
            resource_type="input",
            resource_id=f"shortcut_{shortcut.replace('+', '_')}",
            payload=payload,
            priority="P1"  # Shortcuts are higher priority
        )
        
        sent = self._emitter.send_event(event)
        if sent:
            print(f"[SHORTCUT] {shortcut}", flush=True)


def parse_args():
    parser = argparse.ArgumentParser(description="Input Hook Sensor")
    parser.add_argument(
        "--ingest-url",
        default="http://127.0.0.1:8080/events",
        help="Collector ingest URL"
    )
    parser.add_argument(
        "--debounce-ms",
        type=int,
        default=100,
        help="Debounce interval in milliseconds"
    )
    return parser.parse_args()


def main():
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s %(levelname)s %(message)s"
    )
    args = parse_args()
    
    config = EmitConfig(ingest_url=args.ingest_url)
    emitter = HttpEmitter(config)
    
    sensor = InputHookSensor(emitter, debounce_ms=args.debounce_ms)
    
    print("Input Hook Sensor running. Press Ctrl+C to stop.")
    print("Capturing: Mouse clicks, Special keys, Shortcuts")
    print("NOT capturing: Regular character keys (for privacy)")
    print("-" * 50)
    
    sensor.run()


if __name__ == "__main__":
    main()
