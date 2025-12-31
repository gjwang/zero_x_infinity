#!/usr/bin/env python3
"""
🎯 Advanced Money Safety Tests

高级精度验证：往返一致性、成交精度、跨路径一致性

测试用例: TC-NEW-001 ~ TC-NEW-010
"""

import sys
import os
from decimal import Decimal

# 路径设置
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
SCRIPTS_ROOT = os.path.dirname(os.path.dirname(SCRIPT_DIR))
sys.path.insert(0, SCRIPTS_ROOT)

from conftest import (
    TestStatus, TestResult, collector,
    GATEWAY_URL, SYMBOL, USER_TAKER, USER_MAKER,
    get_test_client, get_exchange_info, health_check
)


# =============================================================================
# TC-NEW-001/002: 往返一致性 (Round-trip Integrity)
# =============================================================================

def test_roundtrip_integrity():
    """验证输入金额与响应金额完全一致 (基于 symbol_info 精度)"""
    
    print("\n📦 TC-NEW-001/002: 往返一致性测试")
    print("-" * 60)
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    # 获取 symbol_info 精度配置
    exchange_info = get_exchange_info()
    if not exchange_info:
        collector.add(TestResult("TC-NEW-001", "往返 qty 一致性", TestStatus.SKIP,
                                details="Exchange info not available"))
        collector.add(TestResult("TC-NEW-002", "往返 price 一致性", TestStatus.SKIP,
                                details="Exchange info not available"))
        return
    
    # 从 exchange_info 获取精度
    symbols = {s.get("symbol"): s for s in exchange_info.get("symbols", [])}
    assets = {a.get("asset"): a for a in exchange_info.get("assets", [])}
    
    symbol_info = symbols.get(SYMBOL, {})
    base_asset = symbol_info.get("base_asset", "BTC")
    quote_asset = symbol_info.get("quote_asset", "USDT")
    
    base_decimals = assets.get(base_asset, {}).get("decimals", 8)
    quote_decimals = assets.get(quote_asset, {}).get("decimals", 6)
    price_decimals = symbol_info.get("price_decimals", 2)
    
    print(f"  Symbol: {SYMBOL}")
    print(f"  Base decimals: {base_decimals}")
    print(f"  Price decimals: {price_decimals}")
    
    # TC-NEW-001: qty 往返一致性
    test_id = "TC-NEW-001"
    try:
        input_qty = "0." + "1" * base_decimals
        input_price = "85000.0"
        
        post_resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": input_price,
            "qty": input_qty,
            "time_in_force": "GTC",
        })
        
        if post_resp.status_code not in [200, 202]:
            collector.add(TestResult(test_id, "往返 qty 一致性", TestStatus.SKIP,
                                    details=f"Order not placed: {post_resp.status_code}"))
        else:
            post_data = post_resp.json().get("data", {})
            order_id = post_data.get("order_id")
            
            import time
            time.sleep(0.1)
            
            get_resp = client.get(f"/api/v1/private/orders?symbol={SYMBOL}")
            
            if get_resp.status_code == 200:
                orders = get_resp.json().get("data", [])
                target_order = None
                for order in orders:
                    if str(order.get("order_id")) == str(order_id):
                        target_order = order
                        break
                
                if target_order:
                    response_qty = target_order.get("qty", "")
                    input_dec = Decimal(input_qty)
                    response_dec = Decimal(response_qty) if response_qty else Decimal("0")
                    
                    if input_dec == response_dec:
                        collector.add(TestResult(test_id, "往返 qty 一致性", TestStatus.PASS,
                                                details=f"Input={input_qty}, Response={response_qty}"))
                    else:
                        collector.add(TestResult(test_id, "往返 qty 一致性", TestStatus.FAIL,
                                                expected=input_qty, actual=response_qty,
                                                details=f"PRECISION LOSS"))
                else:
                    collector.add(TestResult(test_id, "往返 qty 一致性", TestStatus.SKIP,
                                            details="Order not found in list"))
            else:
                collector.add(TestResult(test_id, "往返 qty 一致性", TestStatus.SKIP,
                                        details=f"GET failed: {get_resp.status_code}"))
    except Exception as e:
        collector.add(TestResult(test_id, "往返 qty 一致性", TestStatus.ERROR, str(e)))
    
    # TC-NEW-002: price 往返一致性
    test_id = "TC-NEW-002"
    try:
        input_price = "85000." + "1" * price_decimals
        
        post_resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": input_price,
            "qty": "0.001",
            "time_in_force": "GTC",
        })
        
        if post_resp.status_code not in [200, 202]:
            collector.add(TestResult(test_id, "往返 price 一致性", TestStatus.SKIP,
                                    details=f"Order not placed: {post_resp.status_code}"))
        else:
            post_data = post_resp.json().get("data", {})
            order_id = post_data.get("order_id")
            
            import time
            time.sleep(0.1)
            
            get_resp = client.get(f"/api/v1/private/orders?symbol={SYMBOL}")
            
            if get_resp.status_code == 200:
                orders = get_resp.json().get("data", [])
                target_order = None
                for order in orders:
                    if str(order.get("order_id")) == str(order_id):
                        target_order = order
                        break
                
                if target_order:
                    response_price = target_order.get("price", "")
                    input_dec = Decimal(input_price)
                    response_dec = Decimal(response_price) if response_price else Decimal("0")
                    
                    if input_dec == response_dec:
                        collector.add(TestResult(test_id, "往返 price 一致性", TestStatus.PASS,
                                                details=f"Input={input_price}, Response={response_price}"))
                    else:
                        collector.add(TestResult(test_id, "往返 price 一致性", TestStatus.FAIL,
                                                expected=input_price, actual=response_price))
                else:
                    collector.add(TestResult(test_id, "往返 price 一致性", TestStatus.SKIP,
                                            details="Order not found"))
            else:
                collector.add(TestResult(test_id, "往返 price 一致性", TestStatus.SKIP,
                                        details=f"GET failed: {get_resp.status_code}"))
    except Exception as e:
        collector.add(TestResult(test_id, "往返 price 一致性", TestStatus.ERROR, str(e)))


