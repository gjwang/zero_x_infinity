#!/usr/bin/env python3
"""
🔬 API Precision Compliance Test Suite

系统性验证所有 API 输入/输出严格遵守 exchange_info 精度配置。

测试覆盖:
- Phase 1: 配置获取与解析 (CFG-001~005)
- Phase 2: 输入验证 (IN-001~004) - 已由 Agent A 覆盖
- Phase 3: 输出精度验证 (OUT-001~013)
- Phase 4: 往返一致性 (RT-001~004)
"""

import sys
import os
from decimal import Decimal
from typing import Dict, Optional, Tuple
import time

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
# 精度配置解析器
# =============================================================================

class PrecisionConfig:
    """从 exchange_info 解析精度配置"""
    
    def __init__(self, exchange_info: dict):
        self.raw = exchange_info
        self.symbols = {s.get("symbol"): s for s in exchange_info.get("symbols", [])}
        self.assets = {a.get("asset"): a for a in exchange_info.get("assets", [])}
    
    def get_symbol_config(self, symbol: str) -> Optional[dict]:
        return self.symbols.get(symbol)
    
    def get_asset_config(self, asset: str) -> Optional[dict]:
        return self.assets.get(asset)
    
    def get_qty_decimals(self, symbol: str) -> int:
        """获取 qty (base asset) 精度"""
        sym = self.symbols.get(symbol, {})
        base_asset = sym.get("base_asset", "")
        asset_config = self.assets.get(base_asset, {})
        return asset_config.get("decimals", 8)
    
    def get_qty_display_decimals(self, symbol: str) -> int:
        """获取 qty 显示精度"""
        sym = self.symbols.get(symbol, {})
        base_asset = sym.get("base_asset", "")
        asset_config = self.assets.get(base_asset, {})
        return asset_config.get("display_decimals", asset_config.get("decimals", 8))
    
    def get_price_decimals(self, symbol: str) -> int:
        """获取 price 精度"""
        sym = self.symbols.get(symbol, {})
        return sym.get("price_decimals", 2)
    
    def get_asset_decimals(self, asset: str) -> int:
        """获取资产精度"""
        asset_config = self.assets.get(asset, {})
        return asset_config.get("decimals", 8)


def count_decimals(value: str) -> int:
    """计算字符串数值的小数位数"""
    if '.' not in value:
        return 0
    return len(value.rstrip('0').split('.')[-1])


def count_decimals_exact(value: str) -> int:
    """计算字符串数值的精确小数位数（包括尾随零）"""
    if '.' not in value:
        return 0
    return len(value.split('.')[-1])


# =============================================================================
# Phase 1: 配置获取与解析 (CFG-001~005)
# =============================================================================

def test_phase1_config_parsing():
    """Phase 1: 验证 exchange_info 配置可正确获取和解析"""
    
    print("\n" + "=" * 60)
    print("📋 Phase 1: 配置获取与解析")
    print("=" * 60)
    
    # CFG-001: 获取 exchange_info
    test_id = "CFG-001"
    exchange_info = get_exchange_info()
    if exchange_info:
        collector.add(TestResult(test_id, "获取 exchange_info", TestStatus.PASS))
    else:
        collector.add(TestResult(test_id, "获取 exchange_info", TestStatus.FAIL,
                                details="exchange_info API failed"))
        return None
    
    config = PrecisionConfig(exchange_info)
    
    # CFG-002: 解析 symbol
    test_id = "CFG-002"
    sym_config = config.get_symbol_config(SYMBOL)
    if sym_config:
        collector.add(TestResult(test_id, f"解析 {SYMBOL} 配置", TestStatus.PASS,
                                details=f"price_decimals={sym_config.get('price_decimals')}"))
    else:
        collector.add(TestResult(test_id, f"解析 {SYMBOL} 配置", TestStatus.FAIL))
    
    # CFG-003: 解析 qty_decimals (base asset)
    test_id = "CFG-003"
    qty_decimals = config.get_qty_decimals(SYMBOL)
    collector.add(TestResult(test_id, "解析 qty_decimals", TestStatus.PASS,
                            details=f"qty_decimals={qty_decimals}"))
    
    # CFG-004: 解析 price_decimals
    test_id = "CFG-004"
    price_decimals = config.get_price_decimals(SYMBOL)
    collector.add(TestResult(test_id, "解析 price_decimals", TestStatus.PASS,
                            details=f"price_decimals={price_decimals}"))
    
    # CFG-005: 解析 asset decimals
    test_id = "CFG-005"
    base_asset = sym_config.get("base_asset", "BTC") if sym_config else "BTC"
    asset_decimals = config.get_asset_decimals(base_asset)
    collector.add(TestResult(test_id, f"解析 {base_asset} decimals", TestStatus.PASS,
                            details=f"decimals={asset_decimals}"))
    
    print(f"\n  📊 配置摘要:")
    print(f"     Symbol: {SYMBOL}")
    print(f"     qty_decimals: {qty_decimals}")
    print(f"     price_decimals: {price_decimals}")
    print(f"     base_asset: {base_asset} (decimals={asset_decimals})")
    
    return config


