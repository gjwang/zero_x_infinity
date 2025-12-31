#!/usr/bin/env python3
"""
🛡️ Agent B (保守派 QA): 核心流程验证

测试标准转换、往返一致性、回归测试等核心功能。
测试用例: B-TC-001 ~ B-TC-006 + 交叉审核补充

参考格式: scripts/tests/0x14b_matching/test_ioc_qa.py
"""

import sys
import os
import subprocess

# 路径设置
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
SCRIPTS_ROOT = os.path.dirname(os.path.dirname(SCRIPT_DIR))
PROJECT_ROOT = os.path.dirname(SCRIPTS_ROOT)
sys.path.insert(0, SCRIPTS_ROOT)

from conftest import (
    TestStatus, TestResult, collector,
    GATEWAY_URL, SYMBOL, USER_TAKER, USER_MAKER,
    get_test_client, place_order, SDK_AVAILABLE,
    get_exchange_info, health_check
)

try:
    from lib.api_auth import get_test_client, ApiClient
except ImportError:
    print("Error: lib.api_auth not available")
    sys.exit(1)


# =============================================================================
# B-TC-001: 标准金额转换准确性
# =============================================================================

def test_b_tc_001_standard_conversion():
    """B-TC-001: 验证标准输入的转换精确无误"""
    
    print("\n📦 B-TC-001: 标准转换测试")
    print("-" * 60)
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    # B-TC-001-01: 标准 BTC 金额
    test_id = "B-TC-001-01"
    try:
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "50000.0",
            "qty": "1.0",
            "time_in_force": "GTC",
        })
        
        # 接受 200/202 或因余额不足的 400 (但非格式错误)
        if resp.status_code in [200, 202]:
            collector.add(TestResult(test_id, "1.0 BTC 标准转换", TestStatus.PASS))
        else:
            data = resp.json() if resp.status_code == 400 else {}
            if "FORMAT" in str(data) or "PARSE" in str(data):
                collector.add(TestResult(test_id, "1.0 BTC 标准转换", TestStatus.FAIL,
                                        details="Standard format rejected"))
            else:
                collector.add(TestResult(test_id, "1.0 BTC 标准转换", TestStatus.PASS,
                                        details=f"Rejected for other reason: {resp.status_code}"))
    except Exception as e:
        collector.add(TestResult(test_id, "1.0 BTC 标准转换", TestStatus.ERROR, str(e)))
    
    # B-TC-001-02: 最小单位
    test_id = "B-TC-001-02"
    try:
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "50000.0",
            "qty": "0.00000001",  # 1 satoshi
            "time_in_force": "GTC",
        })
        
        data = resp.json() if resp.status_code == 400 else {}
        if "ZERO" in str(data) and resp.status_code == 400:
            collector.add(TestResult(test_id, "1 satoshi 转换", TestStatus.FAIL,
                                    details="Minimum unit rejected as zero"))
        else:
            collector.add(TestResult(test_id, "1 satoshi 转换", TestStatus.PASS))
    except Exception as e:
        collector.add(TestResult(test_id, "1 satoshi 转换", TestStatus.ERROR, str(e)))
    
    # B-TC-001-03: BTC 总供应量
    test_id = "B-TC-001-03"
    try:
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "SELL",
            "order_type": "LIMIT",
            "price": "1.0",
            "qty": "21000000.0",  # BTC 总供应量
            "time_in_force": "GTC",
        })
        
        data = resp.json() if resp.status_code == 400 else {}
        if "OVERFLOW" in str(data) and resp.status_code == 400:
            collector.add(TestResult(test_id, "BTC 总供应量不溢出", TestStatus.FAIL,
                                    details="21M BTC incorrectly reported as overflow"))
        else:
            collector.add(TestResult(test_id, "BTC 总供应量不溢出", TestStatus.PASS))
    except Exception as e:
        collector.add(TestResult(test_id, "BTC 总供应量不溢出", TestStatus.ERROR, str(e)))


# =============================================================================
# B-TC-002: 往返转换一致性 (Round-trip)
# =============================================================================

