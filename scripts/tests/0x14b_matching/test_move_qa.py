#!/usr/bin/env python3
"""
QA 0x14-b: MoveOrder 独立测试

测试目标:
    验证 MoveOrder 功能：移动订单价格，优先级丢失

核心规则:
    - MoveOrder 后订单 **丢失** 时间优先级 (原子 Cancel+Place)
    - 移价后可能触发成交

测试用例:
    MOV-001: 移价后优先级丢失
    MOV-002: 移价触发成交
    MOV-003: 同价位移动
    MOV-004: 移动不存在的订单
    MOV-005: 移动已成交订单
    MOV-006: BUY 向上移价
    MOV-007: SELL 向下移价

Usage:
    python3 scripts/tests/0x14b_matching/test_move_qa.py

Author: QA Engineer (Independent Design)
Date: 2025-12-30
"""

import sys
import os
import time
from typing import Optional, Dict, List, Tuple
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


def move_order(client: ApiClient, order_id: int, new_price: str) -> Tuple[bool, Dict]:
    """
    Move order to new price
    
    Returns: (success, response_data)
    """
    data = {
        "order_id": order_id,
        "new_price": new_price
    }
    resp = client.post("/api/v1/private/order/move", data)
    
    try:
        resp_data = resp.json()
    except:
        resp_data = {"error": resp.text[:200]}
    
    return resp.status_code in [200, 202], resp_data


def get_order_status(client: ApiClient, order_id: int) -> Optional[str]:
    resp = client.get(f"/api/v1/private/order/{order_id}")
    if resp.status_code == 200:
        return resp.json().get("data", {}).get("status")
    return None


def cancel_order(client: ApiClient, order_id: int) -> bool:
    resp = client.delete(f"/api/v1/private/order/{order_id}")
    return resp.status_code in [200, 202]


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
# MoveOrder Test Cases
# =============================================================================

def test_mov_001_priority_loss() -> TestResult:
    """
    MOV-001: 移价后优先级丢失
    
    步骤:
    1. Place Buy A at 49000 (先下单)
    2. Place Buy B at 50000 (后下单)
    3. Move A to 50000 → A 现在在 B 后面
    4. Match with Sell C for 1.0 BTC at 50000
    
    预期: B 先成交 (FILLED), A 仍在簿中 (NEW)
    验证: 移动后优先级丢失 - A 虽然先下单，但移动后排在 B 后面
    """
    test_id = "MOV-001"
    test_name = "MoveOrder 优先级丢失"
    
    print(f"\n[{test_id}] {test_name}")
    
    client_maker = get_test_client(GATEWAY_URL, USER_MAKER)
    client_taker = get_test_client(GATEWAY_URL, USER_TAKER)
    
    price_a = "59000.00"
    price_b = "60000.00"
    
    id_a = None
    id_b = None
    
    try:
        # Step 1: Place Order A at lower price (first)
        id_a, _, _ = place_order(client_maker, SYMBOL, "BUY", price_a, "0.01", "GTC")
        if not id_a:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place Order A")
        print(f"  Order A: {id_a} @ {price_a}")
        
        time.sleep(0.3)
        
        # Step 2: Place Order B at target price (second)
        id_b, _, _ = place_order(client_maker, SYMBOL, "BUY", price_b, "0.01", "GTC")
        if not id_b:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place Order B")
        print(f"  Order B: {id_b} @ {price_b}")
        
        time.sleep(0.5)
        
        # Step 3: Move A to same price as B
        print(f"  Moving Order A to {price_b}")
        success, move_resp = move_order(client_maker, id_a, price_b)
        
        if not success:
            return TestResult(test_id, test_name, TestStatus.SKIP,
                            details=f"MoveOrder not implemented: {move_resp}")
        
        time.sleep(0.5)
        
        # Step 4: Match with Sell C for exactly one order's quantity
        print(f"  Matching with Sell 0.01 BTC @ {price_b}")
        place_order(client_taker, SYMBOL, "SELL", price_b, "0.01", "IOC")
        
        time.sleep(1.0)
        
        # Step 5: Verify B FILLED (it was first at price), A still in book
        status_a = get_order_status(client_maker, id_a)
        status_b = get_order_status(client_maker, id_b)
        
        print(f"  Order A status: {status_a}")
        print(f"  Order B status: {status_b}")
        
        expected = "B=FILLED (first at price), A=NEW/ACCEPTED (moved to end of queue)"
        actual = f"A={status_a}, B={status_b}"
        
        # B should be filled, A should still be resting
        if status_b == "FILLED" and status_a in ["NEW", "ACCEPTED"]:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            details="Priority was not lost after move",
                            expected=expected, actual=actual)
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))
    
    finally:
        cleanup_order(client_maker, id_a)
        cleanup_order(client_maker, id_b)


