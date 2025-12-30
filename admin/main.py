"""
Admin Dashboard Main Entry Point
Phase 0x0F - Zero X Infinity

FastAPI Best Practices:
- Single PostgreSQL database
- Dependency injection for DB sessions
- Lifespan events for startup/shutdown
- Proper middleware ordering
- UX-10: Trace ID logging with loguru
- SEC-05/08: Sanitized error responses (no stack traces, no framework info)

Run with:
    uvicorn main:app --host 0.0.0.0 --port 8002
"""
from contextlib import asynccontextmanager
from loguru import logger
import re

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse
from fastapi.exceptions import RequestValidationError
from pydantic import ValidationError
from fastapi_amis_admin.admin.settings import Settings as AdminSettings
from fastapi_amis_admin.admin.site import AdminSite

from settings import settings
from database import init_db, close_db
from admin import AssetAdmin, SymbolAdmin, VIPLevelAdmin, AuditLogAdmin, ChainAdmin, ChainAssetAdmin
from auth import AuditLogMiddleware


def sanitize_error_message(msg: str) -> str:
    """Remove framework internals from error messages for security (SEC-05/08)"""
    # Remove framework names
    patterns = [
        r'\bpydantic\b', r'\bfastapi\b', r'\bstarlette\b', r'\bsqlalchemy\b',
        r'\basync def\b', r'\bawait\b', r'\bcoroutine\b',
        r'File "[^"]*"', r'line \d+',  # Stack trace patterns
        r'traceback', r'raise\s+\w+',
    ]
    result = msg
    for pattern in patterns:
        result = re.sub(pattern, '', result, flags=re.IGNORECASE)
    return result.strip()
from logging_config import setup_logging

# Initialize logging with trace_id support
setup_logging(log_dir="./logs", level="INFO")


@asynccontextmanager
async def lifespan(app: FastAPI):
    """
    Lifespan context manager
    Replaces deprecated @app.on_event("startup")
    """
    # Startup
    await init_db(settings.database_url)
    logger.info(f"Started at http://{settings.admin_host}:{settings.admin_port}/admin")
    logger.info(f"Database: PostgreSQL")
    logger.info(f"Logs: ./logs/admin.log, ./logs/admin_audit.log")
    yield
    # Shutdown
    await close_db()
    logger.info("Shutdown complete")



# Create FastAPI app with lifespan
app = FastAPI(
    title="Zero X Infinity Admin",
    version="0.0F",
    lifespan=lifespan,
    docs_url="/docs",      # Swagger UI at /docs
    redoc_url="/redoc",    # ReDoc at /redoc
    openapi_url="/openapi.json",
)


# SEC-05/08: Custom exception handler to sanitize error responses
@app.exception_handler(RequestValidationError)
async def validation_exception_handler(request: Request, exc: RequestValidationError):
    """Sanitize validation errors to hide framework details"""
    # Extract only safe fields from error details
    safe_errors = []
    for error in exc.errors():
        safe_error = {
            "loc": error.get("loc", []),
            "msg": sanitize_error_message(str(error.get("msg", "Invalid input"))),
            "type": "validation_error"  # Generic type, don't expose 'string_too_short' etc.
        }
        safe_errors.append(safe_error)
    
    return JSONResponse(
        status_code=422,
        content={"status": 422, "msg": "Validation Error", "detail": safe_errors}
    )


class SecuritySanitizationMiddleware:
    """
    Middleware to sanitize all responses for security (SEC-05/08)
    Removes framework information from error responses regardless of which handler produced them.
    """
    def __init__(self, app):
        self.app = app
    
    async def __call__(self, scope, receive, send):
        if scope["type"] != "http":
            await self.app(scope, receive, send)
            return
        
        # Capture response body
        response_body = []
        send_wrapper = self._make_send_wrapper(send, response_body)
        await self.app(scope, receive, send_wrapper)
    
    def _make_send_wrapper(self, send, response_body):
        import json
        
        async def send_wrapper(message):
            if message["type"] == "http.response.start":
                # Remove Content-Length header as sanitization may change body size
                headers = message.get("headers", [])
                message["headers"] = [
                    (k, v) for k, v in headers 
                    if k.lower() != b"content-length"
                ]
            elif message["type"] == "http.response.body":
                body = message.get("body", b"")
                if body:
                    try:
                        # Try to parse and sanitize JSON response
                        data = json.loads(body.decode())
                        sanitized = self._sanitize_response(data)
                        message = {**message, "body": json.dumps(sanitized).encode()}
                    except (json.JSONDecodeError, UnicodeDecodeError):
                        pass  # Not JSON, pass through
            await send(message)
        return send_wrapper
    
    def _sanitize_response(self, data):
        """Remove pydantic.dev URLs and framework info from response"""
        if isinstance(data, dict):
            # Remove pydantic URL field from errors
            if "errors" in data and isinstance(data["errors"], list):
                for error in data["errors"]:
                    if isinstance(error, dict):
                        error.pop("url", None)  # Remove pydantic.dev URL
                        if "msg" in error:
                            error["msg"] = sanitize_error_message(str(error["msg"]))
            if "detail" in data and isinstance(data["detail"], list):
                for error in data["detail"]:
                    if isinstance(error, dict):
                        error.pop("url", None)
                        if "msg" in error:
                            error["msg"] = sanitize_error_message(str(error["msg"]))
            # Recursively process nested dicts
            return {k: self._sanitize_response(v) for k, v in data.items()}
        elif isinstance(data, list):
            return [self._sanitize_response(item) for item in data]
        return data