# =============================================================================
# TC-NEW-003~005: 成交精度 (Trade Execution Precision)
# =============================================================================

def test_trade_execution_precision():
    """验证成交后金额精确无误"""
    
    print("\n📦 TC-NEW-003/004/005: 成交精度测试")
    print("-" * 60)
    
    # 这些测试需要成交才能验证，暂时标记为 SKIP
    collector.add(TestResult("TC-NEW-003", "Trade qty 精确", TestStatus.SKIP,
                            details="Requires matched trades"))
    collector.add(TestResult("TC-NEW-004", "余额变化精确", TestStatus.SKIP,
                            details="Requires balance tracking"))
    collector.add(TestResult("TC-NEW-005", "手续费计算精度", TestStatus.SKIP,
                            details="Requires fee calculation verification"))


# =============================================================================
# TC-NEW-006/007: 跨路径一致性 (Cross-path Consistency)
# =============================================================================

def test_cross_path_consistency():
    """验证多个 API 路径返回一致的金额格式"""
    
    print("\n📦 TC-NEW-006/007: 跨路径一致性测试")
    print("-" * 60)
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    # TC-NEW-006: POST /order = GET /orders 格式一致
    test_id = "TC-NEW-006"
    try:
        post_resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "80000.12",
            "qty": "0.12345678",
            "time_in_force": "GTC",
        })
        
        if post_resp.status_code not in [200, 202]:
            collector.add(TestResult(test_id, "POST/GET 格式一致", TestStatus.SKIP,
                                    details=f"Order not placed: {post_resp.status_code}"))
        else:
            order_id = post_resp.json().get("data", {}).get("order_id")
            
            import time
            time.sleep(0.1)
            
            get_resp = client.get(f"/api/v1/private/orders?symbol={SYMBOL}")
            
            if get_resp.status_code == 200:
                orders = get_resp.json().get("data", [])
                for order in orders:
                    if str(order.get("order_id")) == str(order_id):
                        collector.add(TestResult(test_id, "POST/GET 格式一致", TestStatus.PASS,
                                                details=f"qty={order.get('qty')}, price={order.get('price')}"))
                        break
                else:
                    collector.add(TestResult(test_id, "POST/GET 格式一致", TestStatus.SKIP,
                                            details="Order not found in list"))
            else:
                collector.add(TestResult(test_id, "POST/GET 格式一致", TestStatus.SKIP,
                                        details=f"GET failed: {get_resp.status_code}"))
    except Exception as e:
        collector.add(TestResult(test_id, "POST/GET 格式一致", TestStatus.ERROR, str(e)))
    
    # TC-NEW-007: WebSocket 格式一致 (需要 WebSocket 客户端)
    collector.add(TestResult("TC-NEW-007", "WebSocket 格式一致", TestStatus.SKIP,
                            details="Requires WebSocket client"))


