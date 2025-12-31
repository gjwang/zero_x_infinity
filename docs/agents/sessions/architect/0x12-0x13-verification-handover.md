# Architect → Developer Handover: Phase 0x12/0x13 Verification

> **Branch**: `0x12-0x13-verification` (创建于 0x14-c 合并后)
> **Design Spec**: [0x12-real-trading.md](../../src/0x12-real-trading.md), [0x13-market-data.md](../../src/0x13-market-data.md)
> **Date**: 2025-12-31
> **Status**: ⏳ 待 0x14-c 完成后启动

---

## 1. Objective

**验证 Phase 0x12 和 0x13 的 E2E 功能完整性，将状态从 "Code Ready" 更新为 "Verified"。**

当前状态：
| Phase | Status | Gap |
|-------|--------|-----|
| 0x12 Real Trading | 🔸 Code Ready | 需要 E2E 测试验证完整交易流程 |
| 0x13 Market Data | 🔸 Code Ready | 需要 WebSocket + REST API 验证 |

---

## 2. Scope (范围)

### 2.1 Phase 0x12: Real Trading Verification

**测试流程:**
```
User Registration → Deposit (Mock) → Place Order → Match → Trade → Balance Update
```

**验证点:**
| 步骤 | 验证内容 |
|------|----------|
| Deposit | 余额正确增加 |
| Place Order | 订单状态 = NEW，Balance frozen 正确 |
| Match | Trade 生成，价格/数量正确 |
| Trade | 双方余额正确结算 |
| Persistence | Trades 出现在 TDengine |

### 2.2 Phase 0x13: Market Data Verification

**WebSocket 验证:**
| Stream | 验证内容 |
|--------|----------|
| `@trade` | 成交后推送 trade 事件 |
| `@depth` | 订单变动后深度更新 |
| `@ticker` | 成交后 ticker 更新 |

**REST API 验证:**
| Endpoint | 验证内容 |
|----------|----------|
| `/api/v1/public/klines` | 返回正确聚合数据 |
| `/api/v1/public/trades` | 返回最近成交列表 |
| `/api/v1/public/depth` | 返回当前深度快照 |

---

## 3. Implementation Guide (实施指南)

### 3.1 创建验证脚本

**文件**: `scripts/tests/verify_0x12_trading_e2e.py`

```python
#!/usr/bin/env python3
"""
Phase 0x12 Real Trading E2E Verification
"""

import requests
import time

GATEWAY_URL = "http://localhost:8080"

def test_trading_e2e():
    # 1. Register two users
    user1 = register_user("trader1")
    user2 = register_user("trader2")
    
    # 2. Deposit funds
    deposit(user1, "USDT", "10000.00")
    deposit(user2, "BTC", "1.00")
    
    # 3. User1 places buy order
    order1 = place_order(user1, "BTCUSDT", "BUY", "0.1", "50000.00")
    assert order1["status"] == "NEW"
    
    # 4. User2 places sell order (should match)
    order2 = place_order(user2, "BTCUSDT", "SELL", "0.1", "50000.00")
    
    # 5. Wait for match
    time.sleep(0.5)
    
    # 6. Verify trade
    trades = get_trades("BTCUSDT")
    assert len(trades) >= 1
    
    # 7. Verify balances
    assert get_balance(user1, "BTC") == "0.1"  # Received BTC
    assert get_balance(user2, "USDT") >= "4999.00"  # Received USDT (minus fee)
    
    print("✅ Phase 0x12 Trading E2E PASSED")
```

**文件**: `scripts/tests/verify_0x13_market_data.py`

```python
#!/usr/bin/env python3
"""
Phase 0x13 Market Data E2E Verification
"""

import asyncio
import websockets
import json

async def test_websocket_streams():
    async with websockets.connect("ws://localhost:8080/ws") as ws:
        # Subscribe to trade stream
        await ws.send(json.dumps({"method": "SUBSCRIBE", "params": ["btcusdt@trade"]}))
        
        # Trigger a trade (via REST API in another thread)
        # ...
        
        # Wait for trade event
        msg = await asyncio.wait_for(ws.recv(), timeout=5.0)
        data = json.loads(msg)
        assert data["stream"] == "btcusdt@trade"
        
    print("✅ Phase 0x13 WebSocket Streams PASSED")
```

---

## 4. Verification (验证)

### 4.1 运行验证脚本
```bash
# Start dependencies
docker-compose up -d postgres tdengine

# Start Gateway
cargo run --release -- --gateway --port 8080 &

# Run verification
uv run python scripts/tests/verify_0x12_trading_e2e.py
uv run python scripts/tests/verify_0x13_market_data.py
```

### 4.2 Update Documentation
验证通过后，更新以下文档：
1. `docs/src/0x12-real-trading.md` - Status: ✅ Verified
2. `docs/src/0x13-market-data.md` - Status: ✅ Verified
3. `docs/src/0x00-mvp-roadmap.md` - Phase IV: Complete

---

## 5. Definition of Done (完成标准)

- [ ] `verify_0x12_trading_e2e.py` 通过
- [ ] `verify_0x13_market_data.py` 通过
- [ ] 文档状态更新
- [ ] Roadmap 更新

---

## 6. Acceptance (验收)

完成后请创建 **Dev → Arch Handover** 报告，包含：
1. 测试结果截图/日志
2. 任何发现的问题或设计变更建议
