#!/usr/bin/env python3
"""
🔥 Agent A (激进派 QA): 边缘测试

测试极端输入、边界值、畸形格式等攻击向量。
测试用例: A-TC-001 ~ A-TC-007 + 交叉审核补充

参考格式: scripts/tests/0x14b_matching/test_ioc_qa.py
"""

import sys
import os

# 路径设置
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
SCRIPTS_ROOT = os.path.dirname(os.path.dirname(SCRIPT_DIR))
sys.path.insert(0, SCRIPTS_ROOT)

from conftest import (
    TestStatus, TestResult, collector,
    GATEWAY_URL, SYMBOL, USER_TAKER,
    get_test_client, place_order, SDK_AVAILABLE,
    ASSET_DECIMALS, health_check
)

try:
    from lib.api_auth import get_test_client, ApiClient
except ImportError:
    print("Error: lib.api_auth not available")
    sys.exit(1)


# =============================================================================
# A-TC-001: 超精度数值攻击 (Precision Overflow)
# =============================================================================

def test_a_tc_001_precision_boundary():
    """A-TC-001: 验证系统拒绝超过资产精度的输入"""
    
    print("\n📦 A-TC-001: 精度边界测试")
    print("-" * 60)
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    # A-TC-001-01: BTC 9位精度被拒绝
    test_id = "A-TC-001-01"
    try:
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "50000.0",
            "qty": "1.123456789",  # 9 位 > BTC 的 8 位
            "time_in_force": "GTC",
        })
        
        if resp.status_code in [400, 422]:
            collector.add(TestResult(test_id, "BTC 9位精度被拒绝", TestStatus.PASS,
                                    expected="400|422", actual=str(resp.status_code)))
        else:
            collector.add(TestResult(test_id, "BTC 9位精度被拒绝", TestStatus.FAIL,
                                    expected="400|422", actual=str(resp.status_code)))
    except Exception as e:
        collector.add(TestResult(test_id, "BTC 9位精度被拒绝", TestStatus.ERROR, str(e)))
    
    # A-TC-001-02: BTC 8位精度接受
    test_id = "A-TC-001-02"
    try:
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "50000.0",
            "qty": "0.12345678",  # 8 位 = BTC 最大
            "time_in_force": "GTC",
        })
        
        # 可能因余额不足失败 (400)，但不应是精度错误
        data = resp.json() if resp.status_code != 200 else {}
        if resp.status_code in [200, 202]:
            collector.add(TestResult(test_id, "BTC 8位精度接受", TestStatus.PASS))
        elif "PRECISION" in str(data):
            collector.add(TestResult(test_id, "BTC 8位精度接受", TestStatus.FAIL,
                                    details="8-decimal precision incorrectly rejected"))
        else:
            collector.add(TestResult(test_id, "BTC 8位精度接受", TestStatus.PASS,
                                    details=f"Rejected for other reason: {resp.status_code}"))
    except Exception as e:
        collector.add(TestResult(test_id, "BTC 8位精度接受", TestStatus.ERROR, str(e)))
    
    # A-TC-001-03: USDT 7位精度被拒绝
    test_id = "A-TC-001-03"
    try:
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "50000.1234567",  # 7 位 > USDT 的 6 位
            "qty": "1.0",
            "time_in_force": "GTC",
        })
        
        if resp.status_code in [400, 422]:
            collector.add(TestResult(test_id, "USDT 7位价格精度被拒绝", TestStatus.PASS))
        else:
            collector.add(TestResult(test_id, "USDT 7位价格精度被拒绝", TestStatus.FAIL,
                                    expected="400|422", actual=str(resp.status_code)))
    except Exception as e:
        collector.add(TestResult(test_id, "USDT 7位价格精度被拒绝", TestStatus.ERROR, str(e)))
    
    # A-TC-001-04: 最小单位值接受
    test_id = "A-TC-001-04"
    try:
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "50000.0",
            "qty": "0.00000001",  # 1 satoshi
            "time_in_force": "GTC",
        })
        
        data = resp.json() if resp.status_code != 200 else {}
        if "ZERO" in str(data) or ("PRECISION" in str(data) and resp.status_code == 400):
            collector.add(TestResult(test_id, "最小单位值接受", TestStatus.FAIL,
                                    details="Minimum unit incorrectly rejected"))
        else:
            collector.add(TestResult(test_id, "最小单位值接受", TestStatus.PASS))
    except Exception as e:
        collector.add(TestResult(test_id, "最小单位值接受", TestStatus.ERROR, str(e)))


