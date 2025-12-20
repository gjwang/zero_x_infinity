# Gateway E2E 注入测试报告

**测试日期**: 2025-12-20  
**测试人员**: Antigravity AI  
**测试分支**: `0x09-f-integration-test`

---

## 1. 测试目标

验证 Gateway HTTP API 是否能正确接收订单并持久化到 TDengine。

---

## 2. 测试环境

| 组件 | 版本/状态 |
|------|----------|
| Gateway | `./target/release/zero_x_infinity --gateway --port 8080` |
| TDengine | Docker `tdengine/tdengine:latest` |
| 数据库名 | `trading` |
| 测试数据 | `fixtures/orders.csv` (100K 订单) |

---

## 3. 测试工具

| 工具 | 用途 |
|------|------|
| `scripts/inject_orders.py` | 顺序注入 CSV 订单到 Gateway API |
| `scripts/test_gateway_e2e_full.sh` | 完整 E2E 测试流程 |

---

## 4. 测试结果

### 4.1 订单注入

| 指标 | 结果 |
|------|------|
| 注入数量 | 1,000 |
| 成功接受 | 1,000 (100%) ✅ |
| 失败数 | 0 |
| 吞吐量 | 2,272 orders/sec |
| 注入模式 | 顺序单线程 (保证订单确定性) |

### 4.2 TDengine 持久化

| 表 | 记录数 | 预期 | 状态 |
|----|--------|------|------|
| `trades` | 1,262 | ~1,262 | ✅ PASS |
| `orders` | 0 | ~1,100 | ❌ FAIL |
| `balances` | 0 | >0 | ❌ FAIL |

---

## 5. 发现的问题

### 5.1 P1: Orders/Balances 不持久化到 TDengine

**严重性**: P1 (生产阻塞)

**现象**:
- 订单通过 Gateway API 提交成功 (HTTP 202 Accepted)
- Trades 正确持久化到 TDengine
- Orders 和 Balances 表为空

**根因分析**:

位置: `src/websocket/service.rs` → `WsService.handle_event`

```rust
// PushEvent::Trade → 有持久化代码 ✅
if let Some(ref db_client) = self.db_client {
    insert_trade_record(...).await;
}

// PushEvent::OrderUpdate → 只有 WebSocket 推送，无持久化 ❌
self.manager.send_to_user(user_id, message);

// PushEvent::BalanceUpdate → 只有 WebSocket 推送，无持久化 ❌
self.manager.send_to_user(user_id, message);
```

**影响**:
- API 查询 `/api/v1/orders` 无数据返回
- API 查询 `/api/v1/balances` 无数据返回
- 交易历史可查询，但订单和余额历史丢失

**建议修复**:
在 `WsService.handle_event` 中为 `OrderUpdate` 和 `BalanceUpdate` 添加 TDengine 持久化逻辑。

---

### 5.2 P2: Pipeline MT 不持久化到 TDengine

**严重性**: P2

**现象**: `Pipeline MT` 模式只输出 CSV，不写入 TDengine

**影响**: 无法通过 TDengine 验证 Pipeline 结果

**跟踪**: 待创建 issue 文档

---

## 6. 测试执行记录

```bash
# 使用 E2E 测试脚本 (推荐)
./scripts/test_gateway_e2e_full.sh quick

# 或手动执行：
# Step 1: 启动 TDengine
docker start tdengine

# Step 2: 启动 Gateway (使用脚本会自动启动)
./target/release/zero_x_infinity --gateway --port 8080 &

# Step 3: 注入 1000 订单
python3 scripts/inject_orders.py --input fixtures/orders.csv --limit 1000

# Step 4: 验证 TDengine 数据
docker exec tdengine taos -s "USE trading; \
  SELECT COUNT(*) as orders FROM orders; \
  SELECT COUNT(*) as trades FROM trades; \
  SELECT COUNT(*) as balances FROM balances"
```

---

## 7. 结论

| 测试项 | 结果 |
|--------|------|
| Gateway HTTP API 接收订单 | ✅ PASS |
| 订单匹配引擎处理 | ✅ PASS |
| Trades 持久化 | ✅ PASS |
| Orders 持久化 | ❌ FAIL (P1 Bug) |
| Balances 持久化 | ❌ FAIL (P1 Bug) |

**总体评估**: Gateway 核心功能正常，但持久化模块不完整，需修复后才能用于生产。

---

## 8. IDE 崩溃问题

### 现象
在执行复合命令时，Antigravity IDE 崩溃：
```
Antigravity server crashed unexpectedly. Please restart to fully restore AI features.
```

### 触发条件
| 命令类型 | 结果 |
|----------|------|
| 单独 `echo` | ✅ 正常 |
| 单独 `docker exec taos` | ✅ 正常 |
| 单独 `pkill` | ✅ 正常 |
| **复合命令** `pkill + docker exec` | ❌ 崩溃 |

### 错误日志
```
got retryable useReactiveState RPC error on attempt [3 / Infinity]
ConnectError: [unknown] reactive component f3e07a43-...not found
```

### 根因分析

**问题**：`pkill -f "zero_x_infinity"` 不仅 kill 了 Gateway，还 kill 了 IDE 的 language_server！

```bash
# IDE 进程的命令行包含项目路径：
/Applications/Antigravity.app/.../language_server_macos_arm \
  --workspace_id file_Users_gjwang_eclipse_workspace_rust_source_zero_x_infinity
```

`pkill -f "zero_x_infinity"` 会匹配到这个进程，导致 IDE 崩溃。

### 对比测试结果
| 命令 | 结果 |
|------|------|
| `kill <PID>` | ✅ 不崩溃 |
| `pkill -f "zero_x_infinity"` | ❌ 崩溃 |

### 修复方案 (commit 8e89167)
使用更精确的匹配 + kill：
```bash
# 不要这样：
pkill -f "zero_x_infinity"  # 会 kill IDE！

# 应该这样：
GW_PID=$(pgrep -f "./target/release/zero_x_infinity" | head -1)
kill "$GW_PID"
```

---

## 9. 待办事项

> **Owner**: TBD  
> **Created**: 2025-12-20  
> **Branch**: `0x09-f-integration-test`

### P1 - 生产阻塞
| 任务 | 状态 | 预计 |
|------|------|------|
| 修复 Orders 持久化 | ⏳ Pending | 0x09-g |
| 修复 Balances 持久化 | ⏳ Pending | 0x09-g |

### P2 - 功能增强
| 任务 | 状态 | 预计 |
|------|------|------|
| 修复 Pipeline MT TDengine 集成 | ⏳ Pending | 0x09-h |
| 添加 `/api/v1/health` 健康检查 API | ⏳ Pending | 0x09-h |

### P3 - 测试优化
| 任务 | 状态 | 预计 |
|------|------|------|
| 添加持久化自动化测试 | ⏳ Pending | 0x10 |
| 运行 100K 完整注入测试 | ⏳ Pending | 0x10 |
