# QA E2E Testing Requirements

## 真正的 E2E 测试

### 什么是真正的 E2E？

```
Admin Dashboard → PostgreSQL → Gateway API → Matching Engine
```

**不是**只测试一个服务！要测试**完整链路**。

---

## 核心 E2E 测试场景

### E2E-01: Asset Creation Propagation ✅
**链路**: Admin创建 → DB写入 → Gateway读取

```python
1. Admin API: POST /admin/api/asset/create
2. Wait 1s for propagation
3. Gateway API: GET /api/v1/assets
4. Verify: Asset appears in Gateway response
```

### E2E-02: Symbol Creation Propagation ✅
**链路**: Admin创建 → DB写入 → Gateway读取

```python
1. Admin API: POST /admin/api/symbol/create
2. Wait 1s
3. Gateway API: GET /api/v1/symbols
4. Verify: Symbol appears in Gateway
```

### E2E-03: Symbol Status Change ✅
**链路**: Admin修改 → DB更新 → Gateway反映

```python
1. Admin API: PATCH /admin/api/symbol/update/{id}
   - Set status=0 (halt)
2. Wait 2s for Gateway reload
3. Gateway API: GET /api/v1/symbols
4. Verify: Symbol status=0 in Gateway
```

### E2E-04: Fee Update Propagation ✅
**链路**: Admin改费率 → DB更新 → Gateway返回新费率

```python
1. Admin API: PATCH /admin/api/symbol/update/{id}
   - Update base_maker_fee, base_taker_fee
2. Wait 2s
3. Gateway API: GET /api/v1/symbols
4. Verify: New fees in Gateway response
```

---

## 为什么从 Gateway API 验证？

### ✅ 正确做法
```
Admin修改 → DB → Gateway API读取
```
- 测试完整链路
- 验证 Gateway 热加载
- 确保配置生效

### ❌ 错误做法
```
Admin修改 → DB查询
```
- 只测了 Admin → DB
- 没验证 Gateway 能否读到
- 链路不完整

---

## 自动化 E2E 脚本

### 脚本: `test_admin_gateway_e2e.py`

```bash
# 需要两个服务都运行
Terminal 1: cd admin && uvicorn main:app --port 8001
Terminal 2: ./target/debug/zero_x_infinity --gateway

# 运行 E2E 测试
Terminal 3: ./admin/test_admin_gateway_e2e.py
```

**测试内容**:
- ✅ 4个核心 E2E 场景
- ✅ Admin → Gateway 完整链路
- ✅ 自动验证传播
- ✅ 失败时详细报告

---

## QA 验收标准

### E2E 必须通过的测试

| Test | 描述 | 必须通过 |
|------|------|----------|
| E2E-01 | Asset 创建传播 | ✅ |
| E2E-02 | Symbol 创建传播 | ✅ |
| E2E-03 | Symbol 状态变更 | ✅ |
| E2E-04 | 费率更新传播 | ✅ |

**只有 pytest 通过 ≠ QA 批准**

**必须**: E2E 测试全部通过！

---

## 集成到一键测试

### 完整测试流程

```bash
#!/bin/bash
# admin/run_all_tests.sh

# 1. Unit Tests
pytest tests/ -v

# 2. 启动服务
uvicorn main:app --port 8001 &
ADMIN_PID=$!

# 3. 等待 Gateway (假设已运行)
sleep 2

# 4. E2E Tests
./test_admin_gateway_e2e.py
E2E_RESULT=$?

# 5. 清理
kill $ADMIN_PID

exit $E2E_RESULT
```

---

**QA 铁律**: 没有真正的 E2E 测试 = 不能批准上线
