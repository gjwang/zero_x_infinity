# Architect → Developer Handover: Phase 0x14-c Money Safety

> **Branch**: `0x14-c-money-safety`
> **Design Spec**: [docs/src/0x14-c-money-safety.md](../../src/0x14-c-money-safety.md)
> **Standards**: [docs/standards/money-type-safety.md](../../standards/money-type-safety.md)
> **Date**: 2025-12-31
> **Architect**: Arch-Agent

---

## 1. 战略背景

本次重构是系统从"快速原型"向"生产级金融基础设施"演进的里程碑：

- **100% 账本正确性基础**：资金恒等定理的系统强制验证
- **技术债务清零**：消除 20+ 处 `10u64.pow` 分散调用
- **可持续迭代保障**：CI 审计阻止绕过类型安全

---

## 2. Scope (范围)

### 2.1 需实现的任务

| Phase | Task | Priority |
|-------|------|----------|
| **Phase 1** | `scripts/audit_money_safety.sh` + CI 集成 | P0 |
| **Phase 1.5** | Gateway handlers 使用 `money::parse_qty/price()` | P0 |
| **Phase 2** | 存量代码迁移（见下方扫描结果）| P1 |
| **Phase 2.5** | 意图封装 API 迁移 | P2 |

### 2.2 需迁移的文件（代码扫描结果）

| File | Line(s) | Priority | Action |
|------|---------|----------|--------|
| `src/persistence/queries.rs` | 485, 1153, 1174 | **P0** | 使用 `SymbolInfo::quote_qty()` |
| `src/sentinel/eth.rs` | 585, 613 | **P1** | 使用 `ChainAsset::decimals` |
| `src/models.rs` | 363, 385-413 | **P2** | 移至 test module 或使用常量 |
| `src/csv_io.rs` | 148, 152, 248 | **P3** | 使用 `SymbolManager` |
| `src/websocket/service.rs` | 273-311 | ✅ | 已使用 money 模块 |
| `src/symbol_manager.rs` | 25 | ✅ | 白名单（核心设施）|

---

## 3. Implementation Guide

### 3.1 Phase 1: CI 审计脚本

创建 `scripts/audit_money_safety.sh`：

```bash
#!/bin/bash
set -e

echo "🔍 Auditing money safety..."

ALLOWED_FILES="money.rs|symbol_manager.rs"

VIOLATIONS=$(grep -rn "10u64.pow" --include="*.rs" src/ | grep -v -E "$ALLOWED_FILES" || true)
if [ -n "$VIOLATIONS" ]; then
    echo "❌ FAIL: Found 10u64.pow outside allowed files:"
    echo "$VIOLATIONS"
    exit 1
fi

echo "✅ Money safety audit passed!"
```

集成到 `.github/workflows/ci.yml`：
```yaml
- name: Money Safety Audit
  run: chmod +x scripts/audit_money_safety.sh && ./scripts/audit_money_safety.sh
```

### 3.2 Phase 1.5: Gateway Handler 改造

**文件**: `src/gateway/handlers.rs`

```rust
// Before
let qty: u64 = request.quantity.parse()?;

// After
let qty = money::parse_qty(&request.quantity, symbol_id, &symbol_mgr)
    .map_err(|e| (StatusCode::BAD_REQUEST, format!("{}", e)))?;
```

### 3.3 Phase 2: 存量代码迁移

参考设计文档中 Section 3.3 的详细迁移指南。

---

## 4. Definition of Done

- [ ] `scripts/audit_money_safety.sh` 存在且可执行
- [ ] CI workflow 包含审计步骤
- [ ] 审计脚本在当前代码上通过（所有违规已修复）
- [ ] 所有 370+ 测试通过
- [ ] 无新增 `10u64.pow()` 在白名单外

---

## 5. Acceptance

完成后请：
1. 运行 `./scripts/audit_money_safety.sh` 确认通过
2. 运行 `cargo test` 确认全绿
3. 创建 **Dev → QA Handover** 报告
4. 通知 QA 进行验收测试