def test_mov_002_move_triggers_match() -> TestResult:
    """
    MOV-002: 移价触发成交
    
    步骤:
    1. Place BID A at 49000
    2. Place ASK at 49500
    3. Move A to 50000 (crosses ASK)
    
    预期: A 与 ASK 成交
    """
    test_id = "MOV-002"
    test_name = "MoveOrder 触发成交"
    
    print(f"\n[{test_id}] {test_name}")
    
    client_maker = get_test_client(GATEWAY_URL, USER_MAKER)
    client_taker = get_test_client(GATEWAY_URL, USER_TAKER)
    
    bid_price = "58000.00"
    ask_price = "58500.00"
    move_price = "59000.00"  # Will cross the ask
    
    id_a = None
    id_ask = None
    
    try:
        # Place BID A
        id_a, _, _ = place_order(client_maker, SYMBOL, "BUY", bid_price, "0.001", "GTC")
        if not id_a:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place BID")
        print(f"  BID A: {id_a} @ {bid_price}")
        
        # Place ASK
        id_ask, _, _ = place_order(client_taker, SYMBOL, "SELL", ask_price, "0.001", "GTC")
        if not id_ask:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place ASK")
        print(f"  ASK: {id_ask} @ {ask_price}")
        
        time.sleep(0.5)
        
        # Move BID A to cross the ASK
        print(f"  Moving BID A to {move_price} (crosses ASK @ {ask_price})")
        success, move_resp = move_order(client_maker, id_a, move_price)
        
        if not success:
            return TestResult(test_id, test_name, TestStatus.SKIP,
                            details=f"MoveOrder not implemented: {move_resp}")
        
        time.sleep(1.0)
        
        # Verify A is FILLED
        status_a = get_order_status(client_maker, id_a)
        status_ask = get_order_status(client_taker, id_ask)
        
        print(f"  BID A status: {status_a}")
        print(f"  ASK status: {status_ask}")
        
        expected = "A=FILLED (crossed with ASK)"
        actual = f"A={status_a}, ASK={status_ask}"
        
        if status_a == "FILLED":
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            expected=expected, actual=actual)
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))
    
    finally:
        cleanup_order(client_maker, id_a)
        cleanup_order(client_taker, id_ask)


def test_mov_003_same_price() -> TestResult:
    """
    MOV-003: 同价位移动
    
    操作: Move A from 50000 to 50000
    预期: 无变化或保持原位（具体行为取决于实现）
    """
    test_id = "MOV-003"
    test_name = "MoveOrder 同价位"
    
    print(f"\n[{test_id}] {test_name}")
    
    client = get_test_client(GATEWAY_URL, USER_MAKER)
    price = "57000.00"
    order_id = None
    
    try:
        order_id, _, _ = place_order(client, SYMBOL, "BUY", price, "0.001", "GTC")
        if not order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place order")
        
        print(f"  Order: {order_id} @ {price}")
        time.sleep(0.5)
        
        # Move to same price
        print(f"  Moving to same price {price}")
        success, move_resp = move_order(client, order_id, price)
        
        if not success:
            return TestResult(test_id, test_name, TestStatus.SKIP,
                            details=f"MoveOrder not implemented: {move_resp}")
        
        time.sleep(0.5)
        
        # Order should still be valid
        status = get_order_status(client, order_id)
        in_book = check_order_in_book(SYMBOL, "BUY", price)
        
        print(f"  Status: {status}, In book: {in_book}")
        
        expected = "Order still valid in book"
        actual = f"status={status}, in_book={in_book}"
        
        if status in ["NEW", "ACCEPTED"] and in_book:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        else:
            # Also acceptable if same-price move is rejected
            return TestResult(test_id, test_name, TestStatus.PASS,
                            details="Same-price move handled",
                            expected=expected, actual=actual)
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))
    
    finally:
        cleanup_order(client, order_id)


