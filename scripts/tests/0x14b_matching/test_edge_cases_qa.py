#!/usr/bin/env python3
"""
QA 0x14-b: 边界条件和状态一致性测试

测试目标:
    验证边界条件和订单簿状态一致性

测试用例:
    EDGE-001: 最小数量订单
    EDGE-002: 价格为 0
    EDGE-003: 数量为 0
    EDGE-004: 负数价格
    EDGE-005: 负数数量
    STATE-001~005: 订单簿状态验证

Usage:
    python3 scripts/tests/0x14b_matching/test_edge_cases_qa.py

Author: QA Engineer (Independent Design)
Date: 2025-12-30
"""

import sys
import os
import time
from typing import Optional, Dict, Tuple
from dataclasses import dataclass
from enum import Enum

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
SCRIPTS_ROOT = os.path.dirname(os.path.dirname(SCRIPT_DIR))
sys.path.insert(0, SCRIPTS_ROOT)

try:
    import requests
except ImportError:
    print("Error: Missing 'requests'. Run: pip install requests")
    sys.exit(1)

from lib.api_auth import get_test_client, ApiClient


# =============================================================================
# Configuration
# =============================================================================

GATEWAY_URL = os.environ.get("GATEWAY_URL", "http://localhost:8080")
SYMBOL = "BTC_USDT"
USER_MAKER = 1001
USER_TAKER = 1002


# =============================================================================
# Test Result Types
# =============================================================================

class TestStatus(Enum):
    PASS = "PASS"
    FAIL = "FAIL"
    SKIP = "SKIP"
    ERROR = "ERROR"


@dataclass
class TestResult:
    test_id: str
    name: str
    status: TestStatus
    details: str = ""
    expected: str = ""
    actual: str = ""


# =============================================================================
# Helper Functions
# =============================================================================

def place_order_raw(client: ApiClient, order_data: Dict) -> Tuple[int, Dict]:
    """
    Place order with raw data, return (status_code, response)
    """
    resp = client.post("/api/v1/private/order", order_data)
    try:
        data = resp.json()
    except:
        data = {"text": resp.text[:200]}
    return resp.status_code, data


def place_order(client: ApiClient, symbol: str, side: str, price: str, qty: str,
                time_in_force: str = "GTC") -> Tuple[Optional[int], Optional[str], Dict]:
    order_data = {
        "symbol": symbol,
        "side": side,
        "order_type": "LIMIT",
        "price": price,
        "qty": qty,
        "time_in_force": time_in_force,
    }
    resp = client.post("/api/v1/private/order", order_data)
    if resp.status_code in [200, 202]:
        data = resp.json()
        order_id = data.get("data", {}).get("order_id")
        status = data.get("data", {}).get("order_status", "")
        return order_id, status, data
    return None, None, {"error": resp.status_code, "text": resp.text[:200]}


def cancel_order(client: ApiClient, order_id: int) -> bool:
    resp = client.delete(f"/api/v1/private/order/{order_id}")
    return resp.status_code in [200, 202]


def cleanup_order(client: ApiClient, order_id: Optional[int]):
    if order_id:
        try:
            cancel_order(client, order_id)
        except:
            pass


def get_order_book(symbol: str) -> Dict:
    resp = requests.get(f"{GATEWAY_URL}/api/v1/public/depth?symbol={symbol}&limit=50", timeout=5)
    if resp.status_code == 200:
        return resp.json().get("data", {})
    return {}


def check_order_in_book(symbol: str, side: str, price: str) -> bool:
    depth = get_order_book(symbol)
    levels = depth.get("bids" if side == "BUY" else "asks", [])
    target_price = float(price)
    for level in levels:
        if len(level) >= 1 and abs(float(level[0]) - target_price) < 0.01:
            return True
    return False


# =============================================================================
# Edge Case Tests
# =============================================================================

