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
    AUDITED_PATH_PREFIXES = ["/admin/asset", "/admin/symbol", "/admin/vip"]
    
    async def dispatch(self, request: Request, call_next: Callable) -> Response:
        # Only audit modifying operations on specific paths
        if not self._should_audit(request):
            return await call_next(request)
        
        # Capture request body (for old_value/new_value)
        body = None
        if request.method in ("POST", "PUT", "PATCH"):
            try:
                body = await request.json()
            except Exception:
                body = None
        
        # Execute the request
        response = await call_next(request)
        
        # Log after successful operations
        if 200 <= response.status_code < 300:
            await self._create_audit_log(request, body)
        
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
        # Get admin info from request state (set by auth middleware)
        admin_id = getattr(request.state, "admin_id", 0)
        admin_username = getattr(request.state, "admin_username", "unknown")
        
        # Get client IP
        ip_address = request.client.host if request.client else "unknown"
        
        # Extract entity info
        entity_type, entity_id = self._extract_entity_info(request.url.path)
        
        # Get database session (should be available from app state)
        db: Optional[AsyncSession] = getattr(request.state, "db", None)
        
        if db is None:
            # Fallback: log to console if no DB session
            print(f"[AUDIT] {admin_username}@{ip_address} {request.method} {request.url.path}")
            return
        
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
            db.add(log_entry)
            await db.commit()
        except Exception as e:
            print(f"[AUDIT ERROR] Failed to log: {e}")
