# E2E Tests Package
"""
End-to-End Integration Tests for Admin Dashboard

These tests verify the complete chain:
Admin UI → Database → Gateway → Matching Engine

Prerequisites:
- PostgreSQL running on :5433
- Admin Dashboard running on :8001
- Gateway running on :8000

Run with:
    pytest admin/tests/e2e/ -v
    
Or with environment variables:
    ADMIN_URL=http://localhost:8001 \
    GATEWAY_URL=http://localhost:8000 \
    pytest admin/tests/e2e/ -v
"""
