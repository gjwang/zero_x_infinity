# Architect → Developer Handover: Best Practices

> **Purpose**: 标准化Architect完成设计后向Developer传递的流程  
> **Audience**: Architect Agent, Developer Agent, Project Leads

---

## 🎯 核心原则

**Developer不能仅凭Architect说"设计完成"就开始实现**。需要：
1. ✅ 完整的设计文档包
2. ✅ 明确的实施计划
3. ✅ 验收标准和测试策略
4. ✅ 关键设计决策说明

---

## 📋 标准交接流程

### Step 1: Architect创建设计包

**设计包结构**:
```
📁 0xXX Design Package
├── 🏛️ Architecture (Architect创建)
│   ├── 0xXX-architecture-design.md     # 顶层架构
│   └── 0xXX-[component]-design.md      # 组件设计
│
├── 📋 Handover (Architect创建)
│   ├── 0xXX-implementation-plan.md     # → Developer
│   └── 0xXX-test-checklist.md          # → QA
│
└── 📖 Walkthrough (Architect创建)
    └── 0xXX-walkthrough.md              # 团队overview
```

### Step 2: 创建交接文档

**文件位置**: `docs/agents/sessions/shared/arch-to-dev-handover.md`

**必需内容**:
```markdown
# Architect → Developer: [Feature Name] Handover

## 📦 设计交付物

- [x] Architecture Design: `docs/agents/sessions/architect/0xXX-*.md`
- [x] Implementation Plan: `docs/agents/sessions/developer/0xXX-impl-plan.md`
- [x] Test Checklist: `docs/agents/sessions/qa/0xXX-test-checklist.md`
- [x] Walkthrough: `docs/agents/sessions/architect/0xXX-walkthrough.md`

## 🎯 实施目标

**ONE SENTENCE**: [简洁描述这个设计要实现什么]

**关键指标**:
- Performance: [例如: 1M ops/sec]
- Reliability: [例如: 99.99% uptime]
- Scalability: [例如: 支持100个交易对]

## 📋 实施计划概要

### Phase 1: [Core功能] (Priority P0)
- Task 1.1: [具体任务] (~X days)
- Task 1.2: [具体任务] (~X days)

### Phase 2: [扩展功能] (Priority P1)
- Task 2.1: ...

## 🔑 关键设计决策

| 决策 | 原因 | 替代方案 |
|------|------|---------|
| [方案A] | [为什么选择] | [考虑过但未采用的方案] |

## ⚠️ 实施注意事项

### DO (必须)
- [ ] 遵循WAL v2格式规范
- [ ] 使用bincode序列化
- [ ] 实现CRC32校验

### DON'T (禁止)
- [ ] 不要跳过checksum验证
- [ ] 不要使用JSON格式存储二进制数据
- [ ] 不要在热路径使用mutex

## 📝 代码示例

关键接口/结构体示例:
\`\`\`rust
// 预期的API设计
pub struct WalWriter {
    fn new(path: impl AsRef<Path>) -> Result<Self>;
    fn append(&mut self, entry: &WalEntry) -> Result<u64>;
    fn flush(&mut self) -> Result<()>;
}
\`\`\`

## ✅ 验收标准

### 功能验收
- [ ] WAL写入成功
- [ ] Snapshot创建成功
- [ ] Recovery恢复正确

### 性能验收
- [ ] 写入TPS > 100,000
- [ ] 恢复时间 < 30s (100K records)

### 质量验收
- [ ] 单元测试覆盖率 > 80%
- [ ] 所有clippy警告解决
- [ ] 文档注释完整

## 🔗 相关文档

- Architecture: [link]
- Detailed Design: [link]
- Reference Implementation: [link if exists]

## 📞 Ready for Development

Architect签名: @Architect AI Agent  
Date: YYYY-MM-DD  
Status: ✅ Ready for implementation
```

---

### Step 3: Developer接收并确认

**Developer工作流**:

#### 3.1 检查设计包完整性
```bash
# 确认所有设计文档存在
ls docs/agents/sessions/architect/0xXX-*.md
ls docs/agents/sessions/developer/0xXX-impl-plan.md
```

#### 3.2 理解设计意图
- 阅读Architecture Design理解全局
- 阅读Implementation Plan理解任务分解
- 阅读Key Decisions理解技术选择

#### 3.3 创建确认文档
**文件**: `docs/agents/sessions/developer/0xXX-dev-ack.md`

```markdown
# Developer Acknowledgment: [Feature Name]

## 📋 设计包验收

- [x] Architecture文档已阅读
- [x] Implementation Plan已理解
- [x] Key Decisions已认同
- [ ] 有疑问需要澄清 (见下)

## ❓ 问题/澄清

| 问题 | 文档位置 | 建议 |
|------|---------|------|
| [问题描述] | [文件:行号] | [建议解决方案] |

## 📊 工作量评估

| Phase | Architect估计 | Developer评估 | 差异原因 |
|-------|-------------|--------------|---------|
| Phase 1 | 3-5 days | 4-6 days | [原因] |

## ✅ 开始实施

Developer签名: @Developer AI  
Date: YYYY-MM-DD  
Status: ✅ Ready to start
```

---