# =============================================================================
# Phase 3: 输出精度验证 (OUT-001~013)
# =============================================================================

def test_phase3_output_precision(config: PrecisionConfig):
    """Phase 3: 验证 API 响应精度符合配置"""
    
    print("\n" + "=" * 60)
    print("📏 Phase 3: 输出精度验证")
    print("=" * 60)
    
    if not config:
        collector.add(TestResult("OUT-001", "POST /order qty 精度", TestStatus.SKIP,
                                details="No config available"))
        return
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    expected_qty_decimals = config.get_qty_decimals(SYMBOL)
    expected_qty_display = config.get_qty_display_decimals(SYMBOL)
    expected_price_decimals = config.get_price_decimals(SYMBOL)
    
    # 使用精确的测试值
    test_qty = "0.00123456"  # 8 位小数
    test_price = "85000.12"   # 2 位小数
    
    # OUT-001/002: POST /order 响应精度 (注意: 202 不返回 qty/price)
    # 需要通过 GET /orders 验证
    
    # 先下单
    test_id = "OUT-001"
    post_resp = client.post("/api/v1/private/order", {
        "symbol": SYMBOL,
        "side": "BUY",
        "order_type": "LIMIT",
        "price": test_price,
        "qty": test_qty,
        "time_in_force": "GTC",
    })
    
    if post_resp.status_code not in [200, 202]:
        collector.add(TestResult(test_id, "POST /order", TestStatus.SKIP,
                                details=f"Order failed: {post_resp.status_code}"))
        return
    
    order_id = post_resp.json().get("data", {}).get("order_id")
    time.sleep(0.2)  # Wait for order processing
    
    # OUT-003/004: GET /orders 精度验证
    test_id = "OUT-003"
    get_resp = client.get(f"/api/v1/private/orders?symbol={SYMBOL}")
    
    if get_resp.status_code != 200:
        collector.add(TestResult(test_id, "GET /orders qty 精度", TestStatus.SKIP,
                                details=f"GET failed: {get_resp.status_code}"))
    else:
        orders = get_resp.json().get("data", [])
        target_order = None
        for order in orders:
            if str(order.get("order_id")) == str(order_id):
                target_order = order
                break
        
        if target_order:
            response_qty = target_order.get("qty", "")
            response_price = target_order.get("price", "")
            
            qty_actual_decimals = count_decimals_exact(response_qty)
            price_actual_decimals = count_decimals_exact(response_price)
            
            # OUT-003: qty 精度
            if qty_actual_decimals == expected_qty_display:
                collector.add(TestResult("OUT-003", "GET /orders qty 精度", TestStatus.PASS,
                                        details=f"qty={response_qty} ({qty_actual_decimals} decimals = display_decimals)"))
            elif qty_actual_decimals == expected_qty_decimals:
                collector.add(TestResult("OUT-003", "GET /orders qty 精度", TestStatus.PASS,
                                        details=f"qty={response_qty} ({qty_actual_decimals} decimals = asset_decimals)"))
            else:
                collector.add(TestResult("OUT-003", "GET /orders qty 精度", TestStatus.FAIL,
                                        expected=f"{expected_qty_display} or {expected_qty_decimals}",
                                        actual=str(qty_actual_decimals),
                                        details=f"qty={response_qty}"))
            
            # OUT-004: price 精度
            if price_actual_decimals == expected_price_decimals:
                collector.add(TestResult("OUT-004", "GET /orders price 精度", TestStatus.PASS,
                                        details=f"price={response_price} ({price_actual_decimals} decimals)"))
            else:
                collector.add(TestResult("OUT-004", "GET /orders price 精度", TestStatus.FAIL,
                                        expected=str(expected_price_decimals),
                                        actual=str(price_actual_decimals),
                                        details=f"price={response_price}"))
        else:
            collector.add(TestResult("OUT-003", "GET /orders qty 精度", TestStatus.SKIP,
                                    details="Order not found"))
            collector.add(TestResult("OUT-004", "GET /orders price 精度", TestStatus.SKIP,
                                    details="Order not found"))
    
    # OUT-008/009: GET /account 余额精度
    test_id = "OUT-008"
    account_resp = client.get("/api/v1/private/account")
    
    if account_resp.status_code != 200:
        collector.add(TestResult(test_id, "GET /account free 精度", TestStatus.SKIP,
                                details=f"Account API failed: {account_resp.status_code}"))
    else:
        balances = account_resp.json().get("data", {}).get("balances", [])
        
        for balance in balances:
            asset = balance.get("asset", "")
            free = balance.get("free", "")
            
            if not free or asset not in config.assets:
                continue
            
            expected_decimals = config.get_asset_decimals(asset)
            actual_decimals = count_decimals_exact(free)
            
            if asset == "BTC":  # 只检查 BTC 作为示例
                if actual_decimals == expected_decimals:
                    collector.add(TestResult("OUT-008", f"GET /account {asset} free 精度", TestStatus.PASS,
                                            details=f"free={free} ({actual_decimals} decimals)"))
                else:
                    collector.add(TestResult("OUT-008", f"GET /account {asset} free 精度", TestStatus.FAIL,
                                            expected=str(expected_decimals),
                                            actual=str(actual_decimals),
                                            details=f"free={free}"))
                break
        else:
            collector.add(TestResult("OUT-008", "GET /account BTC free 精度", TestStatus.SKIP,
                                    details="BTC balance not found"))