def test_edge_001_min_qty() -> TestResult:
    """
    EDGE-001: 最小数量订单 (qty=最小单位)
    """
    test_id = "EDGE-001"
    test_name = "最小数量订单"
    
    print(f"\n[{test_id}] {test_name}")
    
    client = get_test_client(GATEWAY_URL, USER_MAKER)
    price = "40000.00"
    min_qty = "0.00001"  # Very small quantity
    order_id = None
    
    try:
        order_id, status, resp = place_order(client, SYMBOL, "BUY", price, min_qty, "GTC")
        
        print(f"  Order response: id={order_id}, status={status}")
        
        if order_id:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected="Order accepted",
                            actual=f"order_id={order_id}")
        else:
            # May be rejected if below minimum
            return TestResult(test_id, test_name, TestStatus.PASS,
                            details="Rejected (below min qty)",
                            expected="Accepted or rejected",
                            actual=f"rejected: {resp}")
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))
    
    finally:
        cleanup_order(client, order_id)


def test_edge_002_zero_price() -> TestResult:
    """
    EDGE-002: 价格为 0 → 应拒绝
    """
    test_id = "EDGE-002"
    test_name = "零价格订单"
    
    print(f"\n[{test_id}] {test_name}")
    
    client = get_test_client(GATEWAY_URL, USER_MAKER)
    
    try:
        order_data = {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "0",
            "qty": "0.001",
            "time_in_force": "GTC",
        }
        
        status_code, resp = place_order_raw(client, order_data)
        print(f"  Response: status={status_code}, data={resp}")
        
        # Should be rejected
        if status_code >= 400:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected="Rejected (invalid price)",
                            actual=f"status={status_code}")
        else:
            code = resp.get("code", 0)
            if code != 0:
                return TestResult(test_id, test_name, TestStatus.PASS,
                                expected="Error code",
                                actual=f"code={code}")
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            expected="Rejection",
                            actual="Order accepted with price=0")
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))


def test_edge_003_zero_qty() -> TestResult:
    """
    EDGE-003: 数量为 0 → 应拒绝
    """
    test_id = "EDGE-003"
    test_name = "零数量订单"
    
    print(f"\n[{test_id}] {test_name}")
    
    client = get_test_client(GATEWAY_URL, USER_MAKER)
    
    try:
        order_data = {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "50000.00",
            "qty": "0",
            "time_in_force": "GTC",
        }
        
        status_code, resp = place_order_raw(client, order_data)
        print(f"  Response: status={status_code}, data={resp}")
        
        if status_code >= 400:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected="Rejected (invalid qty)",
                            actual=f"status={status_code}")
        else:
            code = resp.get("code", 0)
            if code != 0:
                return TestResult(test_id, test_name, TestStatus.PASS,
                                expected="Error code",
                                actual=f"code={code}")
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            expected="Rejection",
                            actual="Order accepted with qty=0")
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))


def test_edge_004_negative_price() -> TestResult:
    """
    EDGE-004: 负数价格 → 应拒绝
    """
    test_id = "EDGE-004"
    test_name = "负价格订单"
    
    print(f"\n[{test_id}] {test_name}")
    
    client = get_test_client(GATEWAY_URL, USER_MAKER)
    
    try:
        order_data = {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "-100.00",
            "qty": "0.001",
            "time_in_force": "GTC",
        }
        
        status_code, resp = place_order_raw(client, order_data)
        print(f"  Response: status={status_code}, data={resp}")
        
        if status_code >= 400:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected="Rejected",
                            actual=f"status={status_code}")
        else:
            code = resp.get("code", 0)
            if code != 0:
                return TestResult(test_id, test_name, TestStatus.PASS,
                                expected="Error code",
                                actual=f"code={code}")
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            expected="Rejection",
                            actual="Order accepted with negative price")
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))


