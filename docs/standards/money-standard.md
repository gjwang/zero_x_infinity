# Money & Currency Standard

## 🎯 Goal
Ensure 100% accuracy and consistency in financial calculations by enforcing a unified, audited conversion interface and forbidding manual scaling.

## 🛡️ Core Mandates

### 1. No Manual Scaling
It is **strictly forbidden** to perform manual arithmetic scaling on currency amounts anywhere except within `src/money.rs`.
- ❌ `let internal = amount * 100_000_000;`
- ✅ `let internal = symbol_mgr.parse_qty(amount_str, symbol_id)?;`

### 2. Mandatory Domain Types
All internal representations of money must use the `ScaledAmount` (for unsigned) or `ScaledAmountSigned` (for signed) domain types.
- ❌ `fn update_balance(user_id: u32, amount: u64)`
- ✅ `fn update_balance(user_id: u32, amount: ScaledAmount)`

### 3. Financial Truncation Policy
Calculations must never introduce "phantom funds" via rounding. Standard formatting for display must **truncate**.
- ✅ `1.999 BTC` displayed with 2 decimals is `"1.99"`.

## 🛠️ Permitted Workflows

### API Boundary (Input)
Convert human-readable strings or Decimals to `ScaledAmount` immediately using `SymbolManager`.

```rust
let amount: ScaledAmount = symbol_mgr.parse_qty(req.quantity, symbol_id)?;
```

### Business Logic (Arithmetic)
Perform calculations using the safe methods provided by `ScaledAmount`.

```rust
let total = balance.checked_add(deposit)?;
```

### Presentation/Persistence (Output)
Use `SymbolManager` or `MoneyFormatter` to convert `ScaledAmount` back to strings.

```rust
let display = symbol_mgr.format_qty(balance, symbol_id);
```

## 🕵️ Enforcement (CI Audit)
Any PR containing manual scaling patterns (e.g., `pow(decimals)`) outside of `src/money.rs` will be automatically rejected by the `audit_money_safety.sh` script.
