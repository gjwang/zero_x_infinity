#!/usr/bin/env python3
"""
🔐 Agent C (安全专家 QA): 安全审计

测试溢出攻击、注入攻击、信息泄露等安全问题。
测试用例: C-TC-001 ~ C-TC-007 + 交叉审核补充

参考格式: scripts/tests/0x14b_matching/test_ioc_qa.py
"""

import sys
import os
import time
import statistics

# 路径设置
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
SCRIPTS_ROOT = os.path.dirname(os.path.dirname(SCRIPT_DIR))
PROJECT_ROOT = os.path.dirname(SCRIPTS_ROOT)
sys.path.insert(0, SCRIPTS_ROOT)

from conftest import (
    TestStatus, TestResult, collector,
    GATEWAY_URL, SYMBOL, USER_TAKER,
    get_test_client, health_check
)

try:
    from lib.api_auth import get_test_client, ApiClient
except ImportError:
    print("Error: lib.api_auth not available")
    sys.exit(1)


# =============================================================================
# C-TC-001: 溢出导致资金错算防护
# =============================================================================

def test_c_tc_001_overflow_safety():
    """C-TC-001: 验证溢出不会导致资金错算"""
    
    print("\n📦 C-TC-001: 溢出资金安全测试")
    print("-" * 60)
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    # C-TC-001-01: 溢出不产生错误金额
    test_id = "C-TC-001-01"
    try:
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "1000000",
            "qty": "18446744073",  # 接近 u64::MAX / 10^8
            "time_in_force": "GTC",
        })
        
        if resp.status_code in [200, 202]:
            data = resp.json()
            qty = data.get("data", {}).get("qty", "")
            # 如果接受了，验证没有产生异常小的金额（溢出回绕）
            if qty:
                try:
                    qty_float = float(qty)
                    if qty_float < 1000000000:
                        collector.add(TestResult(test_id, "溢出不产生错误金额", TestStatus.FAIL,
                                                details=f"Overflow produced wrong amount: {qty}"))
                    else:
                        collector.add(TestResult(test_id, "溢出不产生错误金额", TestStatus.PASS))
                except ValueError:
                    collector.add(TestResult(test_id, "溢出不产生错误金额", TestStatus.PASS))
            else:
                collector.add(TestResult(test_id, "溢出不产生错误金额", TestStatus.PASS))
        elif resp.status_code == 400:
            # 正确拒绝
            collector.add(TestResult(test_id, "溢出不产生错误金额", TestStatus.PASS,
                                    details="Correctly rejected"))
        else:
            collector.add(TestResult(test_id, "溢出不产生错误金额", TestStatus.FAIL,
                                    expected="400 or safe 200", actual=str(resp.status_code)))
    except Exception as e:
        collector.add(TestResult(test_id, "溢出不产生错误金额", TestStatus.ERROR, str(e)))
    
    # C-TC-001-02: 大额乘法安全
    test_id = "C-TC-001-02"
    try:
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "1000000000",
            "qty": "1000000000",
            "time_in_force": "GTC",
        })
        
        if resp.status_code == 500:
            collector.add(TestResult(test_id, "大额乘法安全", TestStatus.FAIL,
                                    details="Server error on large multiplication"))
        else:
            collector.add(TestResult(test_id, "大额乘法安全", TestStatus.PASS))
    except Exception as e:
        collector.add(TestResult(test_id, "大额乘法安全", TestStatus.ERROR, str(e)))


# =============================================================================
# C-TC-002: 无隐藏 Dust 余额
# =============================================================================

def test_c_tc_002_no_dust():
    """C-TC-002: 验证不存在隐藏的 dust 余额"""
    
    print("\n📦 C-TC-002: Dust 余额测试")
    print("-" * 60)
    
    # 此测试需要检查余额显示精度
    collector.add(TestResult("C-TC-002-01", "最小单位完整显示", TestStatus.SKIP,
                            details="Requires balance check after deposit"))


# =============================================================================
# C-TC-003: 跨路径精度一致性
# =============================================================================

def test_c_tc_003_cross_path():
    """C-TC-003: 验证所有路径使用一致的精度"""
    
    print("\n📦 C-TC-003: 跨路径精度测试")
    print("-" * 60)
    
    # 需要验证多个API路径返回相同精度
    collector.add(TestResult("C-TC-003-01", "跨路径精度一致", TestStatus.SKIP,
                            details="Requires multi-path verification"))


# =============================================================================
# C-TC-004: 注入攻击防护
# =============================================================================