# Add security middleware FIRST (outermost)
app.add_middleware(SecuritySanitizationMiddleware)

# Add audit middleware
app.add_middleware(AuditLogMiddleware)

# Create admin site with PostgreSQL
site = AdminSite(
    settings=AdminSettings(
        database_url_async=settings.database_url,
        secret_key=settings.admin_secret_key,
        site_title=settings.site_title,
        site_icon=settings.site_icon,
    ),
)

# Register admin pages
site.register_admin(
    AssetAdmin, SymbolAdmin, VIPLevelAdmin, AuditLogAdmin,
    ChainAdmin, ChainAssetAdmin  # ADR-005: Chain Config support
)

# CRITICAL: Add database middleware BEFORE mounting to properly wrap admin routes
# Without this, UPDATE operations won't persist to database!
app.add_middleware(site.db.asgi_middleware)

# Mount to app (AFTER adding db middleware)
site.mount_app(app)

# SEC-05/08: OVERRIDE fastapi-amis-admin exception handlers AFTER mount
# This is CRITICAL - mount_app registers its own handlers that leak framework info

async def secure_validation_handler(request: Request, exc: RequestValidationError):
    """Sanitized validation error handler (SEC-05/08)"""
    safe_errors = []
    for error in exc.errors():
        safe_errors.append({
            "loc": error.get("loc", []),
            "msg": sanitize_error_message(str(error.get("msg", "Invalid input"))),
            "type": "validation_error"
        })
    return JSONResponse(
        status_code=200,  # Amis expects HTTP 200 with status in body
        content={"status": 422, "msg": "Validation Error", "detail": safe_errors}
    )

async def secure_pydantic_handler(request: Request, exc: ValidationError):
    """Sanitized Pydantic error handler (SEC-05/08)"""
    safe_errors = []
    for error in exc.errors():
        safe_errors.append({
            "loc": list(error.get("loc", [])),
            "msg": sanitize_error_message(str(error.get("msg", "Invalid input"))),
            "type": "validation_error"
        })
    return JSONResponse(
        status_code=200,  # Amis expects HTTP 200 with status in body
        content={"status": 422, "msg": "Validation Error", "detail": safe_errors}
    )

# Register on main app
app.add_exception_handler(RequestValidationError, secure_validation_handler)
app.add_exception_handler(ValidationError, secure_pydantic_handler)


@app.get("/health")
def health_check():
    """
    Health check endpoint
    Simple check without DB dependency for test compatibility
    """
    return {
        "status": "ok",
        "service": "admin-dashboard"
    }


@app.get("/api/chain/detect/{chain_slug}/{contract_address}")
async def detect_token_from_chain(chain_slug: str, contract_address: str):
    """
    Auto-Detect token decimals and symbol from blockchain RPC
    SOP Phase 2: Chain Config - Verify on-chain before binding
    
    Note: This is a stub implementation. Production would call:
    - chains_tb.rpc_urls to get endpoint
    - EVM RPC eth_call for decimals() and symbol()
    """
    # Known tokens for development (hardcoded for now)
    known_tokens = {
        "0xdac17f958d2ee523a2206206994597c13d831ec7": {"symbol": "USDT", "decimals": 6},
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48": {"symbol": "USDC", "decimals": 6},
        "0x1f9840a85d5af5bf1d1762f925bdaddc4201f984": {"symbol": "UNI", "decimals": 18},
    }
    
    addr_lower = contract_address.lower()
    if addr_lower in known_tokens:
        return {
            "status": "ok",
            "chain": chain_slug,
            "contract": contract_address,
            "detected": known_tokens[addr_lower],
        }
    
    # For unknown tokens, return error
    return {
        "status": "error",
        "msg": f"Unable to detect token at {contract_address}. RPC integration pending.",
        "chain": chain_slug,
        "contract": contract_address,
    }


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(
        "main:app",
        host=settings.admin_host,
        port=settings.admin_port,
        reload=True,
    )