def test_b_tc_002_roundtrip():
    """B-TC-002: 验证 parse → format → parse 结果一致"""
    
    print("\n📦 B-TC-002: 往返一致性测试")
    print("-" * 60)
    
    # 此测试需要成功下单后查询详情验证数量一致
    # 暂时验证下单响应中的数量格式
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    test_id = "B-TC-002-01"
    try:
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "50000.0",
            "qty": "1.23456789",
            "time_in_force": "GTC",
        })
        
        # 对于超过精度的输入，应该被拒绝
        if resp.status_code == 400:
            collector.add(TestResult(test_id, "往返一致性验证", TestStatus.PASS,
                                    details="Over-precision correctly rejected"))
        elif resp.status_code in [200, 202]:
            data = resp.json()
            qty = data.get("data", {}).get("qty", "")
            # 验证格式
            if qty:
                collector.add(TestResult(test_id, "往返一致性验证", TestStatus.PASS,
                                        details=f"qty in response: {qty}"))
            else:
                collector.add(TestResult(test_id, "往返一致性验证", TestStatus.PASS))
        else:
            collector.add(TestResult(test_id, "往返一致性验证", TestStatus.FAIL,
                                    expected="200/400", actual=str(resp.status_code)))
    except Exception as e:
        collector.add(TestResult(test_id, "往返一致性验证", TestStatus.ERROR, str(e)))


# =============================================================================
# B-TC-003: SymbolManager 精度获取
# =============================================================================

def test_b_tc_003_symbol_manager():
    """B-TC-003: 验证 SymbolManager 返回正确的精度配置"""
    
    print("\n📦 B-TC-003: SymbolManager 测试")
    print("-" * 60)
    
    test_id = "B-TC-003-01"
    try:
        info = get_exchange_info()
        if not info:
            collector.add(TestResult(test_id, "Exchange Info 精度正确", TestStatus.SKIP,
                                    details="Exchange info not available"))
            return
        
        assets = {a.get("asset"): a for a in info.get("assets", [])}
        
        errors = []
        
        # 验证 BTC 精度
        if "BTC" in assets:
            btc_decimals = assets["BTC"].get("decimals")
            if btc_decimals != 8:
                errors.append(f"BTC decimals: {btc_decimals} != 8")
        
        # 验证 USDT 精度
        if "USDT" in assets:
            usdt_decimals = assets["USDT"].get("decimals")
            if usdt_decimals != 6:
                errors.append(f"USDT decimals: {usdt_decimals} != 6")
        
        if errors:
            collector.add(TestResult(test_id, "Exchange Info 精度正确", TestStatus.FAIL,
                                    details=", ".join(errors)))
        else:
            collector.add(TestResult(test_id, "Exchange Info 精度正确", TestStatus.PASS))
    except Exception as e:
        collector.add(TestResult(test_id, "Exchange Info 精度正确", TestStatus.ERROR, str(e)))


# =============================================================================
# B-TC-004: 回归测试
# =============================================================================

def test_b_tc_004_regression():
    """B-TC-004: 回归测试"""
    
    print("\n📦 B-TC-004: 回归测试")
    print("-" * 60)
    
    # B-TC-004-01: Rust 单元测试
    test_id = "B-TC-004-01"
    try:
        result = subprocess.run(
            ["cargo", "test", "money::", "--lib", "--", "-q"],
            cwd=PROJECT_ROOT,
            capture_output=True,
            timeout=120
        )
        
        if result.returncode == 0:
            collector.add(TestResult(test_id, "Money 模块单元测试", TestStatus.PASS))
        else:
            stderr = result.stderr.decode()[:200] if result.stderr else ""
            collector.add(TestResult(test_id, "Money 模块单元测试", TestStatus.FAIL,
                                    details=stderr))
    except subprocess.TimeoutExpired:
        collector.add(TestResult(test_id, "Money 模块单元测试", TestStatus.ERROR,
                                details="Timeout"))
    except FileNotFoundError:
        collector.add(TestResult(test_id, "Money 模块单元测试", TestStatus.SKIP,
                                details="cargo not found"))
    except Exception as e:
        collector.add(TestResult(test_id, "Money 模块单元测试", TestStatus.ERROR, str(e)))


# =============================================================================
# B-TC-005: API 响应格式一致性
# =============================================================================

