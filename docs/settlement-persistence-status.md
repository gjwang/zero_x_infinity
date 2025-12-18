# Settlement Persistence - Phase 1-3 完成

## 📦 已实现功能

### 1. 基础设施 ✅
- TDengine 客户端连接管理
- Schema 初始化（Super Tables for orders/trades/balances/order_events）
- 持久化模块结构 (`src/persistence/`)

### 2. 数据模型扩展 ✅
- `InternalOrder.cid: Option<String>` - 客户端订单 ID
- `Trade.fee: u64` - 手续费（占位符，当前为 0）
- `Trade.role: u8` - Maker/Taker 角色（占位符，当前为 0）

### 3. 持久化实现 ✅
- **Orders**: `insert_order()`, `update_order_status()`, `insert_order_event()`
- **Trades**: `insert_trade()` (每笔交易插入买卖双方记录), `batch_insert_trades()`
- **Balances**: `snapshot_balance()`, `batch_snapshot_balances()`

### 4. Gateway 查询端点 ✅
- `GET /api/v1/order/:order_id` - 查询单个订单
- `GET /api/v1/orders` - 查询订单列表
- `GET /api/v1/trades` - 查询成交历史
- `GET /api/v1/balances` - 查询用户余额

**注意**: 当前返回 `501 NOT_IMPLEMENTED`，实际查询逻辑待 Phase 4 实现

### 5. 配置 ✅
- 添加 `PersistenceConfig` 到 `config.rs`
- `dev.yaml` 中添加 persistence 配置（默认 `enabled: false`）

## 🚀 使用方法

### 启动 TDengine

```bash
docker run -d \
  --name tdengine \
  -p 6030:6030 \
  -p 6041:6041 \
  tdengine/tdengine:latest
```

### 启用持久化

修改 `config/dev.yaml`:
```yaml
persistence:
  enabled: true
  tdengine_dsn: "taos+ws://root:taosdata@localhost:6041"
```

### 测试连接

```bash
# 运行测试（需要 TDengine 运行）
cargo test --lib persistence -- --ignored
```

## ⏭️ 下一步 (Phase 4)

1. **集成 Settlement 线程**
   - 在 `pipeline_mt.rs` 的 Settlement 线程中调用持久化函数
   - 异步写入 trades, orders, balances

2. **初始化 TDengineClient**
   - 在 `main.rs` 中根据配置初始化客户端
   - 传递给 Gateway 的 AppState

3. **实现查询逻辑**
   - 实现 `get_order()`, `get_orders()`, `get_trades()`, `get_balances()`
   - 处理 TDengine 查询结果的类型转换

## 📝 技术要点

### 占位符字段
- `Trade.fee` 和 `Trade.role` 当前为 0
- 未来需要根据交易对配置计算手续费
- 需要从匹配引擎获取 Maker/Taker 信息

### 错误处理
- 使用 `anyhow` 进行错误处理
- taos crate 不支持 `.context()`，使用 `.map_err()` 替代

### Balance 字段访问
- Balance 字段为私有，使用访问器方法：
  - `balance.avail()`
  - `balance.frozen()`
  - `balance.lock_version()`
  - `balance.settle_version()`

## 🔗 相关文档

- 设计文档: `docs/src/0x09-b-settlement-persistence.md`
- 数据库选型: `docs/src/database-selection-tdengine.md`
- API 规范: `docs/src/api-conventions.md`

## ✅ 编译状态

```bash
cargo check  # ✅ 通过
```
