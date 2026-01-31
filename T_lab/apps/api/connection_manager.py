from typing import Dict, List
from fastapi import WebSocket
from datetime import datetime
import logging

logger = logging.getLogger("tlab.connection")

class WebSocketManager:
    """Manages WebSocket connections for real-time updates."""
    def __init__(self):
        self.active_connections: Dict[str, List[WebSocket]] = {}

    async def connect(self, websocket: WebSocket, session_id: str):
        await websocket.accept()
        if session_id not in self.active_connections:
            self.active_connections[session_id] = []
        self.active_connections[session_id].append(websocket)
        logger.info(f"WebSocket connected: {session_id}")

    def disconnect(self, websocket: WebSocket, session_id: str):
        if session_id in self.active_connections:
            if websocket in self.active_connections[session_id]:
                self.active_connections[session_id].remove(websocket)
            if not self.active_connections[session_id]:
                del self.active_connections[session_id]

    async def broadcast(self, session_id: str, message: dict):
        if session_id in self.active_connections:
            # Broadcast to all connections for this session
            for connection in self.active_connections[session_id]:
                try:
                    await connection.send_json(message)
                except Exception as e:
                    logger.error(f"WebSocket send failed: {e}")

    async def broadcast_state(self, session_id: str, 
                              current_node: str, 
                              status: str,
                              reasoning: str = None):
        """Broadcast pipeline state update with reasoning for Transparent Box."""
        await self.broadcast(session_id, {
            "type": "system_state",
            "current_node": current_node,
            "status": status,
            "reasoning": reasoning,
            "timestamp": datetime.now().isoformat()
        })

# Global Singleton
ws_manager = WebSocketManager()
