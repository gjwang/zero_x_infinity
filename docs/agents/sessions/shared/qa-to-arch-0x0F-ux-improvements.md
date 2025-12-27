# QA → Architect: 0x0F Admin Dashboard UX 改进提案

> **From**: QA Team (Agent Leader)  
> **To**: Architect  
> **Date**: 2025-12-26  
> **Type**: 设计文档更新建议

---

## 📋 背景

在完成 160+ 测试用例的过程中，QA 团队从用户视角发现了若干可改进的 UX 设计点。
这些建议旨在：**减少人为错误、提高可读性、加速操作效率**。

---

## 🎯 目标用户

| 角色 | 主要操作 | 关键需求 |
|------|----------|----------|
| 运营 | 创建 Asset/Symbol、调整费率 | 效率、批量操作 |
| 风控 | Halt Symbol、冻结资产 | 速度、不能出错 |
| 客服 | 查询配置 | 只读、易搜索 |
| 审计 | 查看审计日志 | 筛选、导出 |

---

## 📐 建议的设计更新

### UX-01: Symbol 创建时显示 Asset 名称

**当前设计**：
```
base_asset_id: [1]  ← 用户需要记忆 ID 对应的 Asset
quote_asset_id: [2]
```

**建议设计**：
```
Base Asset: [BTC ▼]  ← 下拉框显示 Asset Code
Quote Asset: [USDT ▼]

--- 或 ---

base_asset_id: [1] → BTC  ← 自动显示对应名称
quote_asset_id: [2] → USDT
```

**验收标准**：
- [ ] 创建/编辑 Symbol 时，asset_id 旁显示 Asset Code
- [ ] 下拉框选项格式："BTC (ID: 1)"

---

### UX-02: Fee 显示百分比格式

**当前设计**：
```
base_maker_fee: 10
base_taker_fee: 20
```

**建议设计**：
```
Maker Fee: 0.10% (10 bps)
Taker Fee: 0.20% (20 bps)
```

**验收标准**：
- [ ] 列表页和编辑页都显示百分比
- [ ] 输入时可用 BPS 或百分比

---

### UX-03: 危险操作二次确认

**涉及操作**：
- Symbol Halt
- Asset Disable
- VIP Level 删除

**建议设计**：

```
┌─────────────────────────────────────────┐
│  ⚠️ 确认 Halt Symbol: BTC_USDT          │
├─────────────────────────────────────────┤
│  影响预览:                               │
│  • 当前活跃订单: 1,234 个                │
│  • 24h 成交量: $12,345,678              │
│                                          │
│  此操作将阻止所有新订单                   │
│                                          │
│  请输入 Symbol 名称确认: [BTC_USDT    ]  │
│                                          │
│        [Halt]    [Cancel]                │
└─────────────────────────────────────────┘
```

**验收标准**：
- [ ] Halt/Disable 需要二次确认
- [ ] 显示影响范围（订单数、交易量）
- [ ] 输入名称确认（防误点）

---

### UX-04: 不可变字段 UI 标识

**涉及字段**：
- Asset: `asset`, `decimals`
- Symbol: `symbol`, `base_asset_id`, `quote_asset_id`, `price_decimals`, `qty_decimals`

**建议设计**：

```
Asset 编辑页:
┌─────────────────────────────┐
│ Asset Code: BTC 🔒          │  ← 灰色背景，禁用
│ Decimals: 8 🔒              │  ← 灰色背景，禁用
│ Name: [Bitcoin         ] ✏️  │  ← 可编辑
│ Status: [Active ▼] ✏️        │  ← 可编辑
└─────────────────────────────┘
```

**验收标准**：
- [ ] 不可变字段显示锁定图标
- [ ] 不可变字段禁用输入
- [ ] Tooltip 说明"创建后不可修改"

---

### UX-05: 错误消息可操作性

**当前**：
```json
{"detail": "validation error"}
```

**建议**：
```json
{
  "field": "asset",
  "error": "格式错误",
  "got": "btc!",
  "expected": "仅大写字母 A-Z, 例如: BTC",
  "hint": "请移除特殊字符 '!'"
}
```

**验收标准**：
- [ ] 错误消息包含字段名
- [ ] 错误消息包含建议格式
- [ ] 前端显示友好提示

---

### UX-06: Symbol base ≠ quote 校验

**问题**：当前允许创建 `BTC_BTC`

**建议**：
- 后端校验 `base_asset_id != quote_asset_id`
- 前端提示 "Base 和 Quote 不能相同"

**验收标准**：
- [ ] 选择相同 Asset 时 UI 禁用确认按钮
- [ ] API 返回明确错误 "BASE_QUOTE_SAME"

---

### UX-07: Symbol 名称与 Asset 一致性检查

**问题**：可以创建 `BTC_USDT` 但 base_asset_id 指向 ETH

**建议**：
- 警告："Symbol 名称 'BTC_USDT' 与选择的 Asset 'ETH' 不匹配"
- 或：自动从 Symbol 名称推导 Asset ID

**验收标准**：
- [ ] 名称与 Asset 不匹配时显示警告
- [ ] 用户确认后可继续（防止合法情况被阻断）

---

## 📊 优先级建议

| 改进 | 优先级 | 理由 |
|------|--------|------|
| UX-03 危险操作确认 | **P0** | 防止误操作导致事故 |
| UX-06 base≠quote | **P0** | 逻辑错误 |
| UX-04 不可变字段 | **P1** | 用户体验 |
| UX-01 显示 Asset 名称 | **P1** | 减少查询 |
| UX-02 Fee 百分比 | **P1** | 可读性 |
| UX-05 错误消息 | **P2** | 调试效率 |
| UX-07 名称一致性 | **P2** | 防止配置错误 |

---

## 📎 相关测试用例

`admin/tests/test_ux_improvements.py` 已包含上述改进的验证测试：
- TC-UX-01 ~ TC-UX-07

---

*QA Team (Agent Leader)*  
*2025-12-26*
