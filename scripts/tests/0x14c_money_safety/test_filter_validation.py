#!/usr/bin/env python3
"""
🔬 Filter Validation Test Suite

验证 Gateway 在下单时正确执行 Filter 限制。
基于 Binance 标准，测试以下验证规则：

LOT_SIZE 规则:
- VAL-001: qty >= minQty
- VAL-002: qty <= maxQty  
- VAL-003: (qty - minQty) % stepSize == 0

PRICE_FILTER 规则:
- VAL-004: price >= minPrice
- VAL-005: (price - minPrice) % tickSize == 0

NOTIONAL 规则:
- VAL-006: price * qty >= minNotional

注意: 这些测试依赖 exchange_info 返回 filters 字段。
如果 filters 尚未实现，测试会 SKIP。
"""

import sys
import os

# 路径设置
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
SCRIPTS_ROOT = os.path.dirname(os.path.dirname(SCRIPT_DIR))
sys.path.insert(0, SCRIPTS_ROOT)

from decimal import Decimal
from conftest import (
    TestStatus, TestResult, collector,
    GATEWAY_URL, SYMBOL, USER_TAKER,
    get_test_client, get_exchange_info, health_check
)


def get_filters_for_symbol(symbol: str) -> dict:
    """从 exchange_info 获取指定 symbol 的 filters"""
    exchange_info = get_exchange_info()
    if not exchange_info:
        return {}
    
    symbols = exchange_info.get("symbols", [])
    for sym in symbols:
        if sym.get("symbol") == symbol:
            filters = sym.get("filters", [])
            return {f.get("filterType"): f for f in filters}
    
    return {}


# =============================================================================
# VAL-001~003: LOT_SIZE 验证
# =============================================================================

def test_lot_size_validation():
    """验证 LOT_SIZE filter 是否被正确执行"""
    
    print("\n" + "=" * 60)
    print("📋 VAL-001~003: LOT_SIZE 验证")
    print("=" * 60)
    
    filters = get_filters_for_symbol(SYMBOL)
    
    if "LOT_SIZE" not in filters:
        for test_id in ["VAL-001", "VAL-002", "VAL-003"]:
            collector.add(TestResult(test_id, "LOT_SIZE 验证", TestStatus.SKIP,
                                    details="LOT_SIZE filter not implemented"))
        return
    
    lot_size = filters["LOT_SIZE"]
    min_qty = Decimal(lot_size.get("minQty", "0"))
    max_qty = Decimal(lot_size.get("maxQty", "999999"))
    step_size = Decimal(lot_size.get("stepSize", "0.00001"))
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    # VAL-001: qty < minQty 应该被拒绝
    test_id = "VAL-001"
    try:
        tiny_qty = str(min_qty / 10)  # 小于 minQty
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "85000.00",
            "qty": tiny_qty,
            "time_in_force": "GTC",
        })
        
        if resp.status_code == 400:
            collector.add(TestResult(test_id, "qty < minQty rejected", TestStatus.PASS,
                                    details=f"qty={tiny_qty} rejected"))
        elif resp.status_code in [200, 202]:
            collector.add(TestResult(test_id, "qty < minQty rejected", TestStatus.FAIL,
                                    expected="400 rejection",
                                    actual=f"{resp.status_code} accepted"))
        else:
            collector.add(TestResult(test_id, "qty < minQty rejected", TestStatus.SKIP,
                                    details=f"Unexpected: {resp.status_code}"))
    except Exception as e:
        collector.add(TestResult(test_id, "qty < minQty rejected", TestStatus.ERROR, str(e)))
    
    # VAL-002: qty > maxQty 应该被拒绝
    test_id = "VAL-002"
    try:
        huge_qty = str(max_qty * 2)  # 大于 maxQty
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "85000.00",
            "qty": huge_qty,
            "time_in_force": "GTC",
        })
        
        if resp.status_code == 400:
            collector.add(TestResult(test_id, "qty > maxQty rejected", TestStatus.PASS,
                                    details=f"qty={huge_qty} rejected"))
        elif resp.status_code in [200, 202]:
            collector.add(TestResult(test_id, "qty > maxQty rejected", TestStatus.FAIL,
                                    expected="400 rejection",
                                    actual=f"{resp.status_code} accepted"))
        else:
            collector.add(TestResult(test_id, "qty > maxQty rejected", TestStatus.SKIP,
                                    details=f"Unexpected: {resp.status_code}"))
    except Exception as e:
        collector.add(TestResult(test_id, "qty > maxQty rejected", TestStatus.ERROR, str(e)))
    
    # VAL-003: qty 不符合 stepSize 应该被拒绝
    test_id = "VAL-003"
    try:
        # 生成不符合 stepSize 的 qty
        bad_qty = str(min_qty + step_size / 2)
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "85000.00",
            "qty": bad_qty,
            "time_in_force": "GTC",
        })
        
        if resp.status_code == 400:
            collector.add(TestResult(test_id, "qty % stepSize != 0 rejected", TestStatus.PASS,
                                    details=f"qty={bad_qty} rejected"))
        elif resp.status_code in [200, 202]:
            collector.add(TestResult(test_id, "qty % stepSize != 0 rejected", TestStatus.FAIL,
                                    expected="400 rejection",
                                    actual=f"{resp.status_code} accepted"))
        else:
            collector.add(TestResult(test_id, "qty % stepSize != 0 rejected", TestStatus.SKIP,
                                    details=f"Unexpected: {resp.status_code}"))
    except Exception as e:
        collector.add(TestResult(test_id, "qty % stepSize != 0 rejected", TestStatus.ERROR, str(e)))


# =============================================================================
# VAL-004~005: PRICE_FILTER 验证
# =============================================================================

