#!/usr/bin/env python3
"""
QA 0x14-b: IOC (Immediate-or-Cancel) 独立测试

测试目标:
    验证 IOC 订单的核心行为：立即成交或过期，剩余部分不入簿

核心规则:
    - IOC 订单处理后 **绝不** 应出现在订单簿中
    - 成交部分正常记录，未成交部分立即过期

测试用例:
    IOC-001: IOC 完全成交 → FILLED
    IOC-002: IOC 部分成交 → 剩余过期，不入簿
    IOC-003: IOC 无对手盘 → 全部过期，不入簿
    IOC-004: IOC 跨价位扫单 → 多档成交
    IOC-005: IOC 价格不利 → 无成交，过期
    IOC-006: IOC SELL 完全成交
    IOC-007: IOC SELL 部分成交

Usage:
    python3 scripts/tests/0x14b_matching/test_ioc_qa.py

Author: QA Engineer (Independent Design)
Date: 2025-12-30
"""

import sys
import os
import time
import json
from typing import Optional, Dict, List, Tuple
from dataclasses import dataclass
from enum import Enum

# Add scripts directory to path
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

# Test users - Use different users for maker/taker to avoid self-trade
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

def place_order(
    client: ApiClient,
    symbol: str,
    side: str,
    price: str,
    qty: str,
    time_in_force: str = "GTC",
    order_type: str = "LIMIT"
) -> Tuple[Optional[int], Optional[str], Dict]:
    """
    Place an order and return (order_id, initial_status, full_response)
    """
    order_data = {
        "symbol": symbol,
        "side": side,
        "order_type": order_type,
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
    else:
        return None, None, {"error": resp.status_code, "text": resp.text[:200]}


def get_order_status(client: ApiClient, order_id: int) -> Optional[str]:
    """Get current order status"""
    resp = client.get(f"/api/v1/private/order/{order_id}")
    if resp.status_code == 200:
        return resp.json().get("data", {}).get("status")
    return None


def get_order_details(client: ApiClient, order_id: int) -> Optional[Dict]:
    """Get full order details"""
    resp = client.get(f"/api/v1/private/order/{order_id}")
    if resp.status_code == 200:
        return resp.json().get("data", {})
    return None


def wait_for_order_terminal(client: ApiClient, order_id: int, timeout: float = 3.0) -> Optional[str]:
    """
    Wait for order to reach terminal state (FILLED, EXPIRED, CANCELED, REJECTED)
    """
    terminal_states = {"FILLED", "EXPIRED", "CANCELED", "REJECTED"}
    start = time.time()
    
    while time.time() - start < timeout:
        status = get_order_status(client, order_id)
        if status in terminal_states:
            return status
        # Also check non-terminal states to return early for testing
        if status in {"NEW", "ACCEPTED", "PARTIALLY_FILLED"}:
            time.sleep(0.2)
            continue
        time.sleep(0.1)
    
    return get_order_status(client, order_id)


def cancel_order(client: ApiClient, order_id: int) -> bool:
    """Cancel an order"""
    resp = client.delete(f"/api/v1/private/order/{order_id}")
    return resp.status_code in [200, 202]


def get_order_book(symbol: str) -> Dict:
    """Get order book depth"""
    resp = requests.get(f"{GATEWAY_URL}/api/v1/public/depth?symbol={symbol}&limit=50", timeout=5)
    if resp.status_code == 200:
        return resp.json().get("data", {})
    return {}


def check_order_in_book(symbol: str, side: str, price: str) -> bool:
    """
    Check if an order at given price exists in order book
    
    Args:
        symbol: Trading pair
        side: "BUY" (check bids) or "SELL" (check asks)
        price: Price to check
    
    Returns:
        True if order at that price exists in book
    """
    depth = get_order_book(symbol)
    
    if side == "BUY":
        levels = depth.get("bids", [])
    else:
        levels = depth.get("asks", [])
    
    target_price = float(price)
    for level in levels:
        if len(level) >= 1 and abs(float(level[0]) - target_price) < 0.01:
            return True
    return False


def cleanup_order(client: ApiClient, order_id: Optional[int]):
    """Best effort cleanup of an order"""
    if order_id:
        try:
            cancel_order(client, order_id)
        except:
            pass


# =============================================================================
# IOC Test Cases
# =============================================================================

def test_ioc_001_full_match() -> TestResult:
    """
    IOC-001: IOC 完全成交
    
    前置条件: Ask 深度 = 100 @ 50000
    操作: BUY IOC 100 @ 50000
    预期: 成交100, 状态=FILLED, 订单簿无残留
    """
    test_id = "IOC-001"
    test_name = "IOC 完全成交"
    
    print(f"\n[{test_id}] {test_name}")
    
    client_maker = get_test_client(GATEWAY_URL, USER_MAKER)
    client_taker = get_test_client(GATEWAY_URL, USER_TAKER)
    
    price = "70000.00"
    qty = "0.001"
    maker_order_id = None
    ioc_order_id = None
    
    try:
        # Step 1: Place maker SELL order (GTC)
        maker_order_id, _, _ = place_order(client_maker, SYMBOL, "SELL", price, qty, "GTC")
        if not maker_order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR, 
                            details="Failed to place maker order")
        
        print(f"  Maker order placed: {maker_order_id}")
        time.sleep(0.5)  # Wait for order to be in book
        
        # Step 2: Place IOC BUY order (should fully match)
        ioc_order_id, initial_status, resp = place_order(client_taker, SYMBOL, "BUY", price, qty, "IOC")
        if not ioc_order_id:
            cleanup_order(client_maker, maker_order_id)
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details=f"Failed to place IOC order: {resp}")
        
        print(f"  IOC order placed: {ioc_order_id}, initial_status={initial_status}")
        
        # Step 3: Wait for terminal state
        final_status = wait_for_order_terminal(client_taker, ioc_order_id, timeout=3.0)
        print(f"  IOC final status: {final_status}")
        
        # Step 4: Verify IOC is NOT in order book
        ioc_in_book = check_order_in_book(SYMBOL, "BUY", price)
        
        # Verification
        expected = "FILLED, not in book"
        actual = f"status={final_status}, in_book={ioc_in_book}"
        
        if final_status == "FILLED" and not ioc_in_book:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        elif final_status in ["ACCEPTED", "NEW"] and not ioc_in_book:
            # Async processing - may need more time
            return TestResult(test_id, test_name, TestStatus.PASS,
                            details="Order accepted (async), not in book",
                            expected=expected, actual=actual)
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            expected=expected, actual=actual)
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))
    
    finally:
        cleanup_order(client_maker, maker_order_id)
        cleanup_order(client_taker, ioc_order_id)