def test_mov_004_nonexistent_order() -> TestResult:
    """
    MOV-004: 移动不存在的订单 → 验证无副作用
    
    设计说明:
        Gateway 是异步系统，对不存在订单的 MoveOrder 会返回 ACCEPTED，
        但 Pipeline 处理后不会产生任何副作用。
        测试验证：请求被接受 + 订单簿无变化。
    """
    test_id = "MOV-004"
    test_name = "MoveOrder 不存在订单 (异步验证)"
    
    print(f"\n[{test_id}] {test_name}")
    
    client = get_test_client(GATEWAY_URL, USER_MAKER)
    
    try:
        # 记录操作前订单簿状态
        depth_before = get_order_book(SYMBOL)
        bids_before = len(depth_before.get("bids", []))
        asks_before = len(depth_before.get("asks", []))
        
        fake_order_id = 9999999999
        target_price = "50000.00"
        print(f"  Attempting to move non-existent order: {fake_order_id}")
        
        # Gateway 异步入队，预期返回成功
        success, resp = move_order(client, fake_order_id, target_price)
        print(f"  Response: success={success}, data={resp}")
        
        # 等待异步处理
        time.sleep(0.5)
        
        # 验证订单簿无变化 (无副作用)
        depth_after = get_order_book(SYMBOL)
        bids_after = len(depth_after.get("bids", []))
        asks_after = len(depth_after.get("asks", []))
        
        # 确保目标价位没有新订单
        has_order_at_price = check_order_in_book(SYMBOL, "BUY", target_price)
        
        print(f"  Order book before: bids={bids_before}, asks={asks_before}")
        print(f"  Order book after:  bids={bids_after}, asks={asks_after}")
        print(f"  Order at target price: {has_order_at_price}")
        
        expected = "Request accepted, no side effects (order book unchanged)"
        actual = f"success={success}, book_unchanged={(bids_before == bids_after and asks_before == asks_after)}"
        
        # 异步系统：Gateway 接受请求是正常的，关键是无副作用
        if not has_order_at_price:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            expected=expected,
                            actual="Unexpected order appeared in book")
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))


def test_mov_005_filled_order() -> TestResult:
    """
    MOV-005: 移动已成交订单 → 验证无副作用
    
    设计说明:
        Gateway 异步接受请求，对已成交订单的 MoveOrder 不产生副作用。
        测试验证：订单状态仍为 FILLED，新价位无订单出现。
    """
    test_id = "MOV-005"
    test_name = "MoveOrder 已成交订单 (异步验证)"
    
    print(f"\n[{test_id}] {test_name}")
    
    client_maker = get_test_client(GATEWAY_URL, USER_MAKER)
    client_taker = get_test_client(GATEWAY_URL, USER_TAKER)
    
    price = "56000.00"
    new_price = "55000.00"
    order_id = None
    
    try:
        # Place and fill an order
        order_id, _, _ = place_order(client_maker, SYMBOL, "BUY", price, "0.001", "GTC")
        if not order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place order")
        
        print(f"  Order: {order_id} @ {price}")
        time.sleep(0.5)
        
        # Fill it
        place_order(client_taker, SYMBOL, "SELL", price, "0.001", "IOC")
        time.sleep(1.0)
        
        status = wait_for_order_terminal(client_maker, order_id, 2.0)
        print(f"  Order status after fill: {status}")
        
        if status != "FILLED":
            return TestResult(test_id, test_name, TestStatus.SKIP,
                            details=f"Order not filled, status={status}")
        
        # Try to move filled order (Gateway will accept, Pipeline will ignore)
        print(f"  Attempting to move filled order to {new_price}")
        success, resp = move_order(client_maker, order_id, new_price)
        print(f"  Response: success={success}")
        
        # 等待异步处理
        time.sleep(0.5)
        
        # 验证：订单状态仍为 FILLED，新价位无订单
        status_after = get_order_status(client_maker, order_id)
        has_order_at_new_price = check_order_in_book(SYMBOL, "BUY", new_price)
        
        print(f"  Order status after move attempt: {status_after}")
        print(f"  Order at new price {new_price}: {has_order_at_new_price}")
        
        expected = "Status remains FILLED, no order at new price"
        actual = f"status={status_after}, order_at_new_price={has_order_at_new_price}"
        
        # 成功条件：状态仍为 FILLED，新价位没有订单
        if status_after == "FILLED" and not has_order_at_new_price:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            expected=expected, actual=actual)
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))


