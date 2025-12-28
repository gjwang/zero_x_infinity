# QA Feedback: 0x11-a Real Chain Integration

**To**: Developer Team
**From**: QA Team (Agents A, B, C)
**Date**: 2025-12-28
**Status**: 🔴 **REJECTED (Fix Incomplete)**

## 🚨 Critical Blocker (P0) - RE-OPENED

### DEF-001: Gateway Address Generation Incompatible with Regtest
**Component**: Gateway (`src/funding/chain_adapter.rs`)
**Status**: **REJECTED** after independent verification.

**Findings (Post-Fix Verification)**:
The "fix" implemented in `MockBtcChain` merely prepends `bcrt1` to a random string.
*   **Result**: Generated addresses (e.g., `bcrt1...`) have **invalid checksums**.
*   **Evidence**: Real `bitcoind` node rejects transactions to these addresses with error: `Invalid Bitcoin address`.
*   **Conclusion**: Cosmetic fixes are insufficient. The system must generate cryptographically valid Bech32 addresses (SegWit) or legacy addresses acceptable by the Regtest node.

**Required Fix (Re-iterated)**:
Use a proper library (e.g., `bitcoin`, `bitcoincore-rpc`) to generate valid addresses, OR integrate with the `bitcoind` RPC `getnewaddress` method in `dev` mode.

---

## ✅ Verified Items

*   **Sentinel Service**: ✅ **PASS**. Continuous loop (DEF-002) is fixed and verified.
*   **Security**: ✅ **PASS**.

## ⏭️ Next Steps
1.  **Developer**: Please implement legitimate address generation in Phase 0x11-b.
2.  **QA**: Will blocking-wait for valid address generation to verify `TC-B01` (Deposit Lifecycle).

---
*See `docs/src/qa/0x11a_real_chain/test_report.md` for full test log.*