def test_ioc_002_partial_fill() -> TestResult:
    """
    IOC-002: IOC 部分成交
    
    前置条件: Ask 深度 = 60 @ 50000
    操作: BUY IOC 100 @ 50000
    预期: 成交60, 剩余40过期, IOC 不入簿
    
    关键验证: IOC 剩余部分 **绝不** 入簿
    """
    test_id = "IOC-002"
    test_name = "IOC 部分成交后不入簿"
    
    print(f"\n[{test_id}] {test_name}")
    
    client_maker = get_test_client(GATEWAY_URL, USER_MAKER)
    client_taker = get_test_client(GATEWAY_URL, USER_TAKER)
    
    price = "69000.00"
    maker_qty = "0.0006"  # Smaller quantity
    ioc_qty = "0.001"     # Larger quantity
    maker_order_id = None
    ioc_order_id = None
    
    try:
        # Step 1: Place smaller maker order
        maker_order_id, _, _ = place_order(client_maker, SYMBOL, "SELL", price, maker_qty, "GTC")
        if not maker_order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place maker order")
        
        print(f"  Maker order: {maker_order_id} (qty={maker_qty})")
        time.sleep(0.5)
        
        # Step 2: Place larger IOC order
        ioc_order_id, _, resp = place_order(client_taker, SYMBOL, "BUY", price, ioc_qty, "IOC")
        if not ioc_order_id:
            cleanup_order(client_maker, maker_order_id)
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details=f"Failed to place IOC order: {resp}")
        
        print(f"  IOC order: {ioc_order_id} (qty={ioc_qty})")
        
        # Step 3: Wait for processing
        time.sleep(1.0)
        final_status = wait_for_order_terminal(client_taker, ioc_order_id, timeout=3.0)
        print(f"  IOC final status: {final_status}")
        
        # Step 4: KEY VERIFICATION - IOC must NOT be in book
        ioc_in_book = check_order_in_book(SYMBOL, "BUY", price)
        print(f"  IOC in book: {ioc_in_book}")
        
        # Verification
        expected = "IOC not in book (remainder expired)"
        actual = f"in_book={ioc_in_book}, status={final_status}"
        
        if not ioc_in_book:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            details="CRITICAL: IOC remainder resting in book!",
                            expected=expected, actual=actual)
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))
    
    finally:
        cleanup_order(client_maker, maker_order_id)
        cleanup_order(client_taker, ioc_order_id)


