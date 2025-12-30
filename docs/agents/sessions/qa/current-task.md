# QA Current Task

## Session Info
- **Date**: 2025-12-30
- **Role**: QA Engineer
- **Task**: Phase 0x14-b: Verify Order Commands (IOC, Reduce, Move)
- **Status**: ✅ **Task Complete - Verification Successful**

## Goal
Verify the implementation of new Order Commands in the Spot Matching Engine:
1.  **TimeInForce::IOC**: Immediate-or-Cancel orders.
2.  **ReduceOrder**: Reducing quantity while preserving book priority.
3.  **MoveOrder**: Changing price (resulting in priority loss).

## Progress Checklist
- [x] **IOC Verification**
  - [x] Full Match
  - [x] Partial Fill + Try to Rest -> Expire
  - [x] Body (No Match) -> Expire
- [x] **ReduceOrder Verification**
  - [x] Verify Priority is preserved
  - [x] Verify reduce to zero cancels order
- [x] **MoveOrder Verification**
  - [x] Verify Price change works
  - [x] Verify Priority is reset (treated as new order)
- [x] **One-Click Script**
  - [x] Ran `./scripts/run_0x14b_order_commands.sh` -> **PASSED**

## Artifacts Produced
- [`docs/agents/sessions/qa/0x14-b-verification-report.md`](./0x14-b-verification-report.md)

## Handover Notes
**To Architect/Project Lead**:
- The Matching Engine now fully supports the required complex order types for the functionality phase.
- Ready for integration into the larger Benchmark Harness (0x14-c/d).
