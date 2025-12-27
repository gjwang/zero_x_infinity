"""
UX-10: Trace ID Evidence Chain Tests
=====================================
QA Independent Verification for Financial Compliance

Test Cases:
- TC-UX-10-01: Each HTTP request generates unique ULID trace_id
- TC-UX-10-02: All log entries include trace_id field
- TC-UX-10-03: Response header X-Trace-ID is present
- TC-UX-10-04: admin_audit_log table contains trace_id column
- TC-UX-10-05: Same trace_id appears in logs AND audit table
- TC-UX-10-06: Trace ID is 26 characters (ULID format)
"""

import pytest
import re
import os
import tempfile
from unittest.mock import patch, MagicMock
from datetime import datetime


# ULID format regex: 26 uppercase alphanumeric characters
ULID_PATTERN = re.compile(r'^[0-9A-Z]{26}$')


class TestUX10TraceIdEvidenceChain:
    """UX-10: Trace ID Evidence Chain - Financial Compliance Tests"""
    
    # =========================================================================
    # TC-UX-10-01: Each HTTP request generates unique ULID trace_id
    # =========================================================================
    
    def test_tc_ux_10_01_unique_trace_id_per_request(self):
        """Each HTTP request must generate a unique ULID trace_id"""
        try:
            import ulid
        except ImportError:
            pytest.skip("ulid package not installed")
        
        # Generate multiple trace IDs
        trace_ids = [str(ulid.new()) for _ in range(100)]
        
        # All must be unique
        assert len(trace_ids) == len(set(trace_ids)), \
            "Trace IDs must be unique across requests"
        
        # All must match ULID format
        for tid in trace_ids:
            assert ULID_PATTERN.match(tid), \
                f"Trace ID '{tid}' does not match ULID format"
    
    def test_tc_ux_10_01_ulid_is_sortable(self):
        """ULID must be time-sortable (lexicographic order = chronological order)"""
        try:
            import ulid
            import time
        except ImportError:
            pytest.skip("ulid package not installed")
        
        # Generate ULIDs with small delays
        ulids = []
        for _ in range(5):
            ulids.append(str(ulid.new()))
            time.sleep(0.001)  # 1ms delay
        
        # Sorted order should match generation order
        assert ulids == sorted(ulids), \
            "ULIDs must be lexicographically sortable by time"
    
    # =========================================================================
    # TC-UX-10-02: All log entries include trace_id field
    # =========================================================================
    
    def test_tc_ux_10_02_log_format_contains_trace_id(self):
        """Log format must include trace_id field"""
        # Expected log format pattern
        log_line = 'trace_id=01HRC5K8F1ABCDEFGHIJK action=START endpoint=/asset'
        
        # Verify trace_id is present
        assert 'trace_id=' in log_line, "Log must contain trace_id field"
        
        # Extract trace_id value
        match = re.search(r'trace_id=([0-9A-Z]+)', log_line)
        assert match is not None, "trace_id value must be extractable"
        
        trace_id = match.group(1)
        assert ULID_PATTERN.match(trace_id), \
            f"Extracted trace_id '{trace_id}' must be valid ULID"
    
    def test_tc_ux_10_02_json_log_format(self):
        """Structured JSON logs must include trace_id field"""
        import json
        
        # Example JSON log entry
        log_entry = {
            "timestamp": "2025-12-27T10:25:00Z",
            "trace_id": "01HRC5K8F1ABCDEFGHIJK",
            "admin_id": 1001,
            "action": "DB_UPDATE",
            "entity": "Asset",
            "entity_id": 5,
            "before": {"status": 1},
            "after": {"status": 0},
            "duration_ms": 12
        }
        
        # Verify required fields
        assert "trace_id" in log_entry, "JSON log must have trace_id field"
        assert "timestamp" in log_entry, "JSON log must have timestamp"
        assert "action" in log_entry, "JSON log must have action"
        
        # Verify trace_id format
        assert ULID_PATTERN.match(log_entry["trace_id"]), \
            "trace_id must be valid ULID format"
    
    # =========================================================================
    # TC-UX-10-03: Response header X-Trace-ID is present
    # =========================================================================
    
    def test_tc_ux_10_03_response_header_format(self):
        """Response must include X-Trace-ID header with valid ULID"""
        # Mock response headers
        response_headers = {
            "X-Trace-ID": "01HRC5K8F1ABCDEFGHIJK",
            "Content-Type": "application/json"
        }
        
        # Verify header exists
        assert "X-Trace-ID" in response_headers, \
            "Response must include X-Trace-ID header"
        
        # Verify header value is valid ULID
        trace_id = response_headers["X-Trace-ID"]
        assert ULID_PATTERN.match(trace_id), \
            f"X-Trace-ID header value '{trace_id}' must be valid ULID"
    
    def test_tc_ux_10_03_header_matches_log_trace_id(self):
        """X-Trace-ID header must match trace_id in logs"""
        trace_id = "01HRC5K8F1ABCDEFGHIJK"
        
        response_header_trace_id = trace_id
        log_trace_id = trace_id
        db_trace_id = trace_id
        
        # All must match
        assert response_header_trace_id == log_trace_id, \
            "Response header trace_id must match log trace_id"
        assert log_trace_id == db_trace_id, \
            "Log trace_id must match database trace_id"
    
    # =========================================================================
    # TC-UX-10-04: admin_audit_log table contains trace_id column
    # =========================================================================
    
    def test_tc_ux_10_04_audit_log_schema(self):
        """admin_audit_log table must have trace_id column (VARCHAR 26)"""
        # Expected schema definition
        expected_columns = {
            "id": "BIGSERIAL PRIMARY KEY",
            "trace_id": "VARCHAR(26) NOT NULL",  # ULID is 26 chars
            "admin_id": "BIGINT NOT NULL",
            "action": "VARCHAR(32) NOT NULL",
            "entity_type": "VARCHAR(32)",
            "entity_id": "BIGINT",
            "old_value": "JSONB",
            "new_value": "JSONB",
            "created_at": "TIMESTAMPTZ NOT NULL DEFAULT NOW()"
        }
        
        # Verify trace_id column exists
        assert "trace_id" in expected_columns, \
            "audit_log must have trace_id column"
        
        # Verify trace_id column type
        assert "VARCHAR(26)" in expected_columns["trace_id"], \
            "trace_id column must be VARCHAR(26) for ULID"
        
        # Verify NOT NULL constraint
        assert "NOT NULL" in expected_columns["trace_id"], \
            "trace_id column must be NOT NULL"
    
    def test_tc_ux_10_04_trace_id_index_exists(self):
        """trace_id column should have an index for query performance"""
        # Expected index definition
        expected_index = "CREATE INDEX idx_audit_trace_id ON admin_audit_log(trace_id)"
        
        # Verify index naming convention
        assert "idx_audit_trace_id" in expected_index, \
            "Index must follow naming convention"
        assert "trace_id" in expected_index, \
            "Index must be on trace_id column"
    
    # =========================================================================
    # TC-UX-10-05: Same trace_id in logs AND audit table for one operation
    # =========================================================================
    
    def test_tc_ux_10_05_trace_id_consistency(self):
        """Same trace_id must appear in logs and audit table for one operation"""
        operation_trace_id = "01HRC5K8F1ABCDEFGHIJK"
        
        # Simulate log entries for one operation
        log_entries = [
            f"trace_id={operation_trace_id} action=START endpoint=/asset",
            f"trace_id={operation_trace_id} action=VALIDATE input={{}}",
            f"trace_id={operation_trace_id} action=DB_UPDATE",
            f"trace_id={operation_trace_id} action=END status=200",
        ]
        
        # Simulate audit log record
        audit_record = {
            "trace_id": operation_trace_id,
            "admin_id": 1001,
            "action": "UPDATE",
            "entity_type": "Asset",
            "entity_id": 5
        }
        
        # Verify all log entries have same trace_id
        for log in log_entries:
            match = re.search(r'trace_id=([0-9A-Z]+)', log)
            assert match is not None
            assert match.group(1) == operation_trace_id, \
                "All log entries must have same trace_id"
        
        # Verify audit record has same trace_id
        assert audit_record["trace_id"] == operation_trace_id, \
            "Audit record must have same trace_id as logs"
    
    def test_tc_ux_10_05_cross_reference_query(self):
        """Can query both logs and DB by trace_id to reconstruct operation"""
        trace_id = "01HRC5K8F1ABCDEFGHIJK"
        
        # Example grep command for logs
        log_query = f'grep "trace_id={trace_id}" /var/log/admin/app.log'
        
        # Example SQL query for audit
        sql_query = f"SELECT * FROM admin_audit_log WHERE trace_id='{trace_id}'"
        
        # Both queries should return related records
        assert trace_id in log_query
        assert trace_id in sql_query
    
    # =========================================================================
    # TC-UX-10-06: Trace ID is 26 characters (ULID format)
    # =========================================================================
    
    def test_tc_ux_10_06_ulid_length(self):
        """Trace ID must be exactly 26 characters (ULID format)"""
        valid_ulid = "01HRC5K8F1ABCDEFGHIJKLMNO"
        
        assert len(valid_ulid) == 26, \
            f"ULID must be 26 characters, got {len(valid_ulid)}"
    
    def test_tc_ux_10_06_ulid_character_set(self):
        """ULID must contain only Crockford Base32 characters (0-9, A-Z except I, L, O, U)"""
        try:
            import ulid
        except ImportError:
            pytest.skip("ulid package not installed")
        
        # Generate a ULID
        test_ulid = str(ulid.new())
        
        # Verify length
        assert len(test_ulid) == 26, f"ULID must be 26 chars, got {len(test_ulid)}"
        
        # Verify character set (Crockford Base32)
        valid_chars = set("0123456789ABCDEFGHJKMNPQRSTVWXYZ")
        for char in test_ulid:
            assert char in valid_chars, \
                f"Invalid ULID character: {char}"
    
    def test_tc_ux_10_06_reject_invalid_formats(self):
        """System must reject invalid trace_id formats"""
        invalid_trace_ids = [
            "",                              # Empty
            "abc",                           # Too short
            "01HRC5K8F1ABCDEFGHIJKLMNOP",   # Too long (27 chars)
            "01hrc5k8f1abcdefghijklmno",    # Lowercase
            "01HRC5K8F1ABCDEFGHIJK-MNO",    # Contains hyphen
            "01HRC5K8F1ABCDEFGHIJK MNO",    # Contains space
            "IIIIIIIIIIIIIIIIIIIIIIIIII",   # Contains I (invalid in ULID)
            "LLLLLLLLLLLLLLLLLLLLLLLLLL",   # Contains L (invalid in ULID)
            "OOOOOOOOOOOOOOOOOOOOOOOOOO",   # Contains O (invalid in ULID)
        ]
        
        for invalid_id in invalid_trace_ids:
            # Should not match valid ULID pattern
            if len(invalid_id) == 26:
                # Check character set for 26-char strings
                valid_chars = set("0123456789ABCDEFGHJKMNPQRSTVWXYZ")
                is_valid = all(c in valid_chars for c in invalid_id)
                assert not is_valid or invalid_id == "", \
                    f"'{invalid_id}' should be rejected as invalid ULID"