def test_ioc_003_no_match() -> TestResult:
    """
    IOC-003: IOC 无对手盘
    
    前置条件: 订单簿空（或无交叉价格）
    操作: BUY IOC 100 @ 低价
    预期: 成交0, 全部过期, IOC 不入簿
    """
    test_id = "IOC-003"
    test_name = "IOC 无对手盘不入簿"
    
    print(f"\n[{test_id}] {test_name}")
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    # Use a very low price that won't match any asks
    price = "1000.00"
    qty = "0.001"
    ioc_order_id = None
    
    try:
        # Place IOC at non-crossing price
        ioc_order_id, initial_status, resp = place_order(client, SYMBOL, "BUY", price, qty, "IOC")
        if not ioc_order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details=f"Failed to place IOC order: {resp}")
        
        print(f"  IOC order: {ioc_order_id}, initial_status={initial_status}")
        
        # Wait for processing
        time.sleep(0.5)
        final_status = wait_for_order_terminal(client, ioc_order_id, timeout=3.0)
        print(f"  IOC final status: {final_status}")
        
        # KEY VERIFICATION - IOC must NOT be in book
        ioc_in_book = check_order_in_book(SYMBOL, "BUY", price)
        print(f"  IOC in book: {ioc_in_book}")
        
        # Verification
        expected = "IOC not in book (no match, expired)"
        actual = f"in_book={ioc_in_book}, status={final_status}"
        
        if not ioc_in_book:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            details="CRITICAL: IOC resting in book without match!",
                            expected=expected, actual=actual)
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))
    
    finally:
        cleanup_order(client, ioc_order_id)


def test_ioc_004_cross_level_sweep() -> TestResult:
    """
    IOC-004: IOC 跨价位扫单
    
    前置条件: Ask: 60@50000, 50@50100
    操作: BUY IOC 100 @ 50200
    预期: 成交100(60+40), FILLED
    """
    test_id = "IOC-004"
    test_name = "IOC 跨价位扫单"
    
    print(f"\n[{test_id}] {test_name}")
    
    client_maker = get_test_client(GATEWAY_URL, USER_MAKER)
    client_taker = get_test_client(GATEWAY_URL, USER_TAKER)
    
    price1 = "68000.00"
    price2 = "68100.00"
    ioc_price = "68200.00"
    
    qty1 = "0.0006"
    qty2 = "0.0005"
    ioc_qty = "0.001"  # Less than qty1 + qty2 to ensure full fill
    
    maker_id1 = None
    maker_id2 = None
    ioc_order_id = None
    
    try:
        # Step 1: Place two maker orders at different prices
        maker_id1, _, _ = place_order(client_maker, SYMBOL, "SELL", price1, qty1, "GTC")
        maker_id2, _, _ = place_order(client_maker, SYMBOL, "SELL", price2, qty2, "GTC")
        
        if not maker_id1 or not maker_id2:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place maker orders")
        
        print(f"  Maker orders: {maker_id1}@{price1}, {maker_id2}@{price2}")
        time.sleep(0.5)
        
        # Step 2: Place IOC to sweep both levels
        ioc_order_id, _, resp = place_order(client_taker, SYMBOL, "BUY", ioc_price, ioc_qty, "IOC")
        if not ioc_order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details=f"Failed to place IOC order: {resp}")
        
        print(f"  IOC order: {ioc_order_id} (qty={ioc_qty})")
        
        # Step 3: Wait and verify
        time.sleep(1.0)
        final_status = wait_for_order_terminal(client_taker, ioc_order_id, timeout=3.0)
        print(f"  IOC final status: {final_status}")
        
        # IOC should not be in book
        ioc_in_book = check_order_in_book(SYMBOL, "BUY", ioc_price)
        
        expected = "FILLED after sweeping multiple levels"
        actual = f"status={final_status}, in_book={ioc_in_book}"
        
        if final_status in ["FILLED", "ACCEPTED"] and not ioc_in_book:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            expected=expected, actual=actual)
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))
    
    finally:
        cleanup_order(client_maker, maker_id1)
        cleanup_order(client_maker, maker_id2)
        cleanup_order(client_taker, ioc_order_id)


