#!/usr/bin/env python3
"""
🔬 Symbol Filters Test Suite

验证 exchange_info API 返回完整的 symbol 配置信息。
对比 Binance 标准，检查是否包含必要的 filters 和 order_types。

测试范围:
- FILTER-001: exchange_info 返回 symbols 数组
- FILTER-002: 每个 symbol 包含 base_asset, quote_asset
- FILTER-003: 检查是否有 filters 字段
- FILTER-004: 检查是否有 order_types 字段
- FILTER-005: 验证 LOT_SIZE 限制 (minQty, maxQty)
- FILTER-006: 验证 PRICE_FILTER 限制 (minPrice, maxPrice, tickSize)
- FILTER-007: 验证 NOTIONAL 限制 (minNotional)
"""

import sys
import os

# 路径设置
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
SCRIPTS_ROOT = os.path.dirname(os.path.dirname(SCRIPT_DIR))
sys.path.insert(0, SCRIPTS_ROOT)

from conftest import (
    TestStatus, TestResult, collector,
    GATEWAY_URL, SYMBOL,
    get_exchange_info, health_check
)


# =============================================================================
# FILTER-001~002: 基础结构验证
# =============================================================================

def test_exchange_info_structure():
    """验证 exchange_info 返回正确的结构"""
    
    print("\n" + "=" * 60)
    print("📋 FILTER-001/002: exchange_info 结构验证")
    print("=" * 60)
    
    exchange_info = get_exchange_info()
    
    # FILTER-001: symbols 数组存在
    test_id = "FILTER-001"
    if exchange_info and "symbols" in exchange_info:
        symbols = exchange_info.get("symbols", [])
        if len(symbols) > 0:
            collector.add(TestResult(test_id, "exchange_info 包含 symbols 数组", TestStatus.PASS,
                                    details=f"Found {len(symbols)} symbols"))
        else:
            collector.add(TestResult(test_id, "exchange_info 包含 symbols 数组", TestStatus.FAIL,
                                    details="symbols array is empty"))
    else:
        collector.add(TestResult(test_id, "exchange_info 包含 symbols 数组", TestStatus.FAIL,
                                details="symbols field missing"))
        return
    
    # FILTER-002: 每个 symbol 包含 base_asset, quote_asset
    test_id = "FILTER-002"
    symbols = exchange_info.get("symbols", [])
    missing_fields = []
    
    for sym in symbols:
        symbol_name = sym.get("symbol", "UNKNOWN")
        if "base_asset" not in sym:
            missing_fields.append(f"{symbol_name}: base_asset")
        if "quote_asset" not in sym:
            missing_fields.append(f"{symbol_name}: quote_asset")
    
    if not missing_fields:
        collector.add(TestResult(test_id, "symbols 包含 base/quote_asset", TestStatus.PASS))
    else:
        collector.add(TestResult(test_id, "symbols 包含 base/quote_asset", TestStatus.FAIL,
                                details=", ".join(missing_fields)))


# =============================================================================
# FILTER-003~004: Binance 兼容字段检查
# =============================================================================

def test_binance_compatible_fields():
    """检查 Binance 兼容的 filters 和 order_types 字段"""
    
    print("\n" + "=" * 60)
    print("📋 FILTER-003/004: Binance 兼容字段检查")
    print("=" * 60)
    
    exchange_info = get_exchange_info()
    if not exchange_info:
        collector.add(TestResult("FILTER-003", "symbols 包含 filters", TestStatus.SKIP,
                                details="exchange_info not available"))
        return
    
    symbols = exchange_info.get("symbols", [])
    target_symbol = None
    
    for sym in symbols:
        if sym.get("symbol") == SYMBOL:
            target_symbol = sym
            break
    
    if not target_symbol:
        collector.add(TestResult("FILTER-003", "symbols 包含 filters", TestStatus.SKIP,
                                details=f"Symbol {SYMBOL} not found"))
        return
    
    # FILTER-003: filters 字段存在
    test_id = "FILTER-003"
    if "filters" in target_symbol:
        filters = target_symbol.get("filters", [])
        collector.add(TestResult(test_id, "symbols 包含 filters", TestStatus.PASS,
                                details=f"Found {len(filters)} filters"))
    else:
        collector.add(TestResult(test_id, "symbols 包含 filters", TestStatus.FAIL,
                                expected="filters array",
                                actual="field missing",
                                details="Binance 兼容需要 filters 数组"))
    
    # FILTER-004: order_types 字段存在
    test_id = "FILTER-004"
    if "order_types" in target_symbol or "orderTypes" in target_symbol:
        order_types = target_symbol.get("order_types") or target_symbol.get("orderTypes", [])
        collector.add(TestResult(test_id, "symbols 包含 order_types", TestStatus.PASS,
                                details=f"Types: {order_types}"))
    else:
        collector.add(TestResult(test_id, "symbols 包含 order_types", TestStatus.FAIL,
                                expected="order_types array",
                                actual="field missing",
                                details="Binance 兼容需要 order_types 数组"))