## 🔄 完整交接流程图

```
Architect完成设计
        ↓
创建设计包 (architecture + impl-plan + test-checklist)
        ↓
创建 arch-to-dev-handover.md
        ↓
═══════════════════════════════════════
        ↓
Developer读取设计文档
        ↓
理解设计意图和关键决策
        ↓
    ┌────────┐
    │ 有疑问? │
    └────┬───┘
         │
    ┌────┴────┐
    │         │
   YES       NO
    │         │
    ↓         ↓
创建clarification请求   创建dev-ack.md
    │         │
    ↓         ↓
Architect回复        开始实施
    │
    └──→ 重新评估 → 继续流程
```

---

## 📁 文件组织结构

```
docs/agents/sessions/
├── shared/
│   ├── arch-to-dev-handover.md    # Architect → Developer交接
│   └── dev-to-qa-handover.md      # Developer → QA交接
│
├── architect/
│   ├── 0xXX-architecture-design.md
│   ├── 0xXX-walkthrough.md
│   └── current-task.md
│
├── developer/
│   ├── 0xXX-impl-plan.md
│   ├── 0xXX-dev-ack.md            # Developer确认
│   └── current-task.md
│
└── qa/
    └── 0xXX-test-checklist.md
```

---

## 🚨 常见错误及避免方法

### ❌ 错误1: 只给设计，不给实施计划
**问题**: Developer不知道从哪开始

**正确做法**:
```markdown
❌ "设计在docs/architecture/中"
✅ "设计在docs/architecture/中，实施计划在docs/developer/0xXX-impl-plan.md，
   建议从Phase 1 Task 1.1开始"
```

### ❌ 错误2: 没有解释关键决策
**问题**: Developer可能做出不一致的选择

**正确做法**:
```markdown
❌ "使用bincode序列化"
✅ "使用bincode序列化 (比JSON快10x，降低WAL文件大小50%)
   替代方案: JSON (可读性好但太慢), protobuf (需要额外schema)"
```

### ❌ 错误3: 验收标准模糊
**问题**: Developer不知道"完成"是什么

**正确做法**:
```markdown
❌ "实现WAL功能"
✅ "实现WAL功能，满足以下标准:
   - 写入TPS > 100,000
   - CRC32校验通过率100%
   - Recovery成功恢复所有记录"
```

---

## 📊 Checklist Template

### Architect交付前自检
- [ ] 设计文档完整 (architecture + detailed design)
- [ ] 实施计划创建 (tasks + timeline + priorities)
- [ ] 测试清单创建 (for QA)
- [ ] 关键决策记录 (decisions + rationale)
- [ ] 代码示例提供 (API signatures)
- [ ] 验收标准明确 (功能 + 性能 + 质量)
- [ ] Walkthrough可读

### Developer接收确认
- [ ] 阅读了所有设计文档
- [ ] 理解了实施计划
- [ ] 认同关键决策 (或提出澄清)
- [ ] 评估了工作量
- [ ] 创建了dev-ack.md

---

## 💡 最佳实践示例

### 优秀的Handover文档示例

```markdown
# Architect → Developer: 0x0D WAL & Snapshot

## 📦 设计交付物

- [x] Architecture: `0x0D-wal-rotation-design.md`
- [x] UBSCore Design: `0x0D-ubscore-wal-snapshot.md`
- [x] Matching Design: `0x0D-matching-wal-snapshot.md`
- [x] Implementation Plan: `0x0D-implementation-plan.md`
- [x] Test Checklist: `0x0D-test-checklist.md`

## 🎯 实施目标

**ONE SENTENCE**: 实现OrderBook状态持久化，支持crash后秒级恢复

**关键指标**:
- WAL写入: > 100,000 TPS
- Snapshot创建: < 100ms
- Recovery时间: < 30s (1M orders)

## 🔑 关键设计决策

| 决策 | 原因 | 替代方案 |
|------|------|---------|
| WAL v2 20-byte header | 跨服务通用格式 | 服务专用格式(难维护) |
| Bincode序列化 | 速度快，体积小 | JSON(可读但慢), Protobuf(需schema) |
| CRC32校验 | 标准，够用 | CRC64(过度), MD5(太慢) |

## ⚠️ 实施注意事项

### DO
- [x] 使用BufWriter提高IO效率
- [x] 每100笔flush一次
- [x] Snapshot使用临时目录+原子rename

### DON'T
- [x] 不要在每次write后fsync
- [x] 不要直接覆盖snapshot文件
- [x] 不要跳过COMPLETE marker检查

## 📞 Ready for Development

Architect: @Architect AI  
Date: 2025-12-25  
Confidence: HIGH
```

---

## 🎓 总结

### Architect的职责
1. **不只是设计文档** - 要提供完整的设计包
2. **解释"为什么"** - 关键决策需要说明理由
3. **明确"完成标准"** - 提供可验证的验收标准

### Developer的职责
1. **全面理解设计** - 不要只看API表面
2. **质疑不清楚的地方** - 提出澄清请求
3. **确认工作量** - 评估是否合理

### 协作的关键
**完整设计包 + 明确验收标准 + 双向确认**

---

*Best Practices Guide v1.0*  
*Created: 2025-12-26*