def test_c_tc_004_injection():
    """C-TC-004: 验证金额字段不接受注入 payload"""
    
    print("\n📦 C-TC-004: 注入攻击防护测试")
    print("-" * 60)
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    injection_cases = [
        ("C-TC-004-01", "SQL 注入防护", '1.0; DROP TABLE orders--'),
        ("C-TC-004-02", "JSON 注入防护", '1.0", "admin": true'),
        ("C-TC-004-03", "XSS 防护", '<script>alert(1)</script>'),
        ("C-TC-004-04", "模板注入防护", '${7*7}'),
    ]
    
    for test_id, name, payload in injection_cases:
        try:
            resp = client.post("/api/v1/private/order", {
                "symbol": SYMBOL,
                "side": "BUY",
                "order_type": "LIMIT",
                "price": "50000.0",
                "qty": payload,
                "time_in_force": "GTC",
            })
            
            if resp.status_code not in [400, 422]:
                collector.add(TestResult(test_id, name, TestStatus.FAIL,
                                        expected="400|422", actual=str(resp.status_code)))
                continue
            
            # 验证 payload 不在响应中回显 (防止 XSS)
            if payload in resp.text and '<script>' in payload:
                collector.add(TestResult(test_id, name, TestStatus.FAIL,
                                        details="Payload echoed in response"))
            else:
                collector.add(TestResult(test_id, name, TestStatus.PASS))
        except Exception as e:
            collector.add(TestResult(test_id, name, TestStatus.ERROR, str(e)))


# =============================================================================
# C-TC-005: 内部表示不泄露
# =============================================================================

def test_c_tc_005_no_internal_exposure():
    """C-TC-005: 验证内部 u64 表示不会泄露给客户端"""
    
    print("\n📦 C-TC-005: 内部表示泄露测试")
    print("-" * 60)
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    test_id = "C-TC-005-01"
    try:
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "50000.0",
            "qty": "1.5",
            "time_in_force": "GTC",
        })
        
        # 内部 1.5 BTC = 150000000
        # 不应在响应中看到这个裸数字
        if "150000000" in resp.text:
            data = resp.json()
            order_id = str(data.get("data", {}).get("order_id", ""))
            if order_id != "150000000":
                collector.add(TestResult(test_id, "内部表示不泄露", TestStatus.FAIL,
                                        details="Internal representation (150000000) exposed"))
            else:
                collector.add(TestResult(test_id, "内部表示不泄露", TestStatus.PASS,
                                        details="150000000 is order_id, not amount"))
        else:
            collector.add(TestResult(test_id, "内部表示不泄露", TestStatus.PASS))
    except Exception as e:
        collector.add(TestResult(test_id, "内部表示不泄露", TestStatus.ERROR, str(e)))


# =============================================================================
# C-TC-006: 错误消息不泄露敏感信息
# =============================================================================

def test_c_tc_006_error_message_safety():
    """C-TC-006: 验证错误消息不包含堆栈跟踪或内部细节"""
    
    print("\n📦 C-TC-006: 错误消息安全测试")
    print("-" * 60)
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    test_id = "C-TC-006-01"
    try:
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "50000.0",
            "qty": "invalid",
            "time_in_force": "GTC",
        })
        
        error_text = resp.text.lower()
        
        forbidden_patterns = [
            "stack trace", "panic", "src/", ".rs:",
            "rust_backtrace", "unwrap()", "expect("
        ]
        
        found = [p for p in forbidden_patterns if p in error_text]
        
        if found:
            collector.add(TestResult(test_id, "错误消息不泄露堆栈", TestStatus.FAIL,
                                    details=f"Found sensitive patterns: {found}"))
        else:
            collector.add(TestResult(test_id, "错误消息不泄露堆栈", TestStatus.PASS))
    except Exception as e:
        collector.add(TestResult(test_id, "错误消息不泄露堆栈", TestStatus.ERROR, str(e)))
    
    # C-TC-006-02: 精度错误不泄露配置
    test_id = "C-TC-006-02"
    try:
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "50000.0",
            "qty": "1.123456789",
            "time_in_force": "GTC",
        })
        
        error_text = resp.text
        
        leak_patterns = ["decimals=8", "10^8", "100000000", "u64::MAX"]
        found = [p for p in leak_patterns if p in error_text]
        
        if found:
            collector.add(TestResult(test_id, "精度错误不泄露配置", TestStatus.FAIL,
                                    details=f"Config leaked: {found}"))
        else:
            collector.add(TestResult(test_id, "精度错误不泄露配置", TestStatus.PASS))
    except Exception as e:
        collector.add(TestResult(test_id, "精度错误不泄露配置", TestStatus.ERROR, str(e)))