def test_price_filter_validation():
    """验证 PRICE_FILTER 是否被正确执行"""
    
    print("\n" + "=" * 60)
    print("📋 VAL-004~005: PRICE_FILTER 验证")
    print("=" * 60)
    
    filters = get_filters_for_symbol(SYMBOL)
    
    if "PRICE_FILTER" not in filters:
        for test_id in ["VAL-004", "VAL-005"]:
            collector.add(TestResult(test_id, "PRICE_FILTER 验证", TestStatus.SKIP,
                                    details="PRICE_FILTER not implemented"))
        return
    
    price_filter = filters["PRICE_FILTER"]
    min_price = Decimal(price_filter.get("minPrice", "0"))
    tick_size = Decimal(price_filter.get("tickSize", "0.01"))
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    # VAL-004: price < minPrice 应该被拒绝
    test_id = "VAL-004"
    if min_price > 0:
        try:
            tiny_price = str(min_price / 10)
            resp = client.post("/api/v1/private/order", {
                "symbol": SYMBOL,
                "side": "BUY",
                "order_type": "LIMIT",
                "price": tiny_price,
                "qty": "0.001",
                "time_in_force": "GTC",
            })
            
            if resp.status_code == 400:
                collector.add(TestResult(test_id, "price < minPrice rejected", TestStatus.PASS,
                                        details=f"price={tiny_price} rejected"))
            elif resp.status_code in [200, 202]:
                collector.add(TestResult(test_id, "price < minPrice rejected", TestStatus.FAIL,
                                        expected="400 rejection",
                                        actual=f"{resp.status_code} accepted"))
            else:
                collector.add(TestResult(test_id, "price < minPrice rejected", TestStatus.SKIP,
                                        details=f"Unexpected: {resp.status_code}"))
        except Exception as e:
            collector.add(TestResult(test_id, "price < minPrice rejected", TestStatus.ERROR, str(e)))
    else:
        collector.add(TestResult(test_id, "price < minPrice rejected", TestStatus.SKIP,
                                details="minPrice is 0 (disabled)"))
    
    # VAL-005: price 不符合 tickSize 应该被拒绝
    test_id = "VAL-005"
    try:
        bad_price = str(Decimal("85000") + tick_size / 2)
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": bad_price,
            "qty": "0.001",
            "time_in_force": "GTC",
        })
        
        if resp.status_code == 400:
            collector.add(TestResult(test_id, "price % tickSize != 0 rejected", TestStatus.PASS,
                                    details=f"price={bad_price} rejected"))
        elif resp.status_code in [200, 202]:
            collector.add(TestResult(test_id, "price % tickSize != 0 rejected", TestStatus.FAIL,
                                    expected="400 rejection",
                                    actual=f"{resp.status_code} accepted"))
        else:
            collector.add(TestResult(test_id, "price % tickSize != 0 rejected", TestStatus.SKIP,
                                    details=f"Unexpected: {resp.status_code}"))
    except Exception as e:
        collector.add(TestResult(test_id, "price % tickSize != 0 rejected", TestStatus.ERROR, str(e)))


# =============================================================================
# VAL-006: NOTIONAL 验证
# =============================================================================

def test_notional_validation():
    """验证 NOTIONAL filter 是否被正确执行"""
    
    print("\n" + "=" * 60)
    print("📋 VAL-006: NOTIONAL 验证")
    print("=" * 60)
    
    filters = get_filters_for_symbol(SYMBOL)
    
    notional = filters.get("NOTIONAL") or filters.get("MIN_NOTIONAL")
    
    if not notional:
        collector.add(TestResult("VAL-006", "NOTIONAL 验证", TestStatus.SKIP,
                                details="NOTIONAL filter not implemented"))
        return
    
    min_notional = Decimal(notional.get("minNotional", "0"))
    
    if min_notional == 0:
        collector.add(TestResult("VAL-006", "NOTIONAL 验证", TestStatus.SKIP,
                                details="minNotional is 0 (disabled)"))
        return
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    # VAL-006: price * qty < minNotional 应该被拒绝
    test_id = "VAL-006"
    try:
        # 使用很小的价格和数量，确保 notional < minNotional
        small_price = "1.00"
        small_qty = "0.00001"
        notional_value = Decimal(small_price) * Decimal(small_qty)
        
        if notional_value >= min_notional:
            small_qty = str(min_notional / Decimal("10000"))  # 确保足够小
        
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": small_price,
            "qty": small_qty,
            "time_in_force": "GTC",
        })
        
        if resp.status_code == 400:
            collector.add(TestResult(test_id, "notional < minNotional rejected", TestStatus.PASS,
                                    details=f"price*qty < {min_notional} rejected"))
        elif resp.status_code in [200, 202]:
            collector.add(TestResult(test_id, "notional < minNotional rejected", TestStatus.FAIL,
                                    expected="400 rejection",
                                    actual=f"{resp.status_code} accepted"))
        else:
            collector.add(TestResult(test_id, "notional < minNotional rejected", TestStatus.SKIP,
                                    details=f"Unexpected: {resp.status_code}"))
    except Exception as e:
        collector.add(TestResult(test_id, "notional < minNotional rejected", TestStatus.ERROR, str(e)))


# =============================================================================
# 主执行入口
# =============================================================================

def run_all_validation_tests():
    """运行所有 Filter 验证测试"""
    print("=" * 80)
    print("🔬 Filter Validation Test Suite")
    print("=" * 80)
    print("\n目标: 验证 Gateway 正确执行 symbol filters")
    
    if not health_check():
        print("❌ Gateway not available!")
        return 1
    
    test_lot_size_validation()
    test_price_filter_validation()
    test_notional_validation()
    
    collector.print_summary()
    
    return 0 if collector.all_passed else 1


if __name__ == "__main__":
    sys.exit(run_all_validation_tests())
