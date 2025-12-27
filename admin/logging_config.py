"""
Logging Configuration for Admin Dashboard using Loguru
UX-10: All logs include trace_id for evidence chain
"""

import sys
from pathlib import Path
from loguru import logger

from auth.audit_middleware import get_trace_id


def trace_id_patcher(record):
    """Add trace_id to all log records"""
    record["extra"]["trace_id"] = get_trace_id() or "-"


def setup_logging(log_dir: str = "./logs", level: str = "INFO"):
    """
    Configure loguru with trace_id support
    
    UX-10 Requirements:
    - TC-UX-10-02: All logs include trace_id
    - Format: timestamp | trace_id | level | message
    
    Log files:
    - admin.log: All logs
    - admin_audit.log: Audit-specific logs
    - admin_error.log: Errors only
    """
    
    # Create log directory
    log_path = Path(log_dir)
    log_path.mkdir(parents=True, exist_ok=True)
    
    # Remove default handler
    logger.remove()
    
    # Add trace_id patcher
    logger.configure(patcher=trace_id_patcher)
    
    # Log format with trace_id
    log_format = (
        "<green>{time:YYYY-MM-DD HH:mm:ss}</green> | "
        "<cyan>{extra[trace_id]}</cyan> | "
        "<level>{level: <8}</level> | "
        "<cyan>{name}</cyan>:<cyan>{function}</cyan>:<cyan>{line}</cyan> - "
        "<level>{message}</level>"
    )
    
    # Console handler (colorized)
    logger.add(
        sys.stderr,
        format=log_format,
        level=level,
        colorize=True,
    )
    
    # Main log file (rotated)
    logger.add(
        log_path / "admin.log",
        format=log_format,
        level=level,
        rotation="10 MB",
        retention="7 days",
        compression="gz",
        encoding="utf-8",
    )
    
    # Audit log file (for audit-specific logs)
    logger.add(
        log_path / "admin_audit.log",
        format=log_format,
        level="INFO",
        rotation="10 MB",
        retention="30 days",  # Keep audit logs longer
        compression="gz",
        encoding="utf-8",
        filter=lambda record: "audit" in record["extra"].get("tags", []),
    )
    
    # Error log file
    logger.add(
        log_path / "admin_error.log",
        format=log_format,
        level="ERROR",
        rotation="10 MB",
        retention="30 days",
        compression="gz",
        encoding="utf-8",
    )
    
    return logger


def audit_log(message: str):
    """Log to audit-specific log file"""
    logger.bind(tags=["audit"]).info(message)
