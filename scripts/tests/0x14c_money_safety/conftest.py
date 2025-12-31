#!/usr/bin/env python3
"""
0x14-c Money Safety: 测试配置与通用基础设施

复用项目已有的 SDK 和测试基础设施，保持一致性。
参考: scripts/tests/0x14b_matching/test_ioc_qa.py
"""

import sys
import os
import time
from typing import Optional, Dict, List, Tuple
from dataclasses import dataclass
from enum import Enum

# =============================================================================
# 路径设置 - 与 0x14b 保持一致
# =============================================================================

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
SCRIPTS_ROOT = os.path.dirname(os.path.dirname(SCRIPT_DIR))
PROJECT_ROOT = os.path.dirname(SCRIPTS_ROOT)
sys.path.insert(0, SCRIPTS_ROOT)

# =============================================================================
# 导入项目 SDK - 复用已有基础设施
# =============================================================================

try:
    import requests
except ImportError:
    print("Error: Missing 'requests'. Run: pip install requests")
    sys.exit(1)

try:
    from lib.api_auth import get_test_client, ApiClient
    from lib.zero_x_infinity_sdk import ZeroXInfinityClient
    SDK_AVAILABLE = True
except ImportError as e:
    print(f"Warning: SDK not fully available: {e}")
    SDK_AVAILABLE = False


# =============================================================================
# 配置 - 与项目标准保持一致
# =============================================================================

GATEWAY_URL = os.environ.get("GATEWAY_URL", "http://localhost:8080")
SYMBOL = "BTC_USDT"

# 测试用户 - 复用 0x14b 的用户配置
USER_MAKER = 1001
USER_TAKER = 1002

# 资产精度配置 (从 SymbolManager 预期值)
ASSET_DECIMALS = {
    "BTC": 8,
    "ETH": 8,
    "USDT": 6,
    "USDC": 6,
}

SYMBOL_INFO = {
    "BTC_USDT": {"base": "BTC", "quote": "USDT", "base_decimals": 8, "quote_decimals": 6},
    "ETH_USDT": {"base": "ETH", "quote": "USDT", "base_decimals": 8, "quote_decimals": 6},
    "ETH_BTC": {"base": "ETH", "quote": "BTC", "base_decimals": 8, "quote_decimals": 8},
}


# =============================================================================
# 测试结果类型 - 与 0x14b 格式保持一致
# =============================================================================

class TestStatus(Enum):
    PASS = "PASS"
    FAIL = "FAIL"
    SKIP = "SKIP"
    ERROR = "ERROR"


@dataclass
class TestResult:
    """测试结果 - 与 0x14b 格式一致"""
    test_id: str
    name: str
    status: TestStatus
    details: str = ""
    expected: str = ""
    actual: str = ""


# =============================================================================
# Helper Functions - 复用 0x14b 模式
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
    与 0x14b 的 place_order 签名一致
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


def place_order_raw(client: ApiClient, json_body: Dict) -> requests.Response:
    """原始下单请求 - 用于畸形输入测试"""
    return client.post("/api/v1/private/order", json_body)


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


def get_balance(client: ApiClient, asset_id: int = 1) -> Optional[Dict]:
    """获取余额"""
    resp = client.get(f"/api/v1/private/balances", params={"asset_id": asset_id})
    if resp.status_code == 200:
        return resp.json().get("data", {})
    return None


def get_exchange_info() -> Optional[Dict]:
    """获取交易所信息 (公开接口)"""
    try:
        resp = requests.get(f"{GATEWAY_URL}/api/v1/public/exchange_info", timeout=5)
        if resp.status_code == 200:
            return resp.json().get("data", {})
    except Exception:
        pass
    return None


def health_check() -> bool:
    """健康检查"""
    try:
        resp = requests.get(f"{GATEWAY_URL}/api/v1/health", timeout=5)
        return resp.status_code == 200
    except Exception:
        return False


# =============================================================================
# 测试结果收集器
# =============================================================================

class TestResultCollector:
    """收集所有测试结果"""
    
    def __init__(self):
        self.results: List[TestResult] = []
    
    def add(self, result: TestResult):
        self.results.append(result)
        # 实时打印
        icon = {"PASS": "✅", "FAIL": "❌", "SKIP": "⏭️", "ERROR": "💥"}[result.status.value]
        print(f"  {icon} [{result.test_id}] {result.name}")
        if result.status in [TestStatus.FAIL, TestStatus.ERROR] and result.details:
            print(f"      → {result.details}")
    
    def summary(self) -> Dict:
        total = len(self.results)
        passed = sum(1 for r in self.results if r.status == TestStatus.PASS)
        failed = sum(1 for r in self.results if r.status == TestStatus.FAIL)
        skipped = sum(1 for r in self.results if r.status == TestStatus.SKIP)
        errors = sum(1 for r in self.results if r.status == TestStatus.ERROR)
        return {
            "total": total,
            "passed": passed,
            "failed": failed,
            "skipped": skipped,
            "errors": errors,
        }
    
    def print_summary(self):
        s = self.summary()
        print()
        print("=" * 60)
        print("📊 Test Summary")
        print("=" * 60)
        print(f"  Total:   {s['total']}")
        print(f"  Passed:  {s['passed']} ✅")
        print(f"  Failed:  {s['failed']} ❌")
        print(f"  Skipped: {s['skipped']} ⏭️")
        print(f"  Errors:  {s['errors']} 💥")
        print()
        
        if s['failed'] > 0:
            print("Failed Tests:")
            for r in self.results:
                if r.status == TestStatus.FAIL:
                    print(f"  - [{r.test_id}] {r.name}")
                    if r.expected:
                        print(f"    Expected: {r.expected}")
                    if r.actual:
                        print(f"    Actual:   {r.actual}")
    
    @property
    def all_passed(self) -> bool:
        return all(r.status in [TestStatus.PASS, TestStatus.SKIP] for r in self.results)


# 全局收集器
collector = TestResultCollector()


# =============================================================================
# 导出
# =============================================================================

__all__ = [
    # 配置
    "GATEWAY_URL", "SYMBOL", "USER_MAKER", "USER_TAKER",
    "ASSET_DECIMALS", "SYMBOL_INFO", "SDK_AVAILABLE",
    # 类型
    "TestStatus", "TestResult",
    # SDK
    "get_test_client", "ApiClient",
    # 函数
    "place_order", "place_order_raw", "get_order_status", "get_order_details",
    "get_balance", "get_exchange_info", "health_check",
    # 收集器
    "collector", "TestResultCollector",
]
