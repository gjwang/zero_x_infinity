# Admin Dashboard 测试脚本说明

## 测试脚本一览

| 脚本 | 用途 | 需要服务器 |
|------|------|-----------|
| `test_quick.sh` | 快速单元测试 | ❌ |
| `verify_final.sh` | 完整验证 | ✅ |
| `verify_all.sh` | 原始验证脚本 | ✅ |

---

## 1. 快速测试 (推荐日常使用)

```bash
cd admin
./test_quick.sh
```

**测试内容**:
- 177个单元测试
- Status API测试 (UX-08)
- 约7秒完成

---

## 2. 完整验证

```bash
cd admin
./verify_final.sh
```

**测试内容**:
1. 环境设置
2. 单元测试 (177个)
3. Status API测试
4. E2E测试 (需要数据库和服务器)

---

## 3. 测试分类

### 单元测试 (不需要服务器)
```bash
pytest tests/ -m "not e2e" --ignore=tests/e2e -v
```

### Status API测试
```bash
pytest tests/test_ux08_status_strings.py -v
```

### E2E测试 (需要服务器)
```bash
# 先启动服务器
uvicorn main:app --port 8001 &

# 再运行E2E
pytest tests/e2e/ -v
```

---

## 4. 当前测试状态

| 类别 | 通过 | 跳过 | 失败 |
|------|------|------|------|
| 单元测试 | 177 | 32 | 0 |
| E2E测试 | 6+ | 8 | 3* |

*E2E失败是下游集成问题（Gateway未运行等）

---

## 5. Status API 设计

**输入**: 只接受字符串
- Asset: `"ACTIVE"` / `"DISABLED"`
- Symbol: `"ONLINE"` / `"OFFLINE"` / `"CLOSE_ONLY"`

**输出**: 字符串 (通过 `field_serializer`)

**整数输入**: 拒绝，返回错误 `"Status must be a string..."`

---

## 6. 关键测试文件

- `tests/test_ux08_status_strings.py` - Status API验证
- `tests/test_core_flow.py` - 核心流程
- `tests/test_input_validation.py` - 输入验证
- `tests/e2e/test_asset_lifecycle.py` - Asset E2E
- `tests/e2e/test_symbol_lifecycle.py` - Symbol E2E