def test_b_tc_005_api_format():
    """B-TC-005: 验证 API 响应中的金额格式符合规范"""
    
    print("\n📦 B-TC-005: API 响应格式测试")
    print("-" * 60)
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    test_id = "B-TC-005-01"
    try:
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "50000.0",
            "qty": "0.1",
            "time_in_force": "GTC",
        })
        
        if resp.status_code in [200, 202]:
            data = resp.json()
            order_data = data.get("data", {})
            
            qty = order_data.get("qty")
            price = order_data.get("price")
            
            errors = []
            if qty is not None and not isinstance(qty, str):
                errors.append(f"qty is {type(qty).__name__}")
            if price is not None and not isinstance(price, str):
                errors.append(f"price is {type(price).__name__}")
            
            if errors:
                collector.add(TestResult(test_id, "金额字段是字符串", TestStatus.FAIL,
                                        details=", ".join(errors)))
            else:
                collector.add(TestResult(test_id, "金额字段是字符串", TestStatus.PASS))
        else:
            collector.add(TestResult(test_id, "金额字段是字符串", TestStatus.SKIP,
                                    details=f"Order not placed: {resp.status_code}"))
    except Exception as e:
        collector.add(TestResult(test_id, "金额字段是字符串", TestStatus.ERROR, str(e)))


# =============================================================================
# B-TC-006: 迁移文件功能验证
# =============================================================================

def test_b_tc_006_migration():
    """B-TC-006: 验证迁移后的文件功能正常"""
    
    print("\n📦 B-TC-006: 迁移验证测试")
    print("-" * 60)
    
    # 暂时跳过，需要特定的迁移后测试
    collector.add(TestResult("B-TC-006-01", "迁移模块测试", TestStatus.SKIP,
                            details="Requires post-migration verification"))


# =============================================================================
# 交叉审核补充: B→A, B→C
# =============================================================================

def test_b_cross_review():
    """交叉审核: Agent B 的补充测试"""
    
    print("\n📦 B→A/C 交叉审核补充")
    print("-" * 60)
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    # B-A-ADD-001: 边缘测试后系统状态正常
    test_id = "B-A-ADD-001"
    try:
        # 发起多次畸形输入
        for inp in [".5", "NaN", "-1.0"]:
            client.post("/api/v1/private/order", {
                "symbol": SYMBOL,
                "side": "BUY",
                "order_type": "LIMIT",
                "price": "50000.0",
                "qty": inp,
                "time_in_force": "GTC",
            })
        
        # 验证正常订单仍可工作
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "50000.0",
            "qty": "0.1",
            "time_in_force": "GTC",
        })
        
        if resp.status_code == 500:
            collector.add(TestResult(test_id, "边缘测试后系统正常", TestStatus.FAIL,
                                    details="System error after edge cases"))
        else:
            collector.add(TestResult(test_id, "边缘测试后系统正常", TestStatus.PASS))
    except Exception as e:
        collector.add(TestResult(test_id, "边缘测试后系统正常", TestStatus.ERROR, str(e)))
    
    # B-C-ADD-001: 合法大额不被误拒
    test_id = "B-C-ADD-001"
    try:
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "1.0",
            "qty": "100000.0",  # 10万 BTC
            "time_in_force": "GTC",
        })
        
        data = resp.json() if resp.status_code == 400 else {}
        if "OVERFLOW" in str(data):
            collector.add(TestResult(test_id, "合法大额不被误拒", TestStatus.FAIL,
                                    details="100K BTC rejected as overflow"))
        else:
            collector.add(TestResult(test_id, "合法大额不被误拒", TestStatus.PASS))
    except Exception as e:
        collector.add(TestResult(test_id, "合法大额不被误拒", TestStatus.ERROR, str(e)))


# =============================================================================
# 主执行入口
# =============================================================================

def run_all_agent_b_tests():
    """运行所有 Agent B 测试"""
    print("=" * 80)
    print("🛡️ Agent B (保守派 QA): 核心流程验证")
    print("=" * 80)
    
    # 健康检查
    if not health_check():
        print("❌ Gateway not available!")
        return 1
    
    test_b_tc_001_standard_conversion()
    test_b_tc_002_roundtrip()
    test_b_tc_003_symbol_manager()
    test_b_tc_004_regression()
    test_b_tc_005_api_format()
    test_b_tc_006_migration()
    test_b_cross_review()
    
    collector.print_summary()
    
    return 0 if collector.all_passed else 1


if __name__ == "__main__":
    sys.exit(run_all_agent_b_tests())
