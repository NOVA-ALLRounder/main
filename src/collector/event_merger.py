"""
Event Merger - Correlates events from multiple sensors.

Merges related events within a time window to reduce noise and 
create more meaningful activity records.
"""

from __future__ import annotations

import logging
from collections import defaultdict
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from typing import Any, Dict, List, Optional

logger = logging.getLogger(__name__)


@dataclass
class MergedEvent:
    """A merged event combining data from multiple sources."""
    primary_event: Dict[str, Any]
    correlated_events: List[Dict[str, Any]] = field(default_factory=list)
    confidence: float = 1.0
    merged_at: datetime = field(default_factory=datetime.utcnow)
    
    @property
    def sources(self) -> List[str]:
        sources = [self.primary_event.get("source", "unknown")]
        for evt in self.correlated_events:
            src = evt.get("source", "unknown")
            if src not in sources:
                sources.append(src)
        return sources
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "primary": self.primary_event,
            "correlated": self.correlated_events,
            "sources": self.sources,
            "confidence": self.confidence,
            "merged_at": self.merged_at.isoformat() + "Z"
        }


class EventMerger:
    """
    Merges events from different sources based on time proximity and context.
    
    Correlation rules:
    1. UI Automation focus + Input Hook click → Confirmed click action
    2. Browser tab change + Browser click → Confirmed navigation
    3. Keyboard shortcut + UI change → Confirmed shortcut action
    """
    
    def __init__(
        self,
        window_ms: int = 500,
        max_buffer_size: int = 100,
        dedup_window_ms: int = 200
    ):
        self._window_ms = window_ms
        self._max_buffer_size = max_buffer_size
        self._dedup_window_ms = dedup_window_ms
        self._event_buffer: List[Dict[str, Any]] = []
        self._merged_events: List[MergedEvent] = []
        self._last_merged_hash: Optional[str] = None
    
    def add_event(self, event: Dict[str, Any]) -> Optional[MergedEvent]:
        """
        Add an event and attempt to merge with existing buffer.
        Returns a MergedEvent if a merge was performed, None otherwise.
        """
        # Parse timestamp
        ts = self._parse_timestamp(event.get("ts", ""))
        if not ts:
            ts = datetime.utcnow()
        
        event["_parsed_ts"] = ts
        
        # Check for duplicates
        if self._is_duplicate(event):
            return None
        
        # Add to buffer
        self._event_buffer.append(event)
        
        # Keep buffer size manageable
        if len(self._event_buffer) > self._max_buffer_size:
            self._event_buffer = self._event_buffer[-self._max_buffer_size:]
        
        # Try to merge
        return self._try_merge(event)
    
    def _parse_timestamp(self, ts_str: str) -> Optional[datetime]:
        if not ts_str:
            return None
        try:
            # Handle ISO format with Z suffix
            ts_str = ts_str.replace("Z", "+00:00")
            return datetime.fromisoformat(ts_str)
        except (ValueError, TypeError):
            return None
    
    def _is_duplicate(self, event: Dict[str, Any]) -> bool:
        """Check if event is a near-duplicate of a recent event."""
        event_hash = self._compute_event_hash(event)
        event_ts = event.get("_parsed_ts", datetime.utcnow())
        
        for buffered in reversed(self._event_buffer[-10:]):
            buffered_ts = buffered.get("_parsed_ts", datetime.utcnow())
            time_diff = abs((event_ts - buffered_ts).total_seconds() * 1000)
            
            if time_diff <= self._dedup_window_ms:
                if self._compute_event_hash(buffered) == event_hash:
                    return True
        
        return False
    
    def _compute_event_hash(self, event: Dict[str, Any]) -> str:
        """Compute a hash for deduplication."""
        source = event.get("source", "")
        event_type = event.get("event_type", "")
        resource_id = event.get("resource", {}).get("id", "")
        
        # Include key payload fields
        payload = event.get("payload", {})
        key_fields = [
            payload.get("element_name", ""),
            payload.get("window_title", ""),
            payload.get("key", ""),
            payload.get("button", "")
        ]
        
        return f"{source}:{event_type}:{resource_id}:{':'.join(str(f) for f in key_fields)}"
    
    def _try_merge(self, new_event: Dict[str, Any]) -> Optional[MergedEvent]:
        """Try to merge the new event with recent events in the buffer."""
        event_type = new_event.get("event_type", "")
        source = new_event.get("source", "")
        new_ts = new_event.get("_parsed_ts", datetime.utcnow())
        
        # Find correlatable events within time window
        correlatable = []
        for buffered in self._event_buffer[:-1]:  # Exclude the new event itself
            buffered_ts = buffered.get("_parsed_ts", datetime.utcnow())
            time_diff = abs((new_ts - buffered_ts).total_seconds() * 1000)
            
            if time_diff <= self._window_ms:
                if self._can_correlate(new_event, buffered):
                    correlatable.append(buffered)
        
        if not correlatable:
            return None
        
        # Create merged event
        merged = MergedEvent(
            primary_event=new_event,
            correlated_events=correlatable,
            confidence=self._compute_confidence(new_event, correlatable)
        )
        
        self._merged_events.append(merged)
        
        # Remove merged events from buffer to avoid re-merging
        for evt in correlatable:
            if evt in self._event_buffer:
                self._event_buffer.remove(evt)
        
        logger.debug(
            "Merged %d events: %s + %s",
            len(correlatable) + 1,
            source,
            [e.get("source") for e in correlatable]
        )
        
        return merged
    
    def _can_correlate(self, event1: Dict[str, Any], event2: Dict[str, Any]) -> bool:
        """Determine if two events can be correlated."""
        src1 = event1.get("source", "")
        src2 = event2.get("source", "")
        type1 = event1.get("event_type", "")
        type2 = event2.get("event_type", "")
        
        # Same source events are not correlated (they should be deduped)
        if src1 == src2:
            return False
        
        # UI Automation + Input Hook = Confirmed interaction
        if {src1, src2} == {"ui_automation", "input_hook"}:
            # Click + Focus change
            if "click" in type1 or "click" in type2:
                return True
            # Key press + UI change
            if ("keyboard" in type1 or "keyboard" in type2):
                return True
        
        # Browser extension + UI Automation = Confirmed browser action
        if {src1, src2} == {"browser_extension", "ui_automation"}:
            return True
        
        # Browser extension + Input Hook = Confirmed browser interaction
        if {src1, src2} == {"browser_extension", "input_hook"}:
            if "click" in type1 or "click" in type2:
                return True
        
        return False
    
    def _compute_confidence(
        self,
        primary: Dict[str, Any],
        correlated: List[Dict[str, Any]]
    ) -> float:
        """Compute confidence score for the merge."""
        # More correlated events = higher confidence
        base_confidence = 0.7
        per_event_bonus = 0.1
        
        confidence = base_confidence + (len(correlated) * per_event_bonus)
        
        # Cap at 1.0
        return min(1.0, confidence)
    
    def get_recent_merged(self, limit: int = 10) -> List[MergedEvent]:
        """Get recent merged events."""
        return self._merged_events[-limit:]
    
    def clear_buffer(self):
        """Clear the event buffer and merged events."""
        self._event_buffer.clear()
        self._merged_events.clear()
    
    def get_stats(self) -> Dict[str, Any]:
        """Get merger statistics."""
        sources = defaultdict(int)
        for evt in self._event_buffer:
            sources[evt.get("source", "unknown")] += 1
        
        return {
            "buffer_size": len(self._event_buffer),
            "merged_count": len(self._merged_events),
            "sources_in_buffer": dict(sources)
        }