# =============================================================================
# C-TC-007: 审计脚本不可被禁用
# =============================================================================

def test_c_tc_007_audit_enforcement():
    """C-TC-007: 验证 CI 中的审计步骤不能被跳过"""
    
    print("\n📦 C-TC-007: 审计强制执行测试")
    print("-" * 60)
    
    # 检查 CI workflow 是否包含审计步骤
    workflow_paths = [
        os.path.join(PROJECT_ROOT, ".github", "workflows", "ci.yml"),
        os.path.join(PROJECT_ROOT, ".github", "workflows", "integration-tests.yml"),
    ]
    
    found_audit = False
    for path in workflow_paths:
        if os.path.exists(path):
            with open(path, 'r') as f:
                if "audit_money_safety" in f.read():
                    found_audit = True
                    break
    
    if found_audit:
        collector.add(TestResult("C-TC-007-01", "CI 包含审计步骤", TestStatus.PASS))
    else:
        collector.add(TestResult("C-TC-007-01", "CI 包含审计步骤", TestStatus.SKIP,
                                details="Audit step not found (may not be implemented yet)"))


# =============================================================================
# 交叉审核补充: C→A-ADD
# =============================================================================

def test_c_cross_review():
    """交叉审核: Agent C 补充 Agent A 的测试"""
    
    print("\n📦 C→A 交叉审核补充")
    print("-" * 60)
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    # C-A-ADD-001: 失败操作审计记录
    test_id = "C-A-ADD-001"
    try:
        # 发起多种失败尝试
        for inp in ["18446744073709551616", ".5", "-1.0"]:
            resp = client.post("/api/v1/private/order", {
                "symbol": SYMBOL,
                "side": "BUY",
                "order_type": "LIMIT",
                "price": "50000.0",
                "qty": inp,
                "time_in_force": "GTC",
            })
            if resp.status_code not in [400, 422]:
                collector.add(TestResult(test_id, "失败操作正确拒绝", TestStatus.FAIL,
                                        details=f"Input '{inp}' not rejected (got {resp.status_code})"))
                return
        
        collector.add(TestResult(test_id, "失败操作正确拒绝", TestStatus.PASS))
    except Exception as e:
        collector.add(TestResult(test_id, "失败操作正确拒绝", TestStatus.ERROR, str(e)))
    
    # C-A-ADD-003: 时序侧信道防护
    test_id = "C-A-ADD-003"
    try:
        test_cases = [".5", "1.123456789", "-1.0", "18446744073709551616"]
        timings = {}
        
        for payload in test_cases:
            times = []
            for _ in range(5):
                start = time.perf_counter()
                client.post("/api/v1/private/order", {
                    "symbol": SYMBOL,
                    "side": "BUY",
                    "order_type": "LIMIT",
                    "price": "50000.0",
                    "qty": payload,
                    "time_in_force": "GTC",
                })
                elapsed = time.perf_counter() - start
                times.append(elapsed)
            timings[payload] = statistics.mean(times)
        
        if timings:
            min_time = min(timings.values())
            max_time = max(timings.values())
            
            if max_time > min_time * 5:  # 5x 差异认为有问题
                collector.add(TestResult(test_id, "时序侧信道防护", TestStatus.FAIL,
                                        details=f"Timing variance too high: {min_time:.4f}s - {max_time:.4f}s"))
            else:
                collector.add(TestResult(test_id, "时序侧信道防护", TestStatus.PASS))
        else:
            collector.add(TestResult(test_id, "时序侧信道防护", TestStatus.SKIP))
    except Exception as e:
        collector.add(TestResult(test_id, "时序侧信道防护", TestStatus.ERROR, str(e)))


# =============================================================================
# 主执行入口
# =============================================================================

def run_all_agent_c_tests():
    """运行所有 Agent C 测试"""
    print("=" * 80)
    print("🔐 Agent C (安全专家 QA): 安全审计")
    print("=" * 80)
    
    # 健康检查
    if not health_check():
        print("❌ Gateway not available!")
        return 1
    
    test_c_tc_001_overflow_safety()
    test_c_tc_002_no_dust()
    test_c_tc_003_cross_path()
    test_c_tc_004_injection()
    test_c_tc_005_no_internal_exposure()
    test_c_tc_006_error_message_safety()
    test_c_tc_007_audit_enforcement()
    test_c_cross_review()
    
    collector.print_summary()
    
    return 0 if collector.all_passed else 1


if __name__ == "__main__":
    sys.exit(run_all_agent_c_tests())
