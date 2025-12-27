"""
Audit Logging Middleware
AC-13: All admin operations must be logged
"""

import json
from datetime import datetime
from typing import Callable, Optional

from fastapi import Request, Response
from starlette.middleware.base import BaseHTTPMiddleware
from sqlalchemy.ext.asyncio import AsyncSession

from models import AdminAuditLog


class AuditLogMiddleware(BaseHTTPMiddleware):
    """
    Middleware to log all admin operations.
    Captures: admin_id, IP, action, path, entity info, timestamps
    """
    
    # Paths that trigger audit logging
    AUDITED_METHODS = {"POST", "PUT", "PATCH", "DELETE"}
    AUDITED_PATH_PREFIXES = ["/admin/AssetAdmin", "/admin/SymbolAdmin", "/admin/VIPLevelAdmin"]
    
    async def dispatch(self, request: Request, call_next: Callable) -> Response:
        import uuid
        trace_id = str(uuid.uuid4())[:8]
        
        # Only audit modifying operations on specific paths
        if not self._should_audit(request):
            return await call_next(request)
        
        print(f"[{trace_id}] AUDIT START: {request.method} {request.url.path}")
        
        # DON'T read body here - it will be consumed and downstream handlers get empty body!
        # Instead, we capture the body through a custom receive wrapper
        
        body_bytes = b""
        
        # Wrap receive to capture body bytes
        original_receive = request.receive
        async def receive_wrapper():
            nonlocal body_bytes
            message = await original_receive()
            if message.get("type") == "http.request":
                body_chunk = message.get("body", b"")
                body_bytes += body_chunk
                print(f"[{trace_id}] RECEIVE: {len(body_chunk)} bytes captured")
            return message
        
        # Replace receive function
        request._receive = receive_wrapper
        
        print(f"[{trace_id}] CALLING downstream handler...")
        
        # Execute the request (downstream can read body normally)
        response = await call_next(request)
        
        print(f"[{trace_id}] RESPONSE: status={response.status_code}, body_captured={len(body_bytes)} bytes")
        
        # Log after successful operations using captured body
        if 200 <= response.status_code < 300:
            body = None
            if body_bytes:
                try:
                    body = json.loads(body_bytes.decode())
                    print(f"[{trace_id}] BODY PARSED: {body}")
                except Exception as e:
                    print(f"[{trace_id}] BODY PARSE FAILED: {e}")
                    body = None
            await self._create_audit_log(request, body)
            print(f"[{trace_id}] AUDIT LOG CREATED")
        
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
            entity_type = parts[1]  # asset, symbol, vip_level
        
        if len(parts) >= 3:
            try:
                entity_id = int(parts[2])
            except ValueError:
                pass
        
        return entity_type, entity_id
    
    async def _create_audit_log(self, request: Request, body: Optional[dict]) -> None:
        """Create audit log entry"""
        from database import SessionLocal
        
        # 1. Get admin info (Try state first, then request.user from auth)
        admin_id = getattr(request.state, "admin_id", 0)
        admin_username = getattr(request.state, "admin_username", "unknown")
        
        # Fallback to request.user (fastapi-user-auth standard)
        if not admin_id and "user" in request.scope:
            try:
                user = request.user
                admin_id = getattr(user, "id", 0)
                admin_username = getattr(user, "username", "unknown")
            except Exception:
                pass
        
        # 2. Get client IP
        ip_address = request.client.host if request.client else "unknown"
        
        # 3. Extract entity info
        entity_type, entity_id = self._extract_entity_info(request.url.path)
        
        # 4. Get database session
        db: Optional[AsyncSession] = getattr(request.state, "db", None)
        
        # If no session in state, create a short-lived one for auditing
        session_to_use = db
        should_close = False
        
        if session_to_use is None:
            if SessionLocal is None:
                print(f"[AUDIT ERROR] Database not initialized, cannot log: {request.method} {request.url.path}")
                return
            session_to_use = SessionLocal()
            should_close = True
            
        try:
            log_entry = AdminAuditLog(
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
            print(f"[AUDIT ERROR] Failed to log: {e}")
            if session_to_use:
                await session_to_use.rollback()
        finally:
            if should_close and session_to_use:
                await session_to_use.close()