# =============================================================================
# A-TC-002: 数值溢出攻击 (Integer Overflow)
# =============================================================================

def test_a_tc_002_integer_overflow():
    """A-TC-002: 验证系统检测并拒绝导致 u64 溢出的输入"""
    
    print("\n📦 A-TC-002: 溢出攻击测试")
    print("-" * 60)
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    overflow_cases = [
        ("A-TC-002-01", "u64::MAX+1 被拒绝", "18446744073709551616"),
        ("A-TC-002-02", "缩放后溢出被拒绝", "184467440737.09551616"),
        ("A-TC-002-03", "超大数值被拒绝", "1" + "0" * 30),
    ]
    
    for test_id, name, qty in overflow_cases:
        try:
            resp = client.post("/api/v1/private/order", {
                "symbol": SYMBOL,
                "side": "BUY",
                "order_type": "LIMIT",
                "price": "50000.0",
                "qty": qty,
                "time_in_force": "GTC",
            })
            
            if resp.status_code in [400, 422]:
                collector.add(TestResult(test_id, name, TestStatus.PASS))
            else:
                collector.add(TestResult(test_id, name, TestStatus.FAIL,
                                        expected="400|422", actual=str(resp.status_code)))
        except Exception as e:
            collector.add(TestResult(test_id, name, TestStatus.ERROR, str(e)))


# =============================================================================
# A-TC-003: 畸形格式攻击 (Malformed Input)
# =============================================================================

def test_a_tc_003_malformed_input():
    """A-TC-003: 验证系统拒绝非标准数字格式"""
    
    print("\n📦 A-TC-003: 畸形格式测试")
    print("-" * 60)
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    malformed_cases = [
        ("A-TC-003-01", "'.5' 格式被拒绝", ".5"),
        ("A-TC-003-02", "'5.' 格式被拒绝", "5."),
        ("A-TC-003-03", "千分位分隔符被拒绝", "1,000.00"),
        ("A-TC-003-04", "科学计数法被拒绝", "1.5e8"),
        ("A-TC-003-05", "空字符串被拒绝", ""),
        ("A-TC-003-06", "NaN 被拒绝", "NaN"),
        ("A-TC-003-07", "Infinity 被拒绝", "Infinity"),
    ]
    
    for test_id, name, qty in malformed_cases:
        try:
            resp = client.post("/api/v1/private/order", {
                "symbol": SYMBOL,
                "side": "BUY",
                "order_type": "LIMIT",
                "price": "50000.0",
                "qty": qty,
                "time_in_force": "GTC",
            })
            
            if resp.status_code in [400, 422]:
                collector.add(TestResult(test_id, name, TestStatus.PASS))
            else:
                collector.add(TestResult(test_id, name, TestStatus.FAIL,
                                        expected="400|422", actual=str(resp.status_code)))
        except Exception as e:
            collector.add(TestResult(test_id, name, TestStatus.ERROR, str(e)))


# =============================================================================
# A-TC-008: JSON 数字格式拒绝 (Breaking Change 验证)
# =============================================================================

