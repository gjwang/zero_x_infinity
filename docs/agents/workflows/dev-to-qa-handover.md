# Developer → QA Handover: Best Practices

> **Purpose**: 标准化Developer完成任务后向QA传递状态的流程  
> **Audience**: Developer Agent, QA Engineer Agent, Project Leads

---

## 🎯 核心原则

**QA不能仅凭Developer说"完成了"就接受**。需要：
1. ✅ 明确的交付物清单
2. ✅ 可执行的验证步骤
3. ✅ 明确的验收标准
4. ✅ Git commit证据

---

## 📋 标准交接流程

### Step 1: Developer创建交付文档

**文件位置**: `docs/agents/sessions/shared/dev-to-qa-handover.md`

**必需内容**:
```markdown
# Developer → QA: [Feature Name] Handover

## 📦 交付物清单

- [x] 功能实现 (commit: abc1234)
- [x] 单元测试 (XX个测试通过)
- [x] 代码审查完成
- [ ] 文档更新

## 🧪 验证步骤

### 1. 验证修复的Bug
\`\`\`bash
# 重新运行失败的测试
./scripts/test_transfer_e2e.sh

# 预期结果: 10/10 PASS (之前是8/10)
# 重点关注: TC-P0-07 (Idempotency) 必须通过
\`\`\`

### 2. 回归测试
\`\`\`bash
cargo test --release
# 预期: 271/271 通过
\`\`\`

### 3. E2E场景
\`\`\`bash
# 具体场景描述
# 预期行为
\`\`\`

## ✅ 验收标准

- [ ] TC-P0-07 (Idempotency) 测试通过
- [ ] 相同cid返回相同transfer_id
- [ ] Balance不会重复扣除
- [ ] 无新增失败测试
- [ ] 代码通过clippy检查

## 📝 实施细节

**修复内容**:
- 在`fsm_transfers_tb`添加UNIQUE约束(user_id, cid)
- 在transfer创建前检查cid是否存在
- 如存在则返回existing_transfer

**Git Commits**:
- abc1234: "Fix: Add idempotency check for transfers"
- def5678: "Test: Add missing idempotency test validation"

## 🔗 相关文档

- QA报告: `docs/agents/sessions/qa/0x0B-transfer-p0-test-report.md`
- Bug描述: `docs/agents/sessions/shared/qa-blockers.md` (P0 section)
- 设计文档: `docs/src/0x0B-a-transfer.md` (Section 1.5.7)

## ⚠️ 已知限制/遗留问题

- None (或明确列出)

## 📞 Ready for QA

Developer签名: @Developer AI Agent  
Date: 2025-12-26 01:35  
Status: ✅ Ready for QA verification
```

---

### Step 2: QA接收并验证

**QA工作流**:

#### 2.1 检查交付文档
```bash
# QA首先查看
cat docs/agents/sessions/shared/dev-to-qa-handover.md

# 检查必需字段
✅ 交付物清单
✅ 验证步骤
✅ 验收标准
✅ Git commits
```

#### 2.2 执行验证步骤
```bash
# 按Developer提供的步骤执行
./scripts/test_transfer_e2e.sh

# 如果结果与预期不符 → 拒绝验收
# 如果结果符合预期 → 继续深度测试
```

#### 2.3 创建验证报告
**文件位置**: `docs/agents/sessions/qa/verification-[feature]-[date].md`

```markdown
# QA Verification: [Feature Name]

## 📋 Developer声明验证

- [x] 交付文档完整
- [x] 验证步骤可执行
- [x] Git commits存在

## 🧪 测试执行结果

### Developer提供的测试
- TC-P0-07: ✅ PASS (之前FAIL)
- E2E全量: ✅ 10/10 PASS (之前8/10)

### QA额外测试
- Edge case 1: ✅ PASS
- Edge case 2: ✅ PASS

## ✅ 验收决定

Status: ✅ **APPROVED** / ❌ **REJECTED**

Reason: (如rejected, 具体说明)

QA签名: @QA Engineer AI  
Date: 2025-12-26
```

---

## 📁 文件组织结构

```
docs/agents/sessions/
├── shared/
│   ├── dev-to-qa-handover.md        # Developer → QA交接文档
│   ├── qa-to-dev-feedback.md        # QA → Developer反馈
│   └── qa-blockers.md               # QA发现的blockers
│
├── developer/
│   ├── current-task.md              # Developer当前任务
│   └── 0x0B-transfer-impl-log.md    # 实施日志
│
└── qa/
    ├── current-task.md              # QA当前任务
    ├── verification-transfer-1226.md # 验证报告
    └── 0x0B-transfer-p0-test-report.md # 原始测试报告
```

---

## 🔄 完整交接流程图

```
Developer完成实现
        ↓
创建 dev-to-qa-handover.md
        ↓
Git commit + push
        ↓
通知QA (更新shared文档)
        ↓
═══════════════════════════════════
        ↓
QA读取 dev-to-qa-handover.md
        ↓
执行验证步骤
        ↓
创建 verification-*.md
        ↓
    ┌────────┐
    │ PASS?  │
    └────┬───┘
         │
    ┌────┴────┐
    │         │
   YES       NO
    │         │
    ↓         ↓
 APPROVED  REJECTED
    │         │
    ↓         ↓
关闭blocker  创建qa-to-dev-feedback.md
    │         │
    ↓         └──→ Developer修复 → 重新handover
下个任务
```