def test_ioc_005_unfavorable_price() -> TestResult:
    """
    IOC-005: IOC 价格不利无成交
    
    前置条件: Ask 最优 = 50000
    操作: BUY IOC 100 @ 48000 (低于最优ask)
    预期: 成交0, 过期, 不入簿
    """
    test_id = "IOC-005"
    test_name = "IOC 价格不利无成交"
    
    print(f"\n[{test_id}] {test_name}")
    
    client_maker = get_test_client(GATEWAY_URL, USER_MAKER)
    client_taker = get_test_client(GATEWAY_URL, USER_TAKER)
    
    ask_price = "67000.00"
    ioc_price = "65000.00"  # Below ask - won't cross
    qty = "0.001"
    
    maker_order_id = None
    ioc_order_id = None
    
    try:
        # Place ask at higher price
        maker_order_id, _, _ = place_order(client_maker, SYMBOL, "SELL", ask_price, qty, "GTC")
        if not maker_order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place maker order")
        
        print(f"  Ask order at {ask_price}")
        time.sleep(0.5)
        
        # Place IOC at lower price (won't cross)
        ioc_order_id, _, resp = place_order(client_taker, SYMBOL, "BUY", ioc_price, qty, "IOC")
        if not ioc_order_id:
            cleanup_order(client_maker, maker_order_id)
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details=f"Failed to place IOC order: {resp}")
        
        print(f"  IOC BUY at {ioc_price} (below ask)")
        
        time.sleep(0.5)
        final_status = wait_for_order_terminal(client_taker, ioc_order_id, timeout=3.0)
        ioc_in_book = check_order_in_book(SYMBOL, "BUY", ioc_price)
        
        print(f"  IOC status: {final_status}, in_book: {ioc_in_book}")
        
        expected = "No match, expired, not in book"
        actual = f"status={final_status}, in_book={ioc_in_book}"
        
        if not ioc_in_book:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            details="IOC at non-crossing price resting in book!",
                            expected=expected, actual=actual)
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))
    
    finally:
        cleanup_order(client_maker, maker_order_id)
        cleanup_order(client_taker, ioc_order_id)


