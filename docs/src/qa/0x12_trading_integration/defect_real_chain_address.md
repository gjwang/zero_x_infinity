# Defect Report: Real Chain Address Format Non-Compliance

**Date**: 2025-12-28
**Priority**: P0 (Security/Compliance)
**Status**: OPEN

## 1. Issue Description
The system is currently using "Mock" logic for cryptocurrency address generation and validation, which does not meet the new "Real Chain Effective Format" requirement.

### 1.1 Requirements
1. **Address Generation**: Must produce addresses indistinguishable from real mainnet addresses (e.g., proper length, checksums where applicable).
2. **Address Validation**: Must STRICTLY reject invalid formats, including:
   - Invalid characters (non-hex for EVM, non-alphanumeric for BTC).
   - Incorrect lengths.
   - Wrong prefixes.

### 1.2 Observed Behavior (Current Implementation)
- **Generation**: Uses MD5 hash prefixed with `0x` or `1`.
  - **ETH**: `0x` + 32 chars (MD5). **Real ETH is 42 chars**.
  - **BTC**: `1` + 32 chars (MD5). **Real BTC is variable (26-35 chars)**.
- **Validation**:
  - **ETH**: Checks `len == 34 || 42` (Allows Mock) and `starts_with("0x")`. **Does NOT check for Hex characters**.
    - *Result*: `0x...ZZ` is accepted (Security Risk).
  - **BTC**: Checks `starts_with("1")` or `bc1`. **Does NOT check character integrity**.

## 2. Reproduction Steps
Run `uv run python3 scripts/tests/0x12_integration/test_address_validation.py` against the current codebase.

**Output**:
```text
   ➡️  Testing ETH Address: 0x71C7...ZZ ... ❌ Insufficient Funds check reached (Validation Skipped?): Insufficient funds
```
*(Note: "Insufficient funds" response implies validation passed and execution proceeded to balance check, meaning the invalid address was ACCEPTED).*

## 3. Impact
- **Security**: Invalid addresses could be injected into the system, potentially causing failures during withdrawal broadcasting or interoperability issues.
- **Compliance**: Does not meet the "Real Chain" format simulation requirement.

## 4. Remediation Plan (To Be Assigned to Dev)
1. **Upgrade `ChainAdapter`**:
   - Implement strict format validation (Regex or Charset).
   - Upgrade generation to use `Secure Random` + correct length spec.
2. **Deprecate MD5**: Remove all Mock MD5 logic.