# =============================================================================
# FILTER-005~007: 具体 Filter 验证
# =============================================================================

def test_filter_details():
    """验证具体的 Filter 内容 (LOT_SIZE, PRICE_FILTER, NOTIONAL)"""
    
    print("\n" + "=" * 60)
    print("📋 FILTER-005~007: Filter 详细内容验证")
    print("=" * 60)
    
    exchange_info = get_exchange_info()
    if not exchange_info:
        for test_id in ["FILTER-005", "FILTER-006", "FILTER-007"]:
            collector.add(TestResult(test_id, "Filter 验证", TestStatus.SKIP,
                                    details="exchange_info not available"))
        return
    
    symbols = exchange_info.get("symbols", [])
    target_symbol = None
    
    for sym in symbols:
        if sym.get("symbol") == SYMBOL:
            target_symbol = sym
            break
    
    if not target_symbol or "filters" not in target_symbol:
        for test_id in ["FILTER-005", "FILTER-006", "FILTER-007"]:
            collector.add(TestResult(test_id, "Filter 验证", TestStatus.SKIP,
                                    details="filters field not found"))
        return
    
    filters = target_symbol.get("filters", [])
    filter_map = {f.get("filterType"): f for f in filters}
    
    # FILTER-005: LOT_SIZE
    test_id = "FILTER-005"
    if "LOT_SIZE" in filter_map:
        lot_size = filter_map["LOT_SIZE"]
        required = ["minQty", "maxQty", "stepSize"]
        missing = [k for k in required if k not in lot_size]
        
        if not missing:
            collector.add(TestResult(test_id, "LOT_SIZE 完整", TestStatus.PASS,
                                    details=f"minQty={lot_size.get('minQty')}, maxQty={lot_size.get('maxQty')}"))
        else:
            collector.add(TestResult(test_id, "LOT_SIZE 完整", TestStatus.FAIL,
                                    details=f"Missing: {missing}"))
    else:
        collector.add(TestResult(test_id, "LOT_SIZE 完整", TestStatus.FAIL,
                                expected="LOT_SIZE filter",
                                actual="not found"))
    
    # FILTER-006: PRICE_FILTER
    test_id = "FILTER-006"
    if "PRICE_FILTER" in filter_map:
        price_filter = filter_map["PRICE_FILTER"]
        required = ["minPrice", "maxPrice", "tickSize"]
        missing = [k for k in required if k not in price_filter]
        
        if not missing:
            collector.add(TestResult(test_id, "PRICE_FILTER 完整", TestStatus.PASS,
                                    details=f"tickSize={price_filter.get('tickSize')}"))
        else:
            collector.add(TestResult(test_id, "PRICE_FILTER 完整", TestStatus.FAIL,
                                    details=f"Missing: {missing}"))
    else:
        collector.add(TestResult(test_id, "PRICE_FILTER 完整", TestStatus.FAIL,
                                expected="PRICE_FILTER filter",
                                actual="not found"))
    
    # FILTER-007: NOTIONAL (MIN_NOTIONAL)
    test_id = "FILTER-007"
    if "NOTIONAL" in filter_map or "MIN_NOTIONAL" in filter_map:
        notional = filter_map.get("NOTIONAL") or filter_map.get("MIN_NOTIONAL")
        if "minNotional" in notional:
            collector.add(TestResult(test_id, "NOTIONAL 完整", TestStatus.PASS,
                                    details=f"minNotional={notional.get('minNotional')}"))
        else:
            collector.add(TestResult(test_id, "NOTIONAL 完整", TestStatus.FAIL,
                                    details="minNotional missing"))
    else:
        collector.add(TestResult(test_id, "NOTIONAL 完整", TestStatus.FAIL,
                                expected="NOTIONAL or MIN_NOTIONAL filter",
                                actual="not found"))


# =============================================================================
# 主执行入口
# =============================================================================

def run_all_filter_tests():
    """运行所有 Symbol Filter 测试"""
    print("=" * 80)
    print("🔬 Symbol Filters Test Suite")
    print("=" * 80)
    print("\n目标: 验证 exchange_info 包含 Binance 兼容的 symbol filters")
    
    if not health_check():
        print("❌ Gateway not available!")
        return 1
    
    test_exchange_info_structure()
    test_binance_compatible_fields()
    test_filter_details()
    
    collector.print_summary()
    
    return 0 if collector.all_passed else 1


if __name__ == "__main__":
    sys.exit(run_all_filter_tests())