---

## 🚨 常见错误及避免方法

### ❌ 错误1: 只说"完成了"
**问题**: 没有具体验证步骤

**正确做法**:
```markdown
❌ "Transfer bug已修复"
✅ "Transfer idempotency已修复，请运行 ./scripts/test_transfer_e2e.sh
   验证TC-P0-07通过，预期结果10/10"
```

### ❌ 错误2: 没有Git证据
**问题**: 无法追溯哪个commit修复了什么

**正确做法**:
```markdown
✅ Git Commits:
   - 7a8b9c0: "Fix: Add UNIQUE constraint on fsm_transfers_tb.cid"
   - 1d2e3f4: "Test: Verify idempotency in TC-P0-07"
```

### ❌ 错误3: 验收标准模糊
**问题**: "所有测试通过" vs "TC-P0-07通过"

**正确做法**:
```markdown
✅ 验收标准:
   - [ ] TC-P0-07测试从FAIL变为PASS
   - [ ] 相同cid产生相同transfer_id
   - [ ] Balance deduction只发生一次
   - [ ] 无回归 (其他8个测试仍然PASS)
```

---

## 📊 Checklist Template

### Developer交付前自检
- [ ] 创建了`dev-to-qa-handover.md`
- [ ] 列出了所有交付物和Git commits
- [ ] 提供了可执行的验证步骤
- [ ] 定义了明确的验收标准
- [ ] 本地执行过所有验证步骤
- [ ] 所有验证步骤都通过了
- [ ] 更新了`current-task.md`状态

### QA验收前检查
- [ ] 阅读了`dev-to-qa-handover.md`
- [ ] 验证了交付物清单完整性
- [ ] 执行了所有验证步骤
- [ ] 验证结果符合验收标准
- [ ] 进行了额外的边缘测试
- [ ] 创建了验证报告
- [ ] 更新了blockers状态

---

## 💡 最佳实践示例

### 优秀的Handover文档示例

```markdown
# Developer → QA: Transfer Idempotency Fix

## 📦 交付物

- [x] 数据库迁移: `migrations/20241226_add_transfer_cid_unique.sql`
- [x] 业务逻辑: `src/internal_transfer/service.rs:243-267`
- [x] 单元测试: `src/internal_transfer/service.rs::test_idempotency`
- [x] E2E测试更新: `scripts/test_transfer_e2e.sh:358-434`

Git commits:
- 7a8b9c0: "Fix: Add idempotency check for internal transfers"
- 1d2e3f4: "Migration: Add UNIQUE constraint on (user_id, cid)"
- 2e3f4g5: "Test: Add unit test for idempotency"

## 🧪 验证步骤

### 前置条件
\`\`\`bash
# 确保数据库已迁移
psql -h localhost -p 5433 -U zero_x_infinity -d zero_x_infinity_db \
  -c "SELECT * FROM fsm_transfers_tb LIMIT 1;"
# 应该看到cid列存在
\`\`\`

### 主要验证
\`\`\`bash
./scripts/test_transfer_e2e.sh
\`\`\`

**关键输出**:
\`\`\`
[TC-P0-07] Idempotency (Duplicate CID)...
    ✓ PASS: Same transfer_id returned (01KDAZEZCAP9...)
    ✓ PASS: Balance unchanged on duplicate (stayed at 955.00)
\`\`\`

### 回归验证
\`\`\`bash
cargo test internal_transfer::service::test --release
# 预期: All tests in module pass
\`\`\`

## ✅ 验收标准

必须满足:
1. [ ] TC-P0-07显示"✓ PASS"（之前是"✗ FAIL"）
2. [ ] Same transfer_id返回（不是不同的ID）
3. [ ] Balance只变化一次（不是两次）
4. [ ] 其他9个P0测试仍然通过
5. [ ] 单元测试全部通过

## 📝 技术实施

**方案**: Database-level uniqueness + Application-level check

**关键代码**:
\`\`\`rust
// src/internal_transfer/service.rs:245
if let Some(cid) = request.cid {
    // Check existing transfer by cid
    if let Some(existing) = self.find_by_cid(user_id, &cid).await? {
        return Ok(existing); // Idempotent return
    }
}
\`\`\`

**数据库约束**:
\`\`\`sql
ALTER TABLE fsm_transfers_tb 
  ADD CONSTRAINT unique_user_cid UNIQUE (user_id, cid);
\`\`\`

## ⚠️ Breaking Changes

None. `cid`字段已存在且为optional, 新增约束不影响现有数据。

## 📞 Ready for QA

Developer: @Developer AI  
Date: 2025-12-26 01:35  
Confidence: HIGH  
Status: ✅ Ready for verification
```

---

## 🎓 总结

### Developer的职责
1. **不只是说"完成了"**
2. **提供可验证的证据** (commits, test commands)
3. **明确验收标准** (不要模糊的"所有测试通过")

### QA的职责
1. **不盲目相信Developer**
2. **执行独立验证**
3. **文档化验收决定**

### 协作的关键
**共享文档 + 明确标准 + 可追溯证据**

---

*Best Practices Guide v1.0*  
*Created: 2025-12-26 01:35*
