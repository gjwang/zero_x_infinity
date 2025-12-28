# AR-001: Architecture Request - WebSocket Authentication

| Status | **REQUESTED** |
| :--- | :--- |
| **Date** | 2025-12-27 |
| **Requester** | QA / Remediation Agent |
| **Driver** | Identity Spoofing Remediation |

## Problem Statement
The current WebSocket implementation relies on a "Strict Anonymous Mode" (ADR-001) which rejects any `user_id != 0`.
While this mitigates immediate identity spoofing, it creates a functional gap: **Authentic users cannot verify their identity or access private channels.**

The user explicitly rejected ADR-001 as a complete solution (`security is not fixed ... require forthar design`), necessitating a robust authentication design.

## Requirements
The Architect must provide a design (e.g., ADR-002) that:
1.  **Authentication mechanism**: Defines how a WebSocket client proves its identity (e.g., JWT in Query Param vs Header vs Handshake Message).
2.  **Integration**: How this integrates with `src/api_auth/` (Ed25519) or standard Session Management.
3.  **State Management**: How `ConnectionManager` stores and validates the authenticated session.
4.  **Migration**: Specific steps to replace the temporary "Strict Anonymous Mode" in `handler.rs` with the new mechanism.

## Constraints
- **Low Latency**: Auth check must not significantly delay connection establishment.
- **Backwards Compatibility**: Must support Anonymous public trade streams simultaneously.
