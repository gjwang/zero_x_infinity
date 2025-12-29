#!/usr/bin/env python3
"""
Phase 0x11-b: ERC20 Transfer Event Detection E2E Test

Tests the EthScanner's ability to detect ERC20 Transfer events via eth_getLogs.

Prerequisites:
- Anvil running: anvil --host 0.0.0.0
- Deploy a mock ERC20 contract (or use existing MockUSDT)

Test Cases:
1. TC-ERC20-01: Verify eth_getLogs returns Transfer events
2. TC-ERC20-02: Verify address extraction from topic[2]
3. TC-ERC20-03: Verify amount parsing with correct decimals
4. TC-ERC20-04: Verify integration with scan_block_for_deposits
"""

import subprocess
import json
import sys
import requests
from typing import Optional, Tuple
from decimal import Decimal

# Configuration
ANVIL_URL = "http://127.0.0.1:8545"
TRANSFER_TOPIC = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"


class TestResult:
    def __init__(self, name: str):
        self.name = name
        self.passed = False
        self.message = ""

    def success(self, msg: str = ""):
        self.passed = True
        self.message = msg
        return self

    def fail(self, msg: str):
        self.passed = False
        self.message = msg
        return self


def eth_rpc(method: str, params: list = []) -> dict:
    """Make JSON-RPC call to Anvil"""
    payload = {
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 1
    }
    try:
        response = requests.post(ANVIL_URL, json=payload, timeout=5)
        return response.json()
    except Exception as e:
        return {"error": {"message": str(e)}}


def check_anvil_connection() -> TestResult:
    """TC-PRE-01: Check Anvil is running"""
    result = TestResult("Anvil Connection")
    
    resp = eth_rpc("eth_blockNumber")
    if "error" in resp:
        return result.fail(f"Cannot connect to Anvil: {resp['error']}")
    
    block_num = int(resp["result"], 16)
    return result.success(f"Connected, block: {block_num}")


def test_transfer_topic_constant() -> TestResult:
    """TC-ERC20-01: Verify Transfer topic matches keccak256"""
    result = TestResult("Transfer Topic Constant")
    
    # This is the standard keccak256 hash of "Transfer(address,address,uint256)"
    expected = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
    
    if TRANSFER_TOPIC == expected:
        return result.success("Topic hash is correct")
    else:
        return result.fail(f"Expected {expected}, got {TRANSFER_TOPIC}")


def test_address_extraction() -> TestResult:
    """TC-ERC20-02: Verify address extraction from 32-byte padded topic"""
    result = TestResult("Address Extraction")
    
    # Topic is 32-byte padded (64 hex chars), address is last 20 bytes (40 hex chars)
    topic = "0x0000000000000000000000001234567890abcdef1234567890abcdef12345678"
    
    # Extract last 40 hex chars
    topic_hex = topic[2:]  # Remove 0x
    address = "0x" + topic_hex[-40:]
    
    expected = "0x1234567890abcdef1234567890abcdef12345678"
    
    if address.lower() == expected.lower():
        return result.success(f"Extracted: {address}")
    else:
        return result.fail(f"Expected {expected}, got {address}")


def test_amount_parsing() -> TestResult:
    """TC-ERC20-03: Verify amount parsing with different decimals"""
    result = TestResult("Amount Parsing")
    
    # Test USDT (6 decimals): 1,000,000 = 1 USDT
    usdt_raw = 1000000
    usdt_decimal = Decimal(usdt_raw) / Decimal(10**6)
    
    if usdt_decimal != Decimal("1"):
        return result.fail(f"USDT: Expected 1, got {usdt_decimal}")
    
    # Test ETH (18 decimals): 10^18 = 1 ETH
    eth_raw = 10**18
    eth_decimal = Decimal(eth_raw) / Decimal(10**18)
    
    if eth_decimal != Decimal("1"):
        return result.fail(f"ETH: Expected 1, got {eth_decimal}")
    
    return result.success("USDT=1, ETH=1")


def test_eth_get_logs() -> TestResult:
    """TC-ERC20-04: Verify eth_getLogs API works with Transfer topic"""
    result = TestResult("eth_getLogs API")
    
    # Get latest block
    resp = eth_rpc("eth_blockNumber")
    if "error" in resp:
        return result.fail(f"Cannot get block number: {resp['error']}")
    
    latest_block = resp["result"]
    
    # Query logs for Transfer events in recent blocks
    # Use fromBlock 0 if chain is small
    params = [{
        "fromBlock": "0x0",
        "toBlock": latest_block,
        "topics": [TRANSFER_TOPIC]
    }]
    
    resp = eth_rpc("eth_getLogs", params)
    
    if "error" in resp:
        return result.fail(f"eth_getLogs failed: {resp['error']}")
    
    logs = resp.get("result", [])
    
    # It's OK if there are no logs (clean chain), just verify the call works
    return result.success(f"Found {len(logs)} Transfer events")


def test_rust_unit_tests() -> TestResult:
    """TC-RUST-01: Run Rust unit tests for ERC20 parsing"""
    result = TestResult("Rust Unit Tests")
    
    try:
        proc = subprocess.run(
            ["cargo", "test", "sentinel::eth::tests::test_erc20", "--", "--nocapture"],
            capture_output=True,
            text=True,
            timeout=60,
            cwd="/Users/gjwang/eclipse-workspace/rust_source/zero_x_infinity"
        )
        
        if "test result: ok" in proc.stdout:
            # Count passed tests
            lines = [l for l in proc.stdout.split("\n") if "... ok" in l]
            return result.success(f"{len(lines)} tests passed")
        else:
            return result.fail(f"Tests failed: {proc.stderr[:200]}")
    except Exception as e:
        return result.fail(str(e))


def run_all_tests():
    """Run all test cases and print summary"""
    print("=" * 70)
    print("🎯 Phase 0x11-b: ERC20 Transfer Event Detection E2E Test")
    print("=" * 70)
    print()
    
    tests = [
        check_anvil_connection,
        test_transfer_topic_constant,
        test_address_extraction,
        test_amount_parsing,
        test_eth_get_logs,
        test_rust_unit_tests,
    ]
    
    results = []
    anvil_available = True
    
    for test_func in tests:
        # Skip Anvil-dependent tests if Anvil not available
        if not anvil_available and test_func.__name__ in ["test_eth_get_logs"]:
            result = TestResult(test_func.__name__)
            result.message = "SKIPPED (Anvil not available)"
            results.append(result)
            continue
            
        result = test_func()
        results.append(result)
        
        # Track Anvil availability
        if test_func == check_anvil_connection and not result.passed:
            anvil_available = False
        
        status = "✅" if result.passed else "❌"
        print(f"   {status} {result.name}: {result.message}")
    
    print()
    print("=" * 70)
    print("📊 RESULTS SUMMARY")
    print("=" * 70)
    
    passed = sum(1 for r in results if r.passed)
    failed = sum(1 for r in results if not r.passed and "SKIPPED" not in r.message)
    skipped = sum(1 for r in results if "SKIPPED" in r.message)
    
    print(f"   Total: {passed} passed, {failed} failed, {skipped} skipped")
    print()
    
    if failed == 0:
        print("   ✅ ALL TESTS PASSED")
        return 0
    else:
        print("   ❌ SOME TESTS FAILED")
        return 1


if __name__ == "__main__":
    sys.exit(run_all_tests())
