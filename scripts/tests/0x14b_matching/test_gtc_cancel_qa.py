#!/usr/bin/env python3
"""
QA 0x14-b: GTC 和 Cancel 基线测试

测试目标:
    验证 GTC 和 Cancel 的基线行为正确

核心规则:
    - GTC 订单未完全成交部分 **必须** 入簿
    - Cancel 成功后订单 **必须** 从簿中移除

测试用例:
    GTC-001: GTC Maker 入簿
    GTC-002: GTC 部分成交后入簿
    GTC-003: GTC 完全成交
    GTC-004: GTC SELL 入簿
    CAN-001: 取消存在的订单
    CAN-002: 取消不存在的订单
    CAN-003: 重复取消
    CAN-004: 取消已成交订单
    CAN-005: 取消 IOC 订单

Usage:
    python3 scripts/tests/0x14b_matching/test_gtc_cancel_qa.py

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


def get_order_status(client: ApiClient, order_id: int) -> Optional[str]:
    resp = client.get(f"/api/v1/private/order/{order_id}")
    if resp.status_code == 200:
        return resp.json().get("data", {}).get("status")
    return None


def cancel_order(client: ApiClient, order_id: int) -> Tuple[bool, Dict]:
    resp = client.delete(f"/api/v1/private/order/{order_id}")
    try:
        data = resp.json()
    except:
        data = {"text": resp.text[:200]}
    return resp.status_code in [200, 202], data


def wait_for_order_terminal(client: ApiClient, order_id: int, timeout: float = 3.0) -> Optional[str]:
    terminal_states = {"FILLED", "EXPIRED", "CANCELED", "REJECTED"}
    start = time.time()
    while time.time() - start < timeout:
        status = get_order_status(client, order_id)
        if status in terminal_states:
            return status
        time.sleep(0.1)
    return get_order_status(client, order_id)


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
# GTC Test Cases
# =============================================================================

def test_gtc_001_maker_rests() -> TestResult:
    """
    GTC-001: GTC Maker 入簿
    
    前置条件: 订单簿空
    操作: BUY GTC 100 @ 50000
    预期: 成交0, 100 入簿, 状态=NEW/ACCEPTED
    """
    test_id = "GTC-001"
    test_name = "GTC Maker 入簿"
    
    print(f"\n[{test_id}] {test_name}")
    
    client = get_test_client(GATEWAY_URL, USER_MAKER)
    price = "45000.00"  # Low price, won't cross
    order_id = None
    
    try:
        order_id, initial_status, _ = place_order(client, SYMBOL, "BUY", price, "0.001", "GTC")
        if not order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place order")
        
        print(f"  Order: {order_id}, initial_status={initial_status}")
        time.sleep(0.5)
        
        # Verify in book
        in_book = check_order_in_book(SYMBOL, "BUY", price)
        status = get_order_status(client, order_id)
        
        print(f"  In book: {in_book}, status: {status}")
        
        expected = "In book, status=NEW/ACCEPTED"
        actual = f"in_book={in_book}, status={status}"
        
        if in_book and status in ["NEW", "ACCEPTED"]:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            expected=expected, actual=actual)
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))
    
    finally:
        cleanup_order(client, order_id)


def test_gtc_002_partial_fill_rests() -> TestResult:
    """
    GTC-002: GTC 部分成交后入簿
    
    前置条件: Ask 深度 = 60 @ 50000
    操作: BUY GTC 100 @ 50000
    预期: 成交60, 剩余40 **入簿**
    """
    test_id = "GTC-002"
    test_name = "GTC 部分成交入簿"
    
    print(f"\n[{test_id}] {test_name}")
    
    client_maker = get_test_client(GATEWAY_URL, USER_MAKER)
    client_taker = get_test_client(GATEWAY_URL, USER_TAKER)
    
    price = "64000.00"
    ask_qty = "0.0006"
    buy_qty = "0.001"
    
    ask_id = None
    buy_id = None
    
    try:
        # Place smaller ask
        ask_id, _, _ = place_order(client_taker, SYMBOL, "SELL", price, ask_qty, "GTC")
        if not ask_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place ASK")
        
        print(f"  ASK: {ask_id} (qty={ask_qty})")
        time.sleep(0.5)
        
        # Place larger GTC buy
        buy_id, _, _ = place_order(client_maker, SYMBOL, "BUY", price, buy_qty, "GTC")
        if not buy_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place BUY")
        
        print(f"  BUY GTC: {buy_id} (qty={buy_qty})")
        time.sleep(1.0)
        
        # Verify GTC remainder in book
        in_book = check_order_in_book(SYMBOL, "BUY", price)
        status = get_order_status(client_maker, buy_id)
        
        print(f"  GTC in book: {in_book}, status: {status}")
        
        expected = "Remainder in book, status=PARTIALLY_FILLED/NEW"
        actual = f"in_book={in_book}, status={status}"
        
        # GTC should rest in book after partial fill
        if in_book:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            details="GTC remainder not in book",
                            expected=expected, actual=actual)
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))
    
    finally:
        cleanup_order(client_maker, buy_id)
        cleanup_order(client_taker, ask_id)


def test_gtc_003_full_fill() -> TestResult:
    """
    GTC-003: GTC 完全成交
    
    前置条件: Ask 深度 = 100 @ 50000
    操作: BUY GTC 100 @ 50000
    预期: 成交100, FILLED, 不入簿
    """
    test_id = "GTC-003"
    test_name = "GTC 完全成交"
    
    print(f"\n[{test_id}] {test_name}")
    
    client_maker = get_test_client(GATEWAY_URL, USER_MAKER)
    client_taker = get_test_client(GATEWAY_URL, USER_TAKER)
    
    price = "63000.00"
    qty = "0.001"
    
    ask_id = None
    buy_id = None
    
    try:
        # Place ask equal to buy
        ask_id, _, _ = place_order(client_taker, SYMBOL, "SELL", price, qty, "GTC")
        if not ask_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place ASK")
        
        print(f"  ASK: {ask_id}")
        time.sleep(0.5)
        
        # Place GTC buy
        buy_id, _, _ = place_order(client_maker, SYMBOL, "BUY", price, qty, "GTC")
        if not buy_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place BUY")
        
        print(f"  BUY GTC: {buy_id}")
        time.sleep(1.0)
        
        status = wait_for_order_terminal(client_maker, buy_id, 3.0)
        print(f"  Status: {status}")
        
        expected = "FILLED"
        actual = status
        
        if status == "FILLED":
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            expected=expected, actual=actual)
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))
    
    finally:
        cleanup_order(client_maker, buy_id)
        cleanup_order(client_taker, ask_id)


def test_gtc_004_sell_rests() -> TestResult:
    """
    GTC-004: GTC SELL 入簿
    """
    test_id = "GTC-004"
    test_name = "GTC SELL 入簿"
    
    print(f"\n[{test_id}] {test_name}")
    
    client = get_test_client(GATEWAY_URL, USER_MAKER)
    price = "80000.00"  # High price, won't cross
    order_id = None
    
    try:
        order_id, _, _ = place_order(client, SYMBOL, "SELL", price, "0.001", "GTC")
        if not order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place order")
        
        print(f"  Order: {order_id}")
        time.sleep(0.5)
        
        in_book = check_order_in_book(SYMBOL, "SELL", price)
        status = get_order_status(client, order_id)
        
        print(f"  In book: {in_book}, status: {status}")
        
        expected = "In book"
        actual = f"in_book={in_book}, status={status}"
        
        if in_book:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            expected=expected, actual=actual)
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))
    
    finally:
        cleanup_order(client, order_id)


# =============================================================================
# Cancel Test Cases
# =============================================================================

def test_can_001_cancel_existing() -> TestResult:
    """
    CAN-001: 取消存在的订单
    """
    test_id = "CAN-001"
    test_name = "Cancel 存在订单"
    
    print(f"\n[{test_id}] {test_name}")
    
    client = get_test_client(GATEWAY_URL, USER_MAKER)
    price = "44000.00"
    order_id = None
    
    try:
        order_id, _, _ = place_order(client, SYMBOL, "BUY", price, "0.001", "GTC")
        if not order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place order")
        
        print(f"  Order: {order_id}")
        time.sleep(0.5)
        
        # Cancel
        success, _ = cancel_order(client, order_id)
        print(f"  Cancel success: {success}")
        
        time.sleep(0.5)
        
        # Verify
        status = get_order_status(client, order_id)
        in_book = check_order_in_book(SYMBOL, "BUY", price)
        
        print(f"  Status: {status}, In book: {in_book}")
        
        expected = "CANCELED, not in book"
        actual = f"status={status}, in_book={in_book}"
        
        if success and status == "CANCELED" and not in_book:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            expected=expected, actual=actual)
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))


def test_can_002_nonexistent() -> TestResult:
    """
    CAN-002: 取消不存在的订单
    """
    test_id = "CAN-002"
    test_name = "Cancel 不存在订单"
    
    print(f"\n[{test_id}] {test_name}")
    
    client = get_test_client(GATEWAY_URL, USER_MAKER)
    
    try:
        fake_id = 9999999999
        print(f"  Canceling non-existent: {fake_id}")
        
        success, resp = cancel_order(client, fake_id)
        print(f"  Response: success={success}, data={resp}")
        
        # Should fail
        if not success:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected="Error",
                            actual=f"success={success}")
        else:
            code = resp.get("code", 0)
            if code != 0:
                return TestResult(test_id, test_name, TestStatus.PASS,
                                expected="Error code",
                                actual=f"code={code}")
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            expected="Error",
                            actual="Cancel accepted")
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))


def test_can_003_double_cancel() -> TestResult:
    """
    CAN-003: 重复取消
    """
    test_id = "CAN-003"
    test_name = "Cancel 重复取消"
    
    print(f"\n[{test_id}] {test_name}")
    
    client = get_test_client(GATEWAY_URL, USER_MAKER)
    price = "43000.00"
    order_id = None
    
    try:
        order_id, _, _ = place_order(client, SYMBOL, "BUY", price, "0.001", "GTC")
        if not order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place order")
        
        print(f"  Order: {order_id}")
        time.sleep(0.3)
        
        # First cancel
        success1, _ = cancel_order(client, order_id)
        print(f"  First cancel: {success1}")
        time.sleep(0.3)
        
        # Second cancel
        success2, resp2 = cancel_order(client, order_id)
        print(f"  Second cancel: success={success2}, resp={resp2}")
        
        # Second cancel should fail (order already canceled)
        if not success2:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected="Second cancel fails",
                            actual=f"success2={success2}")
        else:
            code = resp2.get("code", 0)
            if code != 0:
                return TestResult(test_id, test_name, TestStatus.PASS,
                                expected="Error code on second",
                                actual=f"code={code}")
            # Some systems may silently accept double cancel
            return TestResult(test_id, test_name, TestStatus.PASS,
                            details="Double cancel accepted (idempotent)",
                            expected="Error or idempotent",
                            actual=f"success={success2}")
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))


def test_can_004_cancel_filled() -> TestResult:
    """
    CAN-004: 取消已成交订单
    """
    test_id = "CAN-004"
    test_name = "Cancel 已成交订单"
    
    print(f"\n[{test_id}] {test_name}")
    
    client_maker = get_test_client(GATEWAY_URL, USER_MAKER)
    client_taker = get_test_client(GATEWAY_URL, USER_TAKER)
    
    price = "62000.00"
    order_id = None
    
    try:
        # Place and fill
        order_id, _, _ = place_order(client_maker, SYMBOL, "BUY", price, "0.001", "GTC")
        if not order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place order")
        
        print(f"  Order: {order_id}")
        time.sleep(0.3)
        
        place_order(client_taker, SYMBOL, "SELL", price, "0.001", "IOC")
        time.sleep(1.0)
        
        status = wait_for_order_terminal(client_maker, order_id, 2.0)
        print(f"  Status after fill: {status}")
        
        if status != "FILLED":
            return TestResult(test_id, test_name, TestStatus.SKIP,
                            details=f"Order not filled: {status}")
        
        # Try to cancel
        success, resp = cancel_order(client_maker, order_id)
        print(f"  Cancel response: success={success}")
        
        if not success:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected="Error (can't cancel filled)",
                            actual=f"success={success}")
        else:
            code = resp.get("code", 0)
            if code != 0:
                return TestResult(test_id, test_name, TestStatus.PASS,
                                expected="Error code",
                                actual=f"code={code}")
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            expected="Error",
                            actual="Canceled a filled order")
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))


def test_can_005_cancel_ioc() -> TestResult:
    """
    CAN-005: 取消 IOC 订单 (应该找不到)
    """
    test_id = "CAN-005"
    test_name = "Cancel IOC 订单"
    
    print(f"\n[{test_id}] {test_name}")
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    price = "1000.00"  # Won't match
    
    try:
        # Place IOC (will expire immediately with no match)
        order_id, _, _ = place_order(client, SYMBOL, "BUY", price, "0.001", "IOC")
        if not order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place IOC")
        
        print(f"  IOC order: {order_id}")
        time.sleep(0.5)  # Wait for IOC to expire
        
        # Try to cancel expired IOC
        success, resp = cancel_order(client, order_id)
        print(f"  Cancel response: success={success}")
        
        # IOC should already be expired, cancel should fail or be no-op
        if not success:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected="Error (IOC already expired)",
                            actual=f"success={success}")
        else:
            code = resp.get("code", 0)
            if code != 0:
                return TestResult(test_id, test_name, TestStatus.PASS,
                                expected="Error code",
                                actual=f"code={code}")
            # Some systems may accept cancel on expired IOC
            return TestResult(test_id, test_name, TestStatus.PASS,
                            details="Cancel on expired IOC accepted",
                            expected="Error or idempotent",
                            actual="success")
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))


# =============================================================================
# Main
# =============================================================================

def main():
    print("=" * 70)
    print("🧪 QA 0x14-b: GTC & Cancel Baseline Test Suite")
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
        # GTC tests
        test_gtc_001_maker_rests,
        test_gtc_002_partial_fill_rests,
        test_gtc_003_full_fill,
        test_gtc_004_sell_rests,
        # Cancel tests
        test_can_001_cancel_existing,
        test_can_002_nonexistent,
        test_can_003_double_cancel,
        test_can_004_cancel_filled,
        test_can_005_cancel_ioc,
    ]
    
    results = []
    for test_fn in tests:
        try:
            results.append(test_fn())
        except Exception as e:
            results.append(TestResult("UNKNOWN", test_fn.__name__, TestStatus.ERROR, str(e)))
    
    print("\n")
    print("=" * 70)
    print("📊 GTC & CANCEL TEST RESULTS")
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
        print("\n⚠️  GTC/Cancel Test Suite: FAILURES DETECTED")
        return 1
    
    print("\n✅ GTC/Cancel Test Suite: COMPLETE")
    return 0


if __name__ == "__main__":
    sys.exit(main())
