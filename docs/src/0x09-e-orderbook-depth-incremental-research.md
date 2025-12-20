# Order Book Depth 增量更新方案调研

> **调研目的**：为未来实现 WebSocket 增量推送做技术储备，对比主流交易所方案。

---

## 1. Binance 方案

### 1.1 核心机制

**Snapshot + Delta 模式**

```
客户端流程：
1. 连接 WebSocket
2. 缓冲所有增量消息
3. 获取 REST API 快照（带 lastUpdateId）
4. 丢弃过期的缓冲消息（u <= lastUpdateId）
5. 应用剩余缓冲消息
6. 持续应用新的增量消息
```

### 1.2 消息格式

**增量更新消息：**
```json
{
  "e": "depthUpdate",
  "E": 1234567890,
  "s": "BTCUSDT",
  "U": 157,
  "u": 160,
  "b": [
    ["0.0024", "10"],
    ["0.0025", "0"]
  ],
  "a": [
    ["0.0026", "100"]
  ]
}
```

**字段说明：**
- `e`: 事件类型
- `E`: 事件时间（毫秒）
- `s`: 交易对
- `U`: 本次更新的第一个 update ID
- `u`: 本次更新的最后一个 update ID
- `b`: 买单更新（价格, 数量）
- `a`: 卖单更新（价格, 数量）
- **重要**：数量为 `0` 表示删除该价格档位

### 1.3 同步逻辑

```python
# 伪代码
buffer = []
ws.connect()

# 缓冲消息
while True:
    msg = ws.receive()
    buffer.append(msg)
    
    # 获取快照
    snapshot = rest_api.get_depth()
    
    # 丢弃过期消息
    buffer = [m for m in buffer if m['u'] > snapshot['lastUpdateId']]
    
    # 找到第一个有效消息
    for msg in buffer:
        if msg['U'] <= snapshot['lastUpdateId'] + 1 and msg['u'] >= snapshot['lastUpdateId'] + 1:
            apply(msg)
            break
    
    # 持续应用
    while True:
        msg = ws.receive()
        if msg['pu'] != prev_msg['u']:
            # 检测到遗漏，重新同步
            resync()
        apply(msg)
```

### 1.4 优缺点

**✅ 优点：**
- 带宽高效（只传输变化）
- 容错性好（序列号检测遗漏）
- 生态成熟（最大用户群，现成库）
- 客户端逻辑清晰（`qty=0` 删除）

**❌ 缺点：**
- 初始同步复杂（需要缓冲 + REST 快照）
- 客户端需要维护完整 order book
- 初始延迟较高（~500ms）