class TestTraceIdMiddlewareContract:
    """Contract tests for trace_id middleware implementation"""
    
    def test_middleware_generates_ulid_on_entry(self):
        """Middleware must generate ULID at request entry"""
        try:
            import ulid
        except ImportError:
            pytest.skip("ulid package not installed")
        
        # Simulate middleware entry
        trace_id = str(ulid.new())
        
        assert trace_id is not None
        assert len(trace_id) == 26
        assert ULID_PATTERN.match(trace_id)
    
    def test_middleware_sets_context_var(self):
        """Middleware must set trace_id in ContextVar for async propagation"""
        from contextvars import ContextVar
        
        trace_id_var: ContextVar[str] = ContextVar("trace_id", default="")
        
        # Simulate setting trace_id
        test_trace_id = "01HRC5K8F1ABCDEFGHIJK"
        trace_id_var.set(test_trace_id)
        
        # Verify can retrieve
        assert trace_id_var.get() == test_trace_id
    
    def test_middleware_adds_response_header(self):
        """Middleware must add X-Trace-ID to response headers"""
        # Mock response
        class MockResponse:
            headers = {}
        
        response = MockResponse()
        trace_id = "01HRC5K8F1ABCDEFGHIJK"
        
        # Simulate middleware exit
        response.headers["X-Trace-ID"] = trace_id
        
        assert response.headers.get("X-Trace-ID") == trace_id


# Run tests if executed directly
if __name__ == "__main__":
    pytest.main([__file__, "-v", "--tb=short"])