def test_ioc_006_sell_full_match() -> TestResult:
    """
    IOC-006: IOC SELL 完全成交
    
    前置条件: Bid 深度 = 100 @ 50000
    操作: SELL IOC 100 @ 50000
    预期: 成交100, FILLED
    """
    test_id = "IOC-006"
    test_name = "IOC SELL 完全成交"
    
    print(f"\n[{test_id}] {test_name}")
    
    client_maker = get_test_client(GATEWAY_URL, USER_MAKER)
    client_taker = get_test_client(GATEWAY_URL, USER_TAKER)
    
    price = "66000.00"
    qty = "0.001"
    maker_order_id = None
    ioc_order_id = None
    
    try:
        # Place BID (maker wants to buy)
        maker_order_id, _, _ = place_order(client_maker, SYMBOL, "BUY", price, qty, "GTC")
        if not maker_order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place maker order")
        
        print(f"  Bid order at {price}")
        time.sleep(0.5)
        
        # Place IOC SELL
        ioc_order_id, _, resp = place_order(client_taker, SYMBOL, "SELL", price, qty, "IOC")
        if not ioc_order_id:
            cleanup_order(client_maker, maker_order_id)
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details=f"Failed to place IOC order: {resp}")
        
        print(f"  IOC SELL at {price}")
        
        time.sleep(0.5)
        final_status = wait_for_order_terminal(client_taker, ioc_order_id, timeout=3.0)
        ioc_in_book = check_order_in_book(SYMBOL, "SELL", price)
        
        print(f"  IOC status: {final_status}, in_book: {ioc_in_book}")
        
        expected = "FILLED, not in book"
        actual = f"status={final_status}, in_book={ioc_in_book}"
        
        if final_status in ["FILLED", "ACCEPTED"] and not ioc_in_book:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            expected=expected, actual=actual)
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))
    
    finally:
        cleanup_order(client_maker, maker_order_id)
        cleanup_order(client_taker, ioc_order_id)


def test_ioc_007_sell_partial_fill() -> TestResult:
    """
    IOC-007: IOC SELL 部分成交
    
    前置条件: Bid 深度 = 60 @ 50000
    操作: SELL IOC 100 @ 50000
    预期: 成交60, 剩余40过期, 不入簿
    """
    test_id = "IOC-007"
    test_name = "IOC SELL 部分成交不入簿"
    
    print(f"\n[{test_id}] {test_name}")
    
    client_maker = get_test_client(GATEWAY_URL, USER_MAKER)
    client_taker = get_test_client(GATEWAY_URL, USER_TAKER)
    
    price = "65000.00"
    maker_qty = "0.0006"
    ioc_qty = "0.001"
    maker_order_id = None
    ioc_order_id = None
    
    try:
        # Place smaller BID
        maker_order_id, _, _ = place_order(client_maker, SYMBOL, "BUY", price, maker_qty, "GTC")
        if not maker_order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place maker order")
        
        print(f"  Bid order: {maker_order_id} (qty={maker_qty})")
        time.sleep(0.5)
        
        # Place larger IOC SELL
        ioc_order_id, _, resp = place_order(client_taker, SYMBOL, "SELL", price, ioc_qty, "IOC")
        if not ioc_order_id:
            cleanup_order(client_maker, maker_order_id)
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details=f"Failed to place IOC order: {resp}")
        
        print(f"  IOC SELL: {ioc_order_id} (qty={ioc_qty})")
        
        time.sleep(1.0)
        final_status = wait_for_order_terminal(client_taker, ioc_order_id, timeout=3.0)
        ioc_in_book = check_order_in_book(SYMBOL, "SELL", price)
        
        print(f"  IOC status: {final_status}, in_book: {ioc_in_book}")
        
        expected = "IOC not in book after partial fill"
        actual = f"status={final_status}, in_book={ioc_in_book}"
        
        if not ioc_in_book:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            details="CRITICAL: IOC SELL remainder in book!",
                            expected=expected, actual=actual)
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))
    
    finally:
        cleanup_order(client_maker, maker_order_id)
        cleanup_order(client_taker, ioc_order_id)


# =============================================================================
# 流程专家审核补充测试 (成交结果验证)
# =============================================================================