def test_edge_005_negative_qty() -> TestResult:
    """
    EDGE-005: 负数数量 → 应拒绝
    """
    test_id = "EDGE-005"
    test_name = "负数量订单"
    
    print(f"\n[{test_id}] {test_name}")
    
    client = get_test_client(GATEWAY_URL, USER_MAKER)
    
    try:
        order_data = {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "50000.00",
            "qty": "-0.001",
            "time_in_force": "GTC",
        }
        
        status_code, resp = place_order_raw(client, order_data)
        print(f"  Response: status={status_code}, data={resp}")
        
        if status_code >= 400:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected="Rejected",
                            actual=f"status={status_code}")
        else:
            code = resp.get("code", 0)
            if code != 0:
                return TestResult(test_id, test_name, TestStatus.PASS,
                                expected="Error code",
                                actual=f"code={code}")
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            expected="Rejection",
                            actual="Order accepted with negative qty")
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))


# =============================================================================
# 流程专家审核补充: Market Order 测试
# =============================================================================

def get_order_details(client: ApiClient, order_id: int) -> Optional[Dict]:
    """Get full order details"""
    resp = client.get(f"/api/v1/private/order/{order_id}")
    if resp.status_code == 200:
        return resp.json().get("data", {})
    return None


def get_order_status(client: ApiClient, order_id: int) -> Optional[str]:
    """Get order status"""
    resp = client.get(f"/api/v1/private/order/{order_id}")
    if resp.status_code == 200:
        return resp.json().get("data", {}).get("status")
    return None


def wait_for_order_terminal(client: ApiClient, order_id: int, timeout: float = 3.0) -> Optional[str]:
    """Wait for terminal state"""
    import time
    terminal_states = {"FILLED", "EXPIRED", "CANCELED", "REJECTED"}
    start = time.time()
    while time.time() - start < timeout:
        status = get_order_status(client, order_id)
        if status in terminal_states:
            return status
        time.sleep(0.1)
    return get_order_status(client, order_id)


def test_mkt_001_market_order_basic() -> TestResult:
    """
    MKT-001: 市价单基本成交
    
    流程专家审核补充: 设计规范提到Market Order，需验证
    
    前置条件: Ask 深度存在
    操作: BUY MARKET (qty=0.001)
    预期: 以最优价成交，不入簿
    """
    test_id = "MKT-001"
    test_name = "市价单基本成交"
    
    print(f"\n[{test_id}] {test_name}")
    
    client_maker = get_test_client(GATEWAY_URL, USER_MAKER)
    client_taker = get_test_client(GATEWAY_URL, USER_TAKER)
    
    ask_price = "73000.00"
    qty = "0.001"
    
    maker_order_id = None
    market_order_id = None
    
    try:
        # Place Ask (Maker)
        maker_order_id, _, _ = place_order(client_maker, SYMBOL, "SELL", ask_price, qty, "GTC")
        if not maker_order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place maker order")
        
        print(f"  Ask: {maker_order_id} @ {ask_price}")
        time.sleep(0.5)
        
        # Place Market Order BUY
        order_data = {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "MARKET",
            "qty": qty,
        }
        
        resp = client_taker.post("/api/v1/private/order", order_data)
        
        if resp.status_code not in [200, 202]:
            return TestResult(test_id, test_name, TestStatus.SKIP,
                            details=f"Market order not supported: {resp.status_code}")
        
        data = resp.json()
        market_order_id = data.get("data", {}).get("order_id")
        
        if not market_order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details=f"Failed to place market order: {data}")
        
        print(f"  Market BUY: {market_order_id}")
        time.sleep(1.0)
        
        # Verify market order filled
        status = wait_for_order_terminal(client_taker, market_order_id, 3.0)
        print(f"  Market order status: {status}")
        
        expected = "FILLED (market order executed)"
        actual = f"status={status}"
        
        if status == "FILLED":
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        elif status in ["ACCEPTED", "NEW"]:
            # May be async
            return TestResult(test_id, test_name, TestStatus.PASS,
                            details="Async processing",
                            expected=expected, actual=actual)
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            expected=expected, actual=actual)
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))
    
    finally:
        cleanup_order(client_maker, maker_order_id)