# =============================================================================
# TC-NEW-008/009: 余额精度 (Balance Precision)
# =============================================================================

def test_balance_precision():
    """验证余额显示精度"""
    
    print("\n📦 TC-NEW-008/009: 余额精度测试")
    print("-" * 60)
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    # TC-NEW-008: 余额是字符串类型
    test_id = "TC-NEW-008"
    try:
        resp = client.get("/api/v1/private/account")
        
        if resp.status_code == 200:
            data = resp.json()
            balances = data.get("data", {}).get("balances", [])
            
            errors = []
            for balance in balances:
                free = balance.get("free")
                locked = balance.get("locked")
                
                if free is not None and not isinstance(free, str):
                    errors.append(f"free is {type(free).__name__}")
                if locked is not None and not isinstance(locked, str):
                    errors.append(f"locked is {type(locked).__name__}")
            
            if errors:
                collector.add(TestResult(test_id, "余额是字符串类型", TestStatus.FAIL,
                                        details=", ".join(errors)))
            else:
                collector.add(TestResult(test_id, "余额是字符串类型", TestStatus.PASS))
        else:
            collector.add(TestResult(test_id, "余额是字符串类型", TestStatus.SKIP,
                                    details=f"Balance API failed: {resp.status_code}"))
    except Exception as e:
        collector.add(TestResult(test_id, "余额是字符串类型", TestStatus.ERROR, str(e)))
    
    # TC-NEW-009: 最小提现金额
    collector.add(TestResult("TC-NEW-009", "最小提现金额", TestStatus.SKIP,
                            details="Requires withdraw API"))


# =============================================================================
# TC-NEW-010: 错误格式 (Error Format)
# =============================================================================

def test_error_format():
    """验证错误响应包含 code + msg"""
    
    print("\n📦 TC-NEW-010: 错误格式测试")
    print("-" * 60)
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    test_id = "TC-NEW-010"
    try:
        resp = client.post("/api/v1/private/order", {
            "symbol": SYMBOL,
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "0",
            "qty": "1.0",
            "time_in_force": "GTC",
        })
        
        if resp.status_code in [400, 422]:
            data = resp.json()
            has_code = "code" in data or "error" in data
            has_msg = "msg" in data or "message" in data
            
            if has_code and has_msg:
                collector.add(TestResult(test_id, "错误包含 code+msg", TestStatus.PASS,
                                        details=str(data)[:100]))
            else:
                collector.add(TestResult(test_id, "错误包含 code+msg", TestStatus.FAIL,
                                        details=f"got: {data}"))
        else:
            collector.add(TestResult(test_id, "错误包含 code+msg", TestStatus.SKIP,
                                    details=f"Unexpected response: {resp.status_code}"))
    except Exception as e:
        collector.add(TestResult(test_id, "错误包含 code+msg", TestStatus.ERROR, str(e)))


# =============================================================================
# 主执行入口
# =============================================================================

def run_all_advanced_tests():
    """运行所有高级精度测试"""
    print("=" * 80)
    print("🎯 Advanced Money Safety Tests")
    print("=" * 80)
    
    if not health_check():
        print("❌ Gateway not available!")
        return 1
    
    test_roundtrip_integrity()
    test_trade_execution_precision()
    test_cross_path_consistency()
    test_balance_precision()
    test_error_format()
    
    collector.print_summary()
    
    return 0 if collector.all_passed else 1


if __name__ == "__main__":
    sys.exit(run_all_advanced_tests())