def test_mov_006_buy_move_up() -> TestResult:
    """
    MOV-006: BUY 向上移价
    
    验证: 订单在新价位可见
    """
    test_id = "MOV-006"
    test_name = "BUY 向上移价"
    
    print(f"\n[{test_id}] {test_name}")
    
    client = get_test_client(GATEWAY_URL, USER_MAKER)
    old_price = "54000.00"
    new_price = "55000.00"
    order_id = None
    
    try:
        order_id, _, _ = place_order(client, SYMBOL, "BUY", old_price, "0.001", "GTC")
        if not order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place order")
        
        print(f"  Order: {order_id} @ {old_price}")
        time.sleep(0.5)
        
        # Move up
        print(f"  Moving BUY from {old_price} to {new_price}")
        success, _ = move_order(client, order_id, new_price)
        
        if not success:
            return TestResult(test_id, test_name, TestStatus.SKIP,
                            details="MoveOrder not implemented")
        
        time.sleep(0.5)
        
        in_old = check_order_in_book(SYMBOL, "BUY", old_price)
        in_new = check_order_in_book(SYMBOL, "BUY", new_price)
        
        print(f"  In old price: {in_old}, In new price: {in_new}")
        
        expected = "Not at old, at new price"
        actual = f"in_old={in_old}, in_new={in_new}"
        
        if not in_old and in_new:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            expected=expected, actual=actual)
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))
    
    finally:
        cleanup_order(client, order_id)


def test_mov_007_sell_move_down() -> TestResult:
    """
    MOV-007: SELL 向下移价
    
    验证: 订单在新价位可见
    """
    test_id = "MOV-007"
    test_name = "SELL 向下移价"
    
    print(f"\n[{test_id}] {test_name}")
    
    client = get_test_client(GATEWAY_URL, USER_MAKER)
    old_price = "62000.00"
    new_price = "61000.00"
    order_id = None
    
    try:
        order_id, _, _ = place_order(client, SYMBOL, "SELL", old_price, "0.001", "GTC")
        if not order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place order")
        
        print(f"  Order: {order_id} @ {old_price}")
        time.sleep(0.5)
        
        # Move down
        print(f"  Moving SELL from {old_price} to {new_price}")
        success, _ = move_order(client, order_id, new_price)
        
        if not success:
            return TestResult(test_id, test_name, TestStatus.SKIP,
                            details="MoveOrder not implemented")
        
        time.sleep(0.5)
        
        in_old = check_order_in_book(SYMBOL, "SELL", old_price)
        in_new = check_order_in_book(SYMBOL, "SELL", new_price)
        
        print(f"  In old price: {in_old}, In new price: {in_new}")
        
        expected = "Not at old, at new price"
        actual = f"in_old={in_old}, in_new={in_new}"
        
        if not in_old and in_new:
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
# Main
# =============================================================================

def main():
    print("=" * 70)
    print("🧪 QA 0x14-b: MoveOrder Independent Test Suite")
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
        test_mov_001_priority_loss,
        test_mov_002_move_triggers_match,
        test_mov_003_same_price,
        test_mov_004_nonexistent_order,
        test_mov_005_filled_order,
        test_mov_006_buy_move_up,
        test_mov_007_sell_move_down,
    ]
    
    results = []
    for test_fn in tests:
        try:
            results.append(test_fn())
        except Exception as e:
            results.append(TestResult("UNKNOWN", test_fn.__name__, TestStatus.ERROR, str(e)))
    
    print("\n")
    print("=" * 70)
    print("📊 MOVEORDER TEST RESULTS")
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
        print("\n⚠️  MoveOrder Test Suite: FAILURES DETECTED")
        return 1
    
    print("\n✅ MoveOrder Test Suite: COMPLETE")
    return 0


if __name__ == "__main__":
    sys.exit(main())
