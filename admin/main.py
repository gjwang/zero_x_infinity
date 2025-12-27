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
from admin import AssetAdmin, SymbolAdmin, VIPLevelAdmin, AuditLogAdmin
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


# Add middleware BEFORE mounting (correct order)
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
site.register_admin(AssetAdmin, SymbolAdmin, VIPLevelAdmin, AuditLogAdmin)

# CRITICAL: Add database middleware BEFORE mounting to properly wrap admin routes
# Without this, UPDATE operations won't persist to database!
app.add_middleware(site.db.asgi_middleware)

# Mount to app (AFTER adding db middleware)
site.mount_app(app)


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


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(
        "main:app",
        host=settings.admin_host,
        port=settings.admin_port,
        reload=True,
    )
