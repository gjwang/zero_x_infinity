# Standard Operating Procedure (SOP): Token Listing
**Role**: Operations / Listing Manager
**System**: Admin Dashboard

## 1. 准备工作 (Pre-requisites)

Before listing, you need the following information:

| Item | Description | Example | Source |
| :--- | :--- | :--- | :--- |
| **Logic Symbol** | The unique ticker on the exchange | `UNI` | Project Team |
| **Asset Name** | Full display name | `Uniswap` | Project Team |
| **Chain** | The blockchain network | `ETH` | Project Team |
| **Contract Address** | The Token's Smart Contract | `0x1f98...` | Etherscan / Project |
| **Decimals** | Token precision | `18` | Auto-detected |
| **Min Deposit** | Minimum amount to credit | `0.1` | Ops Decision (Risk) |
| **Withdraw Fee** | Fee deducted per withdrawal | `5.0` | Ops Decision (Gas Cost) |

---

## 2. 操作步骤 (Workflow steps)

### Phase 1: Create Logical Asset (业务定义)
*Define the asset for Trading and User Balances.*

1.  **Navigate**: Admin -> `Assets` -> `Create New`.
2.  **Input**:
    *   **Symbol**: `UNI`
    *   **Name**: `Uniswap`
    *   **Decimals**: `18` (System Internal Precision)
    *   **Initial Permissions**:
        *   `[x] Can Allow Deposit`
        *   `[ ] Can Allow Withdraw` (Recommended: Disable initially)
        *   `[ ] Can Allow Trade` (Recommended: Enable later)
    *   **Status**: `Active`
3.  **Click**: `Save`.
    *   *System Result*: `assets_tb` created. Asset ID generated (e.g., `#10`).

### Phase 2: Bind Chain Asset (链上绑定)
*Tell Sentinel how to find this asset on-chain and set limits.*

1.  **Navigate**: Admin -> `Assets` -> Select `UNI` (#10) -> `Chain Config` Tab.
2.  **Click**: `Add New Binding`.
3.  **Input Configuration (Minimal)**:
    *   **Chain**: Select `ETH` (Ethereum).
    *   **Contract Address**: Paste `0x1f98...`
    *   *(Leave other fields empty - System will fetch them)*

4.  **Action**: Click `Auto-Detect from Chain`.
    *   *System Action*: Queries RPC `decimals()`, `symbol()`.
    *   *Result*:
        *   **Decimals**: Auto-filled `18`. (Locked, Read-only)
        *   **Symbol**: Auto-detected `UNI`. (Verifies against Asset name)
    *   *Ops Action*: Verify the fetched data matches. Adjust `Min Deposit` / `Fee` only if defaults are unsuitable.

5.  **Risk Configuration**: (Review Defaults)
    *   **Min Deposit**: `0.1` (Prevent dust attacks).
    *   **Min Withdraw**: `10.0` (Must be > Fee).
    *   **Withdraw Fee**: `5.0` (Cover Gas + Margin).
6.  **Confirm**: Check detected Decimals match project info.
7.  **Click**: `Bind & Activate`.
    *   *System Result*: `chain_assets_tb` created. Sentinel hot-reloads within 60s.

> **Note**: Risk Parameters (Fee, Min Deposit) are **Chain-Specific**. If you list USDT on both ETH and TRON, you must configure them separately for each chain (e.g., ETH Fee = 5.0, TRON Fee = 1.0).

---

## 3. 结果验证 (Verification)

### Verification A: User Deposit (Hot Test)
1.  Ask a test user to deposit `UNI` to their **Existing ETH Address**.
    *   *Note*: User does NOT need to generate a new address.
2.  Wait 1-2 minutes (Block Confirmation).
3.  Check Admin -> `Deposits`: Should see `+ UNI` record.

### Verification B: System Log
1.  Check `Sentinel Logs`: `[ETH] New asset watched: UNI (0x1f98...)`.

---

## 4. 常见问题 (FAQ)

**Q: 用户需要重新生成地址吗？**
A: **不需要**。只要是 ETH 链上的资产，用户统一使用同一个 ETH 充值地址。系统会自动根据 Contract 地址识别是 UNI 还是 USDT。

**Q: 填错了合约地址怎么办？**
A: `Verify On-Chain` 步骤会报错（Decimal获取失败或为0）。如果强行保存了错误地址，请立即在 Admin 中将该 Binding 设为 `Disabled`，然后重新添加正确的。