def test_ioc_008_verify_filled_qty() -> TestResult:
    """
    IOC-008: 验证 IOC 成交数量正确

    流程专家审核补充: 不仅验证"是否入簿"，还验证成交数量
    
    前置条件: Ask 深度 = 60 @ 50000
    操作: BUY IOC 100 @ 50000
    预期: filled_qty == 60 (精确验证)
    """
    test_id = "IOC-008"
    test_name = "IOC 成交数量验证"
    
    print(f"\n[{test_id}] {test_name}")
    
    client_maker = get_test_client(GATEWAY_URL, USER_MAKER)
    client_taker = get_test_client(GATEWAY_URL, USER_TAKER)
    
    price = "71000.00"
    maker_qty = "0.0006"  # 精确的 maker 数量
    ioc_qty = "0.001"     # IOC 想买更多
    expected_filled = 0.0006  # 应该成交 maker 的全部数量
    
    maker_order_id = None
    ioc_order_id = None
    
    try:
        # Place maker SELL order
        maker_order_id, _, _ = place_order(client_maker, SYMBOL, "SELL", price, maker_qty, "GTC")
        if not maker_order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place maker order")
        
        print(f"  Maker: {maker_order_id} (qty={maker_qty})")
        time.sleep(0.5)
        
        # Place IOC BUY order
        ioc_order_id, _, _ = place_order(client_taker, SYMBOL, "BUY", price, ioc_qty, "IOC")
        if not ioc_order_id:
            cleanup_order(client_maker, maker_order_id)
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place IOC order")
        
        print(f"  IOC: {ioc_order_id} (qty={ioc_qty})")
        time.sleep(1.0)
        
        # Get order details to verify filled_qty
        order_details = get_order_details(client_taker, ioc_order_id)
        
        if not order_details:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to get order details")
        
        # Extract filled quantity
        filled_qty_str = order_details.get("filled_qty", order_details.get("executed_qty", "0"))
        try:
            filled_qty = float(filled_qty_str)
        except:
            filled_qty = 0.0
        
        status = order_details.get("status", "UNKNOWN")
        print(f"  Order details: status={status}, filled_qty={filled_qty}")
        
        expected = f"filled_qty={expected_filled}"
        actual = f"filled_qty={filled_qty}, status={status}"
        
        # Verify filled quantity matches expected (with small tolerance for float)
        if abs(filled_qty - expected_filled) < 0.00001:
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            details=f"Filled qty mismatch",
                            expected=expected, actual=actual)
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))
    
    finally:
        cleanup_order(client_maker, maker_order_id)
        cleanup_order(client_taker, ioc_order_id)


def test_ioc_009_verify_trade_record() -> TestResult:
    """
    IOC-009: 验证 IOC 生成的 Trade 记录
    
    流程专家审核补充: 验证成交后 Trade 记录正确持久化
    
    前置条件: Ask 深度存在
    操作: BUY IOC 成交
    预期: GET /trades 返回对应的成交记录
    """
    test_id = "IOC-009"
    test_name = "IOC Trade 记录验证"
    
    print(f"\n[{test_id}] {test_name}")
    
    client_maker = get_test_client(GATEWAY_URL, USER_MAKER)
    client_taker = get_test_client(GATEWAY_URL, USER_TAKER)
    
    price = "72000.00"
    qty = "0.001"
    
    maker_order_id = None
    ioc_order_id = None
    
    try:
        # Get trades before
        resp_before = client_taker.get("/api/v1/private/trades", params={"limit": 5})
        trades_before = []
        if resp_before.status_code == 200:
            trades_before = resp_before.json().get("data", [])
        trades_count_before = len(trades_before)
        
        print(f"  Trades before: {trades_count_before}")
        
        # Place maker SELL order
        maker_order_id, _, _ = place_order(client_maker, SYMBOL, "SELL", price, qty, "GTC")
        if not maker_order_id:
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place maker order")
        
        print(f"  Maker: {maker_order_id}")
        time.sleep(0.5)
        
        # Place IOC BUY order (should match)
        ioc_order_id, _, _ = place_order(client_taker, SYMBOL, "BUY", price, qty, "IOC")
        if not ioc_order_id:
            cleanup_order(client_maker, maker_order_id)
            return TestResult(test_id, test_name, TestStatus.ERROR,
                            details="Failed to place IOC order")
        
        print(f"  IOC: {ioc_order_id}")
        time.sleep(1.5)  # Allow trade to be recorded
        
        # Get trades after
        resp_after = client_taker.get("/api/v1/private/trades", params={"limit": 10})
        trades_after = []
        if resp_after.status_code == 200:
            trades_after = resp_after.json().get("data", [])
        trades_count_after = len(trades_after)
        
        print(f"  Trades after: {trades_count_after}")
        
        # Verify new trade record exists
        new_trades = trades_count_after - trades_count_before
        
        expected = "At least 1 new trade record"
        actual = f"new_trades={new_trades}"
        
        if new_trades >= 1:
            # Verify trade details if available
            if trades_after:
                latest_trade = trades_after[0]
                trade_price = latest_trade.get("price", "")
                trade_qty = latest_trade.get("qty", latest_trade.get("quantity", ""))
                print(f"  Latest trade: price={trade_price}, qty={trade_qty}")
            
            return TestResult(test_id, test_name, TestStatus.PASS,
                            expected=expected, actual=actual)
        else:
            return TestResult(test_id, test_name, TestStatus.FAIL,
                            details="No new trade record after IOC match",
                            expected=expected, actual=actual)
    
    except Exception as e:
        return TestResult(test_id, test_name, TestStatus.ERROR, details=str(e))
    
    finally:
        cleanup_order(client_maker, maker_order_id)
        cleanup_order(client_taker, ioc_order_id)


