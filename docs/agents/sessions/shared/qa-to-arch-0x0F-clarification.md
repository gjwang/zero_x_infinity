# QA → Architect: 0x0F Design Clarification Request

> **From**: QA Team  
> **To**: Architect  
> **Date**: 2025-12-26  
> **Status**: 🔶 PENDING RESPONSE  
> **Blocking**: Test Plan Finalization

---

## 📋 Summary

QA 完成了 0x0F Admin Dashboard 的多角色审查，发现 **6 个设计缺口** 需要澄清后才能完成测试计划。

---

## ⚠️ Required Clarifications

### GAP-01: Symbol Halt 时 Open Order 处理

**问题**: 当 Admin 将 Symbol 状态设为 `halt` 时，现有的未成交订单如何处理？

**选项**:
1. **Cancel All** - 强制取消所有挂单
2. **Freeze** - 挂单保留但不可修改
3. **Close-Only** - 只允许撤单，不允许新订单

**风险**: 用户资金可能被卡住，导致投诉

**QA Recommendation**: Option 3 (Close-Only) 最安全

**Architect Response**: 
- [ ] Option 1
- [ ] Option 2
- [ ] Option 3
- [ ] Other: _____________

---

### GAP-02: Asset 删除的级联行为

**问题**: 当 Asset 被删除或禁用时，引用该 Asset 的 Symbol 如何处理？

**选项**:
1. **Reject** - 有任何 Symbol 引用则拒绝操作
2. **Cascade** - 级联禁用相关 Symbol
3. **No Delete** - 只允许 Disable，不允许 Delete

**风险**: 可能产生孤儿数据或意外级联

**QA Recommendation**: Option 1 (Reject) 最安全

**Architect Response**: 
- [ ] Option 1
- [ ] Option 2
- [ ] Option 3
- [ ] Other: _____________

---

### GAP-03: Hot Reload SLA

**问题**: 配置变更后，Gateway 多久内必须生效？

**选项**:
1. **5 seconds** - 实时性要求高
2. **30 seconds** - 允许批量更新
3. **Manual Reload** - 需要手动触发

**风险**: 用户体验不一致，Admin 不确定是否生效

**QA Recommendation**: Option 1 (5 seconds) with visual indicator

**Architect Response**: 
- [ ] Option 1: ___ seconds
- [ ] Option 2: ___ seconds  
- [ ] Option 3
- [ ] Other: _____________

---

### GAP-04: Password Policy

**问题**: Admin 账户的密码复杂度要求是什么？

**Required Definition**:
| Property | Value |
|----------|-------|
| Minimum length | ? |
| Require uppercase | Y/N |
| Require number | Y/N |
| Require special char | Y/N |
| Maximum age (days) | ? |
| History (no reuse) | ? previous passwords |

**风险**: 弱密码导致账户被破解

**QA Recommendation**: 12+ chars, uppercase + number + special, 90 days expiry

**Architect Response**: 
| Property | Value |
|----------|-------|
| Minimum length |  |
| Require uppercase |  |
| Require number |  |
| Require special char |  |
| Maximum age (days) |  |
| History (no reuse) |  |

---

### GAP-05: Session Expiry

**问题**: Admin 登录 session 的有效期是多久？

**Required Definition**:
| Property | Value |
|----------|-------|
| Access token expiry | ? |
| Refresh token expiry | ? |
| Idle timeout | ? |
| Force re-auth for sensitive ops | Y/N |

**风险**: 被盗 token 可以无限使用

**QA Recommendation**: Access 15min, Refresh 24h, Idle 30min, Force re-auth for critical ops

**Architect Response**: 
| Property | Value |
|----------|-------|
| Access token expiry |  |
| Refresh token expiry |  |
| Idle timeout |  |
| Force re-auth for sensitive ops |  |

---

### GAP-06: Sub-bps Fee Precision

**问题**: 当输入的 fee_rate 精度超过 1 bps (0.01%) 时如何处理？

**Example**: 用户输入 `0.005%` (0.5 bps)

**选项**:
1. **Reject** - 只接受整数 bps
2. **Round** - 四舍五入到最近 bps
3. **Allow** - 支持小数 bps (需要更高精度存储)

**风险**: 计算误差或精度丢失

**QA Recommendation**: Option 1 (Reject) 保持简单

**Architect Response**: 
- [ ] Option 1
- [ ] Option 2
- [ ] Option 3
- [ ] Other: _____________

---

## ⏰ Response Deadline

为了不阻塞开发进度，请在 **2025-12-27 EOD** 前回复。

---

## 📎 Related Documents

- [QA Test Plan](file:///docs/agents/sessions/qa/0x0F-admin-test-plan.md)
- [Design Doc](file:///docs/src/0x0F-admin-dashboard.md)
- [Arch→QA Handover](file:///docs/agents/sessions/qa/0x0F-admin-handover.md)

---

*QA Team*