**📚 参考文档：**
- [Binance WebSocket Depth Streams](https://binance-docs.github.io/apidocs/spot/en/#diff-depth-stream)
- [How to manage a local order book correctly](https://binance-docs.github.io/apidocs/spot/en/#how-to-manage-a-local-order-book-correctly)

---

## 2. Coinbase 方案

### 2.1 核心机制

**Snapshot + L2 Update 模式**

```
客户端流程：
1. 订阅 level2 channel
2. 接收初始 snapshot
3. 应用后续 l2update 消息
```

### 2.2 消息格式

**初始快照：**
```json
{
  "type": "snapshot",
  "product_id": "BTC-USD",
  "bids": [["10101.10", "0.45054140"]],
  "asks": [["10102.55", "0.57753524"]]
}
```

**增量更新：**
```json
{
  "type": "l2update",
  "product_id": "BTC-USD",
  "time": "2019-08-14T20:42:27.265Z",
  "changes": [
    ["buy", "10101.80", "0.162567"],
    ["sell", "10103.84", "0.0"]
  ]
}
```

**changes 格式：**
- `[side, price, size]`
- `size = "0"` 表示删除

### 2.3 同步逻辑

```python
# 伪代码
ws.subscribe("level2", "BTC-USD")

# 接收快照
snapshot = ws.receive()
order_book = init_from_snapshot(snapshot)

# 应用增量
while True:
    update = ws.receive()
    for change in update['changes']:
        side, price, size = change
        if size == "0":
            order_book.remove(side, price)
        else:
            order_book.update(side, price, size)
```

### 2.4 优缺点

**✅ 优点：**
- 更简单的消息格式
- 明确的 snapshot/update 类型
- 客户端逻辑更简单（无需 REST 调用）
- 初始延迟低

**❌ 缺点：**
- 无序列号检测（无法发现遗漏）
- 生态较小
- 容错性较弱

**📚 参考文档：**
- [Coinbase Advanced Trade WebSocket API](https://docs.cloud.coinbase.com/advanced-trade-api/docs/ws-overview)
- [Level2 Channel](https://docs.cloud.coinbase.com/advanced-trade-api/docs/ws-channels#level2-channel)

---

## 3. Kraken 方案

### 3.1 核心机制

**Snapshot + Delta + Checksum 模式**

```
客户端流程：
1. 订阅 book channel
2. 接收初始快照
3. 应用增量更新
4. 定期验证 checksum
```

### 3.2 消息格式

**快照消息：**
```json
{
  "channelID": 10001,
  "data": {
    "as": [
      ["5541.30000", "2.50700000", "1534614248.123678"],
      ["5541.20000", "0.40100000", "1534614248.345543"]
    ],
    "bs": [
      ["5541.20000", "1.52900000", "1534614248.456738"],
      ["5541.00000", "0.30000000", "1534614248.871234"]
    ]
  },
  "channelName": "book-10",
  "pair": "XBT/USD"
}
```

**增量更新：**
```json
{
  "channelID": 10001,
  "data": {
    "a": [["5541.30000", "0.00000000", "1534614335.345903"]],
    "c": "974942666"
  },
  "channelName": "book-10",
  "pair": "XBT/USD"
}
```

**字段说明：**
- `as`/`a`: asks（卖单）
- `bs`/`b`: bids（买单）
- `c`: checksum（CRC32）
- 每个档位：`[price, volume, timestamp]`

### 3.3 Checksum 验证

```python
# 伪代码
def compute_checksum(order_book):
    # 取前 10 档 bid 和 ask
    bids = order_book.bids[:10]
    asks = order_book.asks[:10]
    
    # 拼接字符串
    s = ""
    for ask in asks:
        s += ask.price.replace(".", "")
        s += ask.volume.replace(".", "")
    for bid in bids:
        s += bid.price.replace(".", "")
        s += bid.volume.replace(".", "")
    
    # 计算 CRC32
    return crc32(s) & 0xFFFFFFFF

# 验证
if msg['c']:
    if compute_checksum(order_book) != int(msg['c']):
        # 不匹配，重新同步
        resync()
```

### 3.4 优缺点

**✅ 优点：**
- Checksum 验证完整性（最可靠）
- 时间戳精确（微秒级）
- 容错性最好

**❌ 缺点：**
- Checksum 计算开销
- 生态较小
- 实现复杂度高

**📚 参考文档：**
- [Kraken WebSocket API](https://docs.kraken.com/websockets/)
- [Book Channel](https://docs.kraken.com/websockets/#message-book)

---

## 4. 方案对比总结

| 特性 | Binance | Coinbase | Kraken |
|------|---------|----------|--------|
| **初始同步** | REST + 缓冲 | WebSocket 快照 | WebSocket 快照 |
| **增量格式** | `[price, qty]` | `[side, price, size]` | `[price, volume, timestamp]` |
| **删除表示** | `qty=0` | `size=0` | `volume=0` |
| **序列号** | ✅ U/u | ❌ 无 | ❌ 无 |
| **完整性验证** | 序列号 | 无 | ✅ Checksum |
| **时间戳** | 毫秒 | ISO 8601 | 微秒 |
| **生态成熟度** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ |
| **实现复杂度** | 中 | 低 | 高 |
| **容错性** | 好 | 中 | 最好 |
| **带宽效率** | 高 | 高 | 高 |

---

## 5. 性能对比

### 5.1 服务端开销

| 方案 | Diff 算法 | 时间复杂度 | 实际开销 | 内存开销 |
|------|-----------|-----------|---------|---------|
| Binance | HashMap 对比 | O(n) | ~2μs | ~10KB |
| Coinbase | HashMap 对比 | O(n) | ~2μs | ~10KB |
| Kraken | HashMap + CRC32 | O(n) | ~5μs | ~10KB |

**结论：** 三种方案服务端开销都很小，可忽略不计。

### 5.2 客户端开销

| 方案 | 初始延迟 | 内存 | CPU |
|------|---------|------|-----|
| Binance | ~500ms（REST + 缓冲） | 中 | 低 |
| Coinbase | ~100ms（WebSocket 快照） | 中 | 低 |
| Kraken | ~100ms + Checksum | 中 | 中（CRC32） |

**结论：** Coinbase 初始延迟最低，Kraken CPU 开销稍高。

---

## 6. 推荐方案

### 6.1 对于我们的系统

**推荐：Binance 方案**

**理由：**
1. **生态最大**：用户熟悉，现成库多
2. **容错性好**：序列号检测遗漏
3. **实现可控**：服务端开销小（~2μs）
4. **渐进式**：可以先实现快照，后续加增量

### 6.2 优化建议

**优化 1：首次连接发送快照**
```
客户端连接 → 立即发送完整快照 → 后续发送增量
无需 REST 调用，减少延迟
```

**优化 2：可选 Checksum**
```
定期发送 checksum（如 Kraken）
客户端可选验证
```

**优化 3：渐进式实现**
```
Phase 1: 快照（已完成）
Phase 2: 增量更新（下一步）
Phase 3: Checksum 验证（可选）
```

---

## 7. 实现路线图

### Phase 1: 快照模式 ✅
- [x] DepthSnapshot 消息
- [x] 定时快照（100ms）
- [x] HTTP API `/api/v1/depth`

### Phase 2: 增量更新（待实现）
- [ ] DepthUpdate 消息类型
- [ ] HashMap-based diff 算法
- [ ] WebSocket `depth.update` 事件
- [ ] update_id 管理

### Phase 3: 高级特性（可选）
- [ ] Checksum 验证
- [ ] 历史消息缓存（重连恢复）
- [ ] 压缩传输

---

## 8. 参考资料

### 官方文档
- [Binance WebSocket API](https://binance-docs.github.io/apidocs/spot/en/#websocket-market-streams)
- [Coinbase Advanced Trade WebSocket](https://docs.cloud.coinbase.com/advanced-trade-api/docs/ws-overview)
- [Kraken WebSocket API](https://docs.kraken.com/websockets/)

### 技术文章
- [How to Build a Crypto Order Book](https://medium.com/@coinapi/how-to-build-a-crypto-order-book-6c7f3b8c5f5e)
- [Order Book Data Structures](https://web.archive.org/web/20110219163448/http://howtohft.wordpress.com/2011/02/15/how-to-build-a-fast-limit-order-book/)

### 开源实现
- [ccxt](https://github.com/ccxt/ccxt) - 统一交易所 API
- [binance-connector-python](https://github.com/binance/binance-connector-python) - Binance 官方 Python SDK

---

## 9. 总结

**核心结论：**
1. Binance 方案是行业标准，推荐采用
2. 服务端实现成本低（~2μs diff）
3. 可以渐进式实现（先快照，后增量）
4. 优化初始同步可以降低延迟

**下一步：**
- 实现 DepthUpdate 消息类型
- 实现 HashMap-based diff 算法
- 集成到 WebSocket 推送系统