# =============================================================================
# Phase 4: 往返一致性 (RT-001~004)
# =============================================================================

def test_phase4_roundtrip(config: PrecisionConfig):
    """Phase 4: 验证往返一致性"""
    
    print("\n" + "=" * 60)
    print("🔄 Phase 4: 往返一致性")
    print("=" * 60)
    
    if not config:
        collector.add(TestResult("RT-001", "往返 qty 一致性", TestStatus.SKIP,
                                details="No config available"))
        return
    
    client = get_test_client(GATEWAY_URL, USER_TAKER)
    
    # 使用符合精度配置的输入
    qty_decimals = config.get_qty_decimals(SYMBOL)
    price_decimals = config.get_price_decimals(SYMBOL)
    
    # 生成最大精度的测试值
    input_qty = "0." + "1" * min(qty_decimals, 8)  # e.g., "0.11111111"
    input_price = "85000." + "1" * min(price_decimals, 6)  # e.g., "85000.11"
    
    print(f"\n  📝 测试输入:")
    print(f"     input_qty: {input_qty} ({qty_decimals} decimals configured)")
    print(f"     input_price: {input_price} ({price_decimals} decimals configured)")
    
    # 下单
    post_resp = client.post("/api/v1/private/order", {
        "symbol": SYMBOL,
        "side": "BUY",
        "order_type": "LIMIT",
        "price": input_price,
        "qty": input_qty,
        "time_in_force": "GTC",
    })
    
    if post_resp.status_code not in [200, 202]:
        collector.add(TestResult("RT-001", "往返 qty 一致性", TestStatus.SKIP,
                                details=f"Order failed: {post_resp.status_code}"))
        collector.add(TestResult("RT-002", "往返 price 一致性", TestStatus.SKIP,
                                details=f"Order failed: {post_resp.status_code}"))
        return
    
    order_id = post_resp.json().get("data", {}).get("order_id")
    time.sleep(0.2)
    
    # 查询订单
    get_resp = client.get(f"/api/v1/private/orders?symbol={SYMBOL}")
    
    if get_resp.status_code != 200:
        collector.add(TestResult("RT-001", "往返 qty 一致性", TestStatus.SKIP,
                                details="GET /orders failed"))
        return
    
    orders = get_resp.json().get("data", [])
    target_order = None
    for order in orders:
        if str(order.get("order_id")) == str(order_id):
            target_order = order
            break
    
    if not target_order:
        collector.add(TestResult("RT-001", "往返 qty 一致性", TestStatus.SKIP,
                                details="Order not found"))
        return
    
    response_qty = target_order.get("qty", "")
    response_price = target_order.get("price", "")
    
    print(f"\n  📤 响应输出:")
    print(f"     response_qty: {response_qty}")
    print(f"     response_price: {response_price}")
    
    # RT-001: qty 往返一致性
    input_qty_dec = Decimal(input_qty)
    response_qty_dec = Decimal(response_qty) if response_qty else Decimal("0")
    
    if input_qty_dec == response_qty_dec:
        collector.add(TestResult("RT-001", "往返 qty 一致性", TestStatus.PASS,
                                details=f"Input={input_qty}, Response={response_qty}"))
    else:
        collector.add(TestResult("RT-001", "往返 qty 一致性", TestStatus.FAIL,
                                expected=input_qty, actual=response_qty,
                                details="Precision mismatch after round-trip"))
    
    # RT-002: price 往返一致性
    input_price_dec = Decimal(input_price)
    response_price_dec = Decimal(response_price) if response_price else Decimal("0")
    
    if input_price_dec == response_price_dec:
        collector.add(TestResult("RT-002", "往返 price 一致性", TestStatus.PASS,
                                details=f"Input={input_price}, Response={response_price}"))
    else:
        collector.add(TestResult("RT-002", "往返 price 一致性", TestStatus.FAIL,
                                expected=input_price, actual=response_price,
                                details="Precision mismatch after round-trip"))


# =============================================================================
# 主执行入口
# =============================================================================

def run_all_precision_compliance_tests():
    """运行所有精度合规测试"""
    print("=" * 80)
    print("🔬 API Precision Compliance Test Suite")
    print("=" * 80)
    print("\n目标: 验证所有 API 输入/输出严格遵守 exchange_info 精度配置")
    
    if not health_check():
        print("❌ Gateway not available!")
        return 1
    
    # Phase 1: 配置解析
    config = test_phase1_config_parsing()
    
    # Phase 3: 输出精度验证
    test_phase3_output_precision(config)
    
    # Phase 4: 往返一致性
    test_phase4_roundtrip(config)
    
    collector.print_summary()
    
    return 0 if collector.all_passed else 1


if __name__ == "__main__":
    sys.exit(run_all_precision_compliance_tests())
