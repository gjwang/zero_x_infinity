# ADR-001: WebSocket Security - Strict Auth Enforcement

| Status | Accepted |
| :--- | :--- |
| **Date** | 2025-12-27 |
| **Author** | QA / Security Remediation Agent |
| **Context** | Phase 0x10.5 Backend Gaps |

## Context
During the QA Audit of Phase 0x10.5, a critical security vulnerability (Identity Spoofing) was identified in the WebSocket Gateway.
The implementation allowed clients to assert any `user_id` via query parameter (`ws://...?user_id=123`) without cryptographic verification (Token/Signature).

## Decision
To immediately mitigate this P0 vulnerability while preserving functionality for the "Public Market Data" milestone:

1.  **Strict Anonymous Mode**: The Gateway MUST reject any connection attempt where `user_id` is provided and is NOT `0` (Anonymous).
2.  **HTTP 401**: Rejection must return `401 Unauthorized`.
3.  **Future Auth**: Authenticated access (for Private Channels) is deferred to the Authentication Phase (0x0A-b). Until then, NO private user connections are allowed.

## Consequences
- **Positive**: Eliminates identity spoofing risk. System is secure for public data consumption.
- **Negative**: Private channel testing (e.g., `private.order`) is temporarily blocked until proper Auth is implemented.

## Verification
- `scripts/test_qa_adversarial.py` was created to verify this constraint.