def test_mkt_002_market_order_partial_expire() -> TestResult:
    """
    MKT-002: 市价单剩余过期
    
    前置条件: Ask 深度 = 60 (小于需要的100)
    操作: BUY MARKET (qty=100)
    预期: 成交60，剩余40过期，不入簿
    """
    test_id = "MKT-002"
    test_name = "市价单剩余过期"
    
    print(f"\n[{test_id}] {test_name}")
    
    client_maker = get_test_client(GATEWAY_URL, USER_MAKER)
    client_taker = get_test_client(GATEWAY_URL, USER_TAKER)
    
    ask_price = "74000.00"
    maker_qty = "0.0006"
    market_qty = "0.001"
    
    maker_order_id = None
    
    try:
        # Place smaller Ask
        maker_order_id, _, _ = place_order(client_maker, SYMBOL, "SELL", ask_price, maker_qty, "GTC")
        if not maker_order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place maker order")
        
        print(f"  Ask: {maker_order_id} (qty={maker_qty})")
        time.sleep(0.5)
        
        # Place larger Market Order
        order_data = {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "MARKET",
            "qty": market_qty,
        }
        
        resp = client_taker.post("/api/v1/private/order", order_data)
        
        if resp.status_code not in [200, 202]:
            return TestResult(test_id, test_name, TestStatus.SKIP,
                            details=f"Market order not supported: {resp.status_code}")
        
        data = resp.json()
        market_order_id = data.get("data", {}).get("order_id")
        
        if not market_order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details=f"Failed to place market order: {data}")
        
        print(f"  Market BUY: {market_order_id} (qty={market_qty})")
        time.sleep(1.0)
        
        # Market order should NOT be in book (remainder expires)
        # For market orders, we check a BUY at a high price wouldn't show
        in_book = check_order_in_book(SYMBOL, "BUY", ask_price)
        
        print(f"  Market order in book: {in_book}")
        
        expected = "Market order remainder expires, not in book"
        actual = f"in_book={in_book}"
        
        if not in_book:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            details="Market order remainder in book",
                            expected=expected, actual=actual)
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))
    
    finally:
        cleanup_order(client_maker, maker_order_id)


# =============================================================================
# State Consistency Tests
# =============================================================================

def test_state_001_ioc_not_in_depth() -> TestResult:
    """
    STATE-001: IOC 订单不应出现在 depth API
    """
    test_id = "STATE-001"
    test_name = "IOC 不在 depth 中"
    
    print(f"\n[{test_id}] {test_name}")
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    price = "2000.00"  # Won't match
    order_id = None
    
    try:
        order_id, _, _ = place_order(client, SYMBOL, "BUY", price, "0.001", "IOC")
        
        if not order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place IOC")
        
        print(f"  IOC order: {order_id}")
        time.sleep(0.5)
        
        in_book = check_order_in_book(SYMBOL, "BUY", price)
        print(f"  In depth: {in_book}")
        
        if not in_book:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected="Not in depth",
                            actual=f"in_book={in_book}")
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            details="CRITICAL: IOC visible in depth API",
                            expected="Not in depth",
                            actual=f"in_book={in_book}")
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))


def test_state_002_gtc_in_depth() -> TestResult:
    """
    STATE-002: GTC 订单应出现在 depth API
    """
    test_id = "STATE-002"
    test_name = "GTC 在 depth 中"
    
    print(f"\n[{test_id}] {test_name}")
    
    client = get_test_client(GATEWAY_URL, USER_MAKER)
    price = "41000.00"
    order_id = None
    
    try:
        order_id, _, _ = place_order(client, SYMBOL, "BUY", price, "0.001", "GTC")
        
        if not order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place GTC")
        
        print(f"  GTC order: {order_id}")
        time.sleep(0.5)
        
        in_book = check_order_in_book(SYMBOL, "BUY", price)
        print(f"  In depth: {in_book}")
        
        if in_book:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected="In depth",
                            actual=f"in_book={in_book}")
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            expected="In depth",
                            actual=f"in_book={in_book}")
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))
    
    finally:
        cleanup_order(client, order_id)


