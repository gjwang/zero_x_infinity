"""
Audit Logging Middleware
AC-13: All admin operations must be logged
UX-10: Trace ID evidence chain with ULID
"""

import json
from contextvars import ContextVar
from datetime import datetime
from typing import Callable, Optional

from fastapi import Request, Response
from starlette.middleware.base import BaseHTTPMiddleware
from starlette.responses import Response as StarletteResponse
from sqlalchemy.ext.asyncio import AsyncSession
from ulid import ULID

from models import AdminAuditLog


# UX-10: ContextVar for trace_id propagation across async boundaries
trace_id_var: ContextVar[str] = ContextVar("trace_id", default="")


def get_trace_id() -> str:
    """Get current trace ID from context"""
    return trace_id_var.get()


def generate_trace_id() -> str:
    """Generate new ULID trace ID (26 chars)"""
    return str(ULID())


class AuditLogMiddleware(BaseHTTPMiddleware):
    """
    Middleware to log all admin operations.
    Captures: trace_id (ULID), admin_id, IP, action, path, entity info, timestamps
    
    UX-10 Requirements:
    - TC-UX-10-01: Each request gets unique ULID
    - TC-UX-10-02: All logs include trace_id  
    - TC-UX-10-03: Response header X-Trace-ID
    - TC-UX-10-04: audit_log has trace_id column
    - TC-UX-10-05: Logs and DB use same trace_id
    - TC-UX-10-06: Trace ID is 26 chars (ULID)
    """
    
    # Paths that trigger audit logging
    AUDITED_METHODS = {"POST", "PUT", "PATCH", "DELETE"}
    AUDITED_PATH_PREFIXES = ["/admin/AssetAdmin", "/admin/SymbolAdmin", "/admin/VIPLevelAdmin"]
    
    async def dispatch(self, request: Request, call_next: Callable) -> Response:
        # UX-10: Generate ULID trace_id for every request
        trace_id = generate_trace_id()
        trace_id_var.set(trace_id)
        
        # Log for all requests (with trace_id)
        print(f"[{trace_id}] {request.method} {request.url.path}")
        
        # Only audit modifying operations on specific paths
        if not self._should_audit(request):
            response = await call_next(request)
            # UX-10: Add trace_id to all responses
            response.headers["X-Trace-ID"] = trace_id
            return response
        
        print(f"[{trace_id}] AUDIT START: {request.method} {request.url.path}")
        
        # Capture body through receive wrapper
        body_bytes = b""
        original_receive = request.receive
        
        async def receive_wrapper():
            nonlocal body_bytes
            message = await original_receive()
            if message.get("type") == "http.request":
                body_chunk = message.get("body", b"")
                body_bytes += body_chunk
            return message
        
        request._receive = receive_wrapper
        
        # Execute the request
        response = await call_next(request)
        
        print(f"[{trace_id}] RESPONSE: status={response.status_code}")
        
        # Log after successful operations
        if 200 <= response.status_code < 300:
            body = None
            if body_bytes:
                try:
                    body = json.loads(body_bytes.decode())
                except Exception:
                    body = None
            await self._create_audit_log(request, body, trace_id)
            print(f"[{trace_id}] AUDIT LOG CREATED")
        
        # UX-10: Add trace_id to response header
        response.headers["X-Trace-ID"] = trace_id
        
        print(f"[{trace_id}] AUDIT END")
        return response
    
    def _should_audit(self, request: Request) -> bool:
        """Check if this request should be audited"""
        if request.method not in self.AUDITED_METHODS:
            return False
        
        path = request.url.path
        return any(path.startswith(prefix) for prefix in self.AUDITED_PATH_PREFIXES)
    
    def _extract_entity_info(self, path: str) -> tuple[Optional[str], Optional[int]]:
        """Extract entity_type and entity_id from path like /admin/asset/123"""
        parts = path.strip("/").split("/")
        entity_type = None
        entity_id = None
        
        if len(parts) >= 2:
            entity_type = parts[1]
        
        if len(parts) >= 3:
            try:
                entity_id = int(parts[2])
            except ValueError:
                pass
        
        return entity_type, entity_id
    
    async def _create_audit_log(self, request: Request, body: Optional[dict], trace_id: str) -> None:
        """Create audit log entry with trace_id"""
        from database import SessionLocal
        
        # Get admin info
        admin_id = getattr(request.state, "admin_id", 0)
        admin_username = getattr(request.state, "admin_username", "unknown")
        
        if not admin_id and "user" in request.scope:
            try:
                user = request.user
                admin_id = getattr(user, "id", 0)
                admin_username = getattr(user, "username", "unknown")
            except Exception:
                pass
        
        ip_address = request.client.host if request.client else "unknown"
        entity_type, entity_id = self._extract_entity_info(request.url.path)
        
        db: Optional[AsyncSession] = getattr(request.state, "db", None)
        session_to_use = db
        should_close = False
        
        if session_to_use is None:
            if SessionLocal is None:
                print(f"[{trace_id}] AUDIT ERROR: Database not initialized")
                return
            session_to_use = SessionLocal()
            should_close = True
            
        try:
            log_entry = AdminAuditLog(
                trace_id=trace_id,  # UX-10: Store trace_id
                admin_id=admin_id,
                admin_username=admin_username,
                ip_address=ip_address,
                action=request.method,
                path=str(request.url.path)[:256],
                entity_type=entity_type,
                entity_id=entity_id,
                new_value=body,
                created_at=datetime.utcnow(),
            )
            session_to_use.add(log_entry)
            await session_to_use.commit()
        except Exception as e:
            print(f"[{trace_id}] AUDIT ERROR: {e}")
            if session_to_use:
                await session_to_use.rollback()
        finally:
            if should_close and session_to_use:
                await session_to_use.close()