def test_a_tc_008_json_number_format():
    """A-TC-008: 验证 JSON 数字格式被拒绝 (必须使用字符串)
    
    Breaking Change: price/qty 必须是字符串
    // ❌ 旧格式 (不再支持): {"price": 85000, "qty": 0.001}
    // ✅ 新格式 (必须使用): {"price": "85000", "qty": "0.001"}
    """
    
    print("\n📦 A-TC-008: JSON 数字格式拒绝测试")
    print("-" * 60)
    
    import json
    import requests
    from lib.api_auth import get_test_client
    
    # 获取认证客户端以复用签名逻辑
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    url = f"{GATEWAY_URL}/api/v1/private/order"
    path = "/api/v1/private/order"
    
    def post_raw_json(raw_json: str) -> requests.Response:
        """发送带认证的原始 JSON 请求"""
        auth = client._sign_request("POST", path, "")
        return requests.post(
            url,
            data=raw_json,
            headers={
                "Content-Type": "application/json",
                "Authorization": auth
            },
            timeout=5
        )
    
    # A-TC-008-01: qty 使用 JSON 数字
    test_id = "A-TC-008-01"
    try:
        payload = json.dumps({
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "85000.00",
            "qty": 0.001,  # JSON 数字，非字符串
            "time_in_force": "GTC",
        })
        
        resp = post_raw_json(payload)
        
        if resp.status_code in [400, 422]:
            collector.add(TestResult(test_id, "qty JSON数字被拒绝", TestStatus.PASS,
                                    details="expected a string"))
        else:
            collector.add(TestResult(test_id, "qty JSON数字被拒绝", TestStatus.FAIL,
                                    expected="400|422", actual=str(resp.status_code)))
    except Exception as e:
        collector.add(TestResult(test_id, "qty JSON数字被拒绝", TestStatus.ERROR, str(e)))
    
    # A-TC-008-02: price 使用 JSON 数字
    test_id = "A-TC-008-02"
    try:
        payload = json.dumps({
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": 85000,  # JSON 数字，非字符串
            "qty": "0.001",
            "time_in_force": "GTC",
        })
        
        resp = post_raw_json(payload)
        
        if resp.status_code in [400, 422]:
            collector.add(TestResult(test_id, "price JSON数字被拒绝", TestStatus.PASS,
                                    details="expected a string"))
        else:
            collector.add(TestResult(test_id, "price JSON数字被拒绝", TestStatus.FAIL,
                                    expected="400|422", actual=str(resp.status_code)))
    except Exception as e:
        collector.add(TestResult(test_id, "price JSON数字被拒绝", TestStatus.ERROR, str(e)))
    
    # A-TC-008-03: 两者都使用 JSON 数字
    test_id = "A-TC-008-03"
    try:
        payload = json.dumps({
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": 85000,   # JSON 数字
            "qty": 0.001,     # JSON 数字
            "time_in_force": "GTC",
        })
        
        resp = post_raw_json(payload)
        
        if resp.status_code in [400, 422]:
            collector.add(TestResult(test_id, "price+qty JSON数字被拒绝", TestStatus.PASS))
        else:
            collector.add(TestResult(test_id, "price+qty JSON数字被拒绝", TestStatus.FAIL,
                                    expected="400|422", actual=str(resp.status_code)))
    except Exception as e:
        collector.add(TestResult(test_id, "price+qty JSON数字被拒绝", TestStatus.ERROR, str(e)))
    
    # A-TC-008-04: 字符串格式正常接受
    test_id = "A-TC-008-04"
    try:
        payload = json.dumps({
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "85000.00",  # 字符串 ✅
            "qty": "0.001",       # 字符串 ✅
            "time_in_force": "GTC",
        })
        
        resp = post_raw_json(payload)
        
        if resp.status_code in [200, 202]:
            collector.add(TestResult(test_id, "字符串格式接受", TestStatus.PASS))
        else:
            # 可能因为余额不足等原因失败，但不应是格式错误
            data = resp.json() if resp.status_code in [400, 422] else {}
            if "string" in str(data).lower():
                collector.add(TestResult(test_id, "字符串格式接受", TestStatus.FAIL,
                                        details="String format incorrectly rejected"))
            else:
                collector.add(TestResult(test_id, "字符串格式接受", TestStatus.PASS,
                                        details=f"Rejected for other reason: {resp.status_code}"))
    except Exception as e:
        collector.add(TestResult(test_id, "字符串格式接受", TestStatus.ERROR, str(e)))


# =============================================================================
# A-TC-004: 零值边界攻击 (Zero Value)
# =============================================================================

def test_a_tc_004_zero_value():
    """A-TC-004: 验证零值在不同上下文中的处理"""
    
    print("\n📦 A-TC-004: 零值边界测试")
    print("-" * 60)
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    zero_cases = [
        ("A-TC-004-01", "零数量被拒绝", "0", "50000.0"),
        ("A-TC-004-02", "零价格被拒绝", "1.0", "0"),
        ("A-TC-004-03", "全零小数被拒绝", "0.00000000", "50000.0"),
        ("A-TC-004-04", "负零被拒绝", "-0", "50000.0"),
    ]
    
    for test_id, name, qty, price in zero_cases:
        try:
            resp = client.post("/api/v1/private/order", {
                "symbol": SYMBOL,
                "side": "BUY",
                "order_type": "LIMIT",
                "price": price,
                "qty": qty,
                "time_in_force": "GTC",
            })
            
            if resp.status_code in [400, 422]:
                collector.add(TestResult(test_id, name, TestStatus.PASS))
            else:
                collector.add(TestResult(test_id, name, TestStatus.FAIL,
                                        expected="400|422", actual=str(resp.status_code)))
        except Exception as e:
            collector.add(TestResult(test_id, name, TestStatus.ERROR, str(e)))