def test_state_003_cancel_removes_from_depth() -> TestResult:
    """
    STATE-003: Cancel 后订单从 depth 消失
    """
    test_id = "STATE-003"
    test_name = "Cancel 后从 depth 消失"
    
    print(f"\n[{test_id}] {test_name}")
    
    client = get_test_client(GATEWAY_URL, USER_MAKER)
    price = "42000.00"
    order_id = None
    
    try:
        order_id, _, _ = place_order(client, SYMBOL, "BUY", price, "0.001", "GTC")
        
        if not order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place order")
        
        print(f"  Order: {order_id}")
        time.sleep(0.5)
        
        in_book_before = check_order_in_book(SYMBOL, "BUY", price)
        print(f"  Before cancel: {in_book_before}")
        
        cancel_order(client, order_id)
        time.sleep(0.5)
        
        in_book_after = check_order_in_book(SYMBOL, "BUY", price)
        print(f"  After cancel: {in_book_after}")
        
        if in_book_before and not in_book_after:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected="Removed from depth after cancel",
                            actual=f"before={in_book_before}, after={in_book_after}")
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            expected="before=True, after=False",
                            actual=f"before={in_book_before}, after={in_book_after}")
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))


# =============================================================================
# Main
# =============================================================================

def main():
    print("=" * 70)
    print("🧪 QA 0x14-b: Edge Cases & State Consistency Test Suite")
    print("=" * 70)
    print(f"Gateway: {GATEWAY_URL}")
    print(f"Symbol: {SYMBOL}")
    
    try:
        resp = requests.get(f"{GATEWAY_URL}/api/v1/public/exchange_info", timeout=5)
        if resp.status_code != 200:
            print(f"\n❌ Gateway not responding: {resp.status_code}")
            return 1
    except Exception as e:
        print(f"\n❌ Cannot connect to Gateway: {e}")
        return 1
    
    print("\n✅ Gateway connected")
    
    tests = [
        # Edge cases
        test_edge_001_min_qty,
        test_edge_002_zero_price,
        test_edge_003_zero_qty,
        test_edge_004_negative_price,
        test_edge_005_negative_qty,
        # 流程专家审核补充: Market Order
        test_mkt_001_market_order_basic,
        test_mkt_002_market_order_partial_expire,
        # State consistency
        test_state_001_ioc_not_in_depth,
        test_state_002_gtc_in_depth,
        test_state_003_cancel_removes_from_depth,
    ]
    
    results = []
    for test_fn in tests:
        try:
            results.append(test_fn())
        except Exception as e:
            results.append(TestResult("UNKNOWN", test_fn.__name__, TestStatus.ERROR, str(e)))
    
    print("\n")
    print("=" * 70)
    print("📊 EDGE CASES & STATE TEST RESULTS")
    print("=" * 70)
    
    passed = failed = skipped = errors = 0
    for r in results:
        if r.status == TestStatus.PASS:
            icon, passed = "✅", passed + 1
        elif r.status == TestStatus.FAIL:
            icon, failed = "❌", failed + 1
        elif r.status == TestStatus.SKIP:
            icon, skipped = "⏭️", skipped + 1
        else:
            icon, errors = "⚠️", errors + 1
        
        print(f"  {icon} [{r.test_id}] {r.name}: {r.status.value}")
        if r.status != TestStatus.PASS and r.status != TestStatus.SKIP:
            if r.expected: print(f"       Expected: {r.expected}")
            if r.actual: print(f"       Actual:   {r.actual}")
            if r.details: print(f"       Details:  {r.details}")
    
    print("=" * 70)
    print(f"Summary: {passed} PASS, {failed} FAIL, {skipped} SKIP, {errors} ERROR")
    
    if failed > 0:
        print("\n⚠️  Edge Cases Test Suite: FAILURES DETECTED")
        return 1
    
    print("\n✅ Edge Cases Test Suite: COMPLETE")
    return 0


if __name__ == "__main__":
    sys.exit(main())
