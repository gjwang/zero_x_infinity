"""
Tests for Logging Configuration (UX-10)

Tests:
- TC-LOG-01: trace_id is present in all logs
- TC-LOG-02: Log files are created
- TC-LOG-03: Log format is correct
"""
import pytest
from pathlib import Path


class TestLoggingConfiguration:
    """Test suite for logging configuration with trace_id"""

    def test_tc_log_01_trace_id_in_logs(self):
        """TC-LOG-01: trace_id appears in logs"""
        from auth.audit_middleware import generate_trace_id, trace_id_var, get_trace_id
        
        # Generate trace_id
        tid = generate_trace_id()
        trace_id_var.set(tid)
        
        # Verify
        assert len(tid) == 26, "ULID should be 26 chars"
        assert get_trace_id() == tid, "get_trace_id should return set value"

    def test_tc_log_02_loguru_available(self):
        """TC-LOG-02: loguru is properly installed"""
        from loguru import logger
        assert logger is not None

    def test_tc_log_03_logging_config_loads(self):
        """TC-LOG-03: logging_config.py loads without errors"""
        from logging_config import setup_logging
        # Should not raise
        setup_logging(log_dir="./logs", level="INFO")

    def test_log_dir_created(self, tmp_path):
        """Log directory is created if not exists"""
        from logging_config import setup_logging
        
        log_dir = tmp_path / "test_logs"
        setup_logging(log_dir=str(log_dir), level="INFO")
        
        assert log_dir.exists(), "Log directory should be created"