# =============================================================================
# A-TC-005 ~ A-TC-007: 其他边缘测试
# =============================================================================

def test_a_tc_005_cross_asset_precision():
    """A-TC-005: 跨资产精度隔离"""
    print("\n📦 A-TC-005: 跨资产精度测试")
    print("-" * 60)
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    test_id = "A-TC-005-01"
    try:
        # BTCUSDT: BTC(8位) / USDT(6位) 独立验证
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "50000.1234567",  # 7位 > USDT 6位
            "qty": "1.0",
            "time_in_force": "GTC",
        })
        
        if resp.status_code == 400:
            collector.add(TestResult(test_id, "Quote 精度独立验证", TestStatus.PASS))
        else:
            collector.add(TestResult(test_id, "Quote 精度独立验证", TestStatus.FAIL,
                                    expected="400", actual=str(resp.status_code)))
    except Exception as e:
        collector.add(TestResult(test_id, "Quote 精度独立验证", TestStatus.ERROR, str(e)))


def test_a_tc_006_display_precision():
    """A-TC-006: 显示精度 vs 存储精度"""
    print("\n📦 A-TC-006: 显示精度测试")
    print("-" * 60)
    
    # 此测试需要成功下单后查询，暂时标记为 SKIP
    collector.add(TestResult("A-TC-006-01", "显示截断不丢失精度", TestStatus.SKIP,
                            details="需要成功下单后验证"))


def test_a_tc_007_audit_script():
    """A-TC-007: 审计脚本验证"""
    print("\n📦 A-TC-007: 审计脚本测试")
    print("-" * 60)
    
    script_path = os.path.join(SCRIPTS_ROOT, "audit_money_safety.sh")
    
    if os.path.exists(script_path):
        collector.add(TestResult("A-TC-007-01", "审计脚本存在", TestStatus.PASS))
    else:
        collector.add(TestResult("A-TC-007-01", "审计脚本存在", TestStatus.FAIL,
                                details=f"Script not found: {script_path}"))


# =============================================================================
# 交叉审核补充: A→C-ADD
# =============================================================================

def test_a_c_add_negative_bypass():
    """A→C-ADD-004: 负数绕过攻击防护"""
    print("\n📦 A→C 交叉审核补充")
    print("-" * 60)
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    negative_cases = [
        ("A-C-ADD-004-01", "负数被拒绝", "-1.0"),
        ("A-C-ADD-004-02", "双负号被拒绝", "--1.0"),
        ("A-C-ADD-004-03", "负小数被拒绝", "-0.00000001"),
    ]
    
    for test_id, name, qty in negative_cases:
        try:
            resp = client.post("/api/v1/private/order", {
                "symbol": SYMBOL,
                "side": "BUY",
                "order_type": "LIMIT",
                "price": "50000.0",
                "qty": qty,
                "time_in_force": "GTC",
            })
            
            if resp.status_code in [400, 422]:
                collector.add(TestResult(test_id, name, TestStatus.PASS))
            else:
                collector.add(TestResult(test_id, name, TestStatus.FAIL,
                                        expected="400|422", actual=str(resp.status_code)))
        except Exception as e:
            collector.add(TestResult(test_id, name, TestStatus.ERROR, str(e)))


# =============================================================================
# 主执行入口
# =============================================================================

def run_all_agent_a_tests():
    """运行所有 Agent A 测试"""
    print("=" * 80)
    print("🔥 Agent A (激进派 QA): 边缘测试")
    print("=" * 80)
    
    # 健康检查
    if not health_check():
        print("❌ Gateway not available!")
        return 1
    
    test_a_tc_001_precision_boundary()
    test_a_tc_002_integer_overflow()
    test_a_tc_003_malformed_input()
    test_a_tc_008_json_number_format()  # Breaking change: JSON number → string
    test_a_tc_004_zero_value()
    test_a_tc_005_cross_asset_precision()
    test_a_tc_006_display_precision()
    test_a_tc_007_audit_script()
    test_a_c_add_negative_bypass()
    
    collector.print_summary()
    
    return 0 if collector.all_passed else 1


if __name__ == "__main__":
    sys.exit(run_all_agent_a_tests())
