"""
Tests for UX-10: Trace ID Evidence Chain

Test Cases:
- TC-UX-10-01: Each request generates unique ULID
- TC-UX-10-02: All logs include trace_id
- TC-UX-10-03: Response header X-Trace-ID exists
- TC-UX-10-04: audit_log table has trace_id column
- TC-UX-10-05: Same operation logs and DB have matching trace_id
- TC-UX-10-06: Trace ID is 26 characters (ULID format)
"""
import pytest
from auth.audit_middleware import generate_trace_id, get_trace_id, trace_id_var


class TestUX10TraceID:
    """Test suite for UX-10: Trace ID Evidence Chain"""

    def test_tc_ux_10_01_unique_ulid_per_request(self):
        """TC-UX-10-01: Each request gets unique ULID"""
        ids = [generate_trace_id() for _ in range(100)]
        # All IDs should be unique
        assert len(set(ids)) == 100, "All trace IDs must be unique"

    def test_tc_ux_10_06_ulid_format_26_chars(self):
        """TC-UX-10-06: Trace ID is 26 characters (ULID format)"""
        trace_id = generate_trace_id()
        assert len(trace_id) == 26, f"ULID must be 26 chars, got {len(trace_id)}"
        # ULID uses Crockford's base32: 0-9, A-Z (excluding I, L, O, U)
        valid_chars = set("0123456789ABCDEFGHJKMNPQRSTVWXYZ")
        assert all(c in valid_chars for c in trace_id), f"Invalid ULID chars: {trace_id}"

    def test_tc_ux_10_02_context_var_propagation(self):
        """TC-UX-10-02: Trace ID propagates via ContextVar"""
        # Set trace ID
        test_id = generate_trace_id()
        trace_id_var.set(test_id)
        
        # Get should return same ID
        assert get_trace_id() == test_id

    def test_ulid_monotonically_increasing(self):
        """ULIDs are time-ordered (lexicographically sortable)"""
        ids = [generate_trace_id() for _ in range(10)]
        # Each ID should be >= previous (monotonic)
        for i in range(1, len(ids)):
            assert ids[i] >= ids[i-1], f"ULID should be monotonic: {ids[i-1]} -> {ids[i]}"


class TestAuditLogModel:
    """Test audit log model has trace_id"""
    
    def test_tc_ux_10_04_model_has_trace_id_column(self):
        """TC-UX-10-04: AdminAuditLog model has trace_id field"""
        from models import AdminAuditLog
        from sqlalchemy import inspect
        
        mapper = inspect(AdminAuditLog)
        columns = [col.key for col in mapper.columns]
        assert "trace_id" in columns, "AdminAuditLog must have trace_id column"
    
    def test_trace_id_max_length_26(self):
        """trace_id column should accept 26 char ULID"""
        from models import AdminAuditLog
        from sqlalchemy import inspect
        
        mapper = inspect(AdminAuditLog)
        trace_col = mapper.columns["trace_id"]
        # String(26) allows up to 26 chars
        assert trace_col.type.length >= 26, f"trace_id must allow 26 chars, got {trace_col.type.length}"
