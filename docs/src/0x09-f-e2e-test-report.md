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
| [inject_orders.py](../scripts/inject_orders.py) | 顺序注入 CSV 订单到 Gateway API |
| [test_gateway_e2e_full.sh](../scripts/test_gateway_e2e_full.sh) | 完整 E2E 测试流程 |

---

## 4. 测试结果

### 4.1 订单注入

| 指标 | 结果 |
|------|------|
| 注入数量 | 1,100 (100 + 1,000) |
| 成功接受 | 1,100 (100%) ✅ |
| 失败数 | 0 |
| 吞吐量 | 2,052 orders/sec |
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

**跟踪**: [pipeline-mt-tdengine-gap.md](issues/pipeline-mt-tdengine-gap.md)

---

## 6. 测试执行记录

```bash
# Step 1: 启动 TDengine
docker start tdengine

# Step 2: 启动 Gateway
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

## 8. 待办事项

- [ ] **P1**: 修复 Orders 持久化
- [ ] **P1**: 修复 Balances 持久化
- [ ] **P2**: 修复 Pipeline MT TDengine 集成
- [ ] 添加持久化自动化测试
- [ ] 运行 100K 完整注入测试 (当前仅测试 1,100)