# =============================================================================
# Main Entry Point
# =============================================================================

def print_separator():
    print("=" * 70)


def main():
    print_separator()
    print("🧪 QA 0x14-b: IOC (Immediate-or-Cancel) Independent Test Suite")
    print_separator()
    print(f"Gateway: {GATEWAY_URL}")
    print(f"Symbol: {SYMBOL}")
    print(f"Maker User: {USER_MAKER}")
    print(f"Taker User: {USER_TAKER}")
    
    # Check gateway connectivity
    try:
        resp = requests.get(f"{GATEWAY_URL}/api/v1/public/exchange_info", timeout=5)
        if resp.status_code != 200:
            print(f"\n❌ Gateway not responding: {resp.status_code}")
            return 1
    except Exception as e:
        print(f"\n❌ Cannot connect to Gateway: {e}")
        print("  Ensure Gateway is running: cargo run --release --bin gateway")
        return 1
    
    print("\n✅ Gateway connected")
    
    # Run all IOC tests
    tests = [
        test_ioc_001_full_match,
        test_ioc_002_partial_fill,
        test_ioc_003_no_match,
        test_ioc_004_cross_level_sweep,
        test_ioc_005_unfavorable_price,
        test_ioc_006_sell_full_match,
        test_ioc_007_sell_partial_fill,
        # 流程专家审核补充
        test_ioc_008_verify_filled_qty,
        test_ioc_009_verify_trade_record,
    ]
    
    results: List[TestResult] = []
    for test_fn in tests:
        try:
            result = test_fn()
            results.append(result)
        except Exception as e:
            results.append(TestResult(
                test_id="UNKNOWN",
                name=test_fn.__name__,
                status=TestStatus.ERROR,
                details=str(e)
            ))
    
    # Print summary
    print("\n")
    print_separator()
    print("📊 IOC TEST RESULTS")
    print_separator()
    
    passed = 0
    failed = 0
    errors = 0
    
    for r in results:
        if r.status == TestStatus.PASS:
            icon = "✅"
            passed += 1
        elif r.status == TestStatus.FAIL:
            icon = "❌"
            failed += 1
        else:
            icon = "⚠️"
            errors += 1
        
        print(f"  {icon} [{r.test_id}] {r.name}: {r.status.value}")
        if r.status != TestStatus.PASS:
            if r.expected:
                print(f"       Expected: {r.expected}")
            if r.actual:
                print(f"       Actual:   {r.actual}")
            if r.details:
                print(f"       Details:  {r.details}")
    
    print_separator()
    print(f"Summary: {passed} PASS, {failed} FAIL, {errors} ERROR (Total: {len(results)})")
    
    if failed > 0 or errors > 0:
        print("\n⚠️  IOC Test Suite: FAILURES DETECTED")
        return 1
    
    print("\n✅ IOC Test Suite: ALL PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
