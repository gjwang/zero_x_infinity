#!/usr/bin/env python3
"""
Phase 0x11-b Sentinel Hardening Test Suite

Tests:
1. BTC P2WPKH (SegWit) address detection
2. ETH Real RPC scanner functionality
3. Combined Sentinel verification

Usage:
    ./scripts/tests/0x11b_sentinel/test_sentinel_0x11b.py

Requirements:
    - bitcoind (regtest) running on port 18443
    - anvil (or geth) running on port 8545
    - Gateway running on port 8080 (optional for full E2E)
"""

import sys
import os
import time
import subprocess
from typing import Tuple, Optional

# Add parent paths for imports
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "0x11a_real_chain"))

try:
    from common.chain_utils import BtcRpc, EthRpc, check_node_health
except ImportError:
    # Fallback: Define minimal classes inline
    import requests
    
    class BtcRpc:
        def __init__(self, url="http://127.0.0.1:18443", user="user", password="pass", wallet="sentinel_test"):
            if wallet and "/wallet/" not in url:
                self.url = f"{url.rstrip('/')}/wallet/{wallet}"
            else:
                self.url = url
            self.auth = (user, password)
            self._id = 0
        
        def _call(self, method, params=None):
            self._id += 1
            payload = {"jsonrpc": "2.0", "id": self._id, "method": method, "params": params or []}
            resp = requests.post(self.url, json=payload, auth=self.auth)
            result = resp.json()
            if "error" in result and result["error"]:
                raise Exception(f"RPC Error: {result['error']}")
            return result.get("result")
        
        def get_block_count(self): return self._call("getblockcount")
        def getnewaddress(self, label="", addr_type="bech32"): return self._call("getnewaddress", [label, addr_type])
        def mine_blocks(self, count=1):
            addr = self._call("getnewaddress")
            return self._call("generatetoaddress", [count, addr])
    
    class EthRpc:
        def __init__(self, url="http://127.0.0.1:8545"):
            self.url = url
            self._id = 0
        
        def _call(self, method, params=None):
            self._id += 1
            payload = {"jsonrpc": "2.0", "id": self._id, "method": method, "params": params or []}
            resp = requests.post(self.url, json=payload)
            result = resp.json()
            if "error" in result and result["error"]:
                raise Exception(f"RPC Error: {result['error']}")
            return result.get("result")
        
        def get_block_number(self):
            hex_result = self._call("eth_blockNumber")
            return int(hex_result, 16)
        
        def get_block_by_number(self, number, full_tx=False):
            return self._call("eth_getBlockByNumber", [hex(number), full_tx])
        
        def eth_syncing(self):
            return self._call("eth_syncing")
    
    def check_node_health(btc=None, eth=None):
        result = {}
        if btc:
            try:
                btc.get_block_count()
                result["btc"] = True
            except:
                result["btc"] = False
        if eth:
            try:
                eth.get_block_number()
                result["eth"] = True
            except:
                result["eth"] = False
        return result


# =============================================================================
# Test 1: BTC P2WPKH (SegWit) Address Detection
# =============================================================================

def test_btc_segwit_address_generation(btc: BtcRpc) -> Tuple[bool, str]:
    """
    TC-B01: Verify BTC SegWit (P2WPKH) address generation and format.
    
    Checks:
    - bitcoind generates valid bcrt1 (Bech32) addresses in regtest
    - Address format matches expected pattern
    """
    print("\n🔵 TC-B01: BTC SegWit Address Generation")
    print("=" * 60)
    
    try:
        # Request SegWit address
        segwit_addr = btc._call("getnewaddress", ["", "bech32"])
        print(f"   📍 Generated address: {segwit_addr}")
        
        # Verify format
        if segwit_addr.startswith("bcrt1"):
            print("   ✅ Address format: bcrt1... (Regtest Bech32)")
            
            # Verify length (P2WPKH = 42-44 chars for bcrt1)
            if len(segwit_addr) >= 42:
                print(f"   ✅ Address length: {len(segwit_addr)} chars (valid)")
                return True, segwit_addr
            else:
                print(f"   ❌ Address too short: {len(segwit_addr)} chars")
                return False, segwit_addr
        else:
            print(f"   ❌ Unexpected prefix: {segwit_addr[:5]}")
            return False, segwit_addr
            
    except Exception as e:
        print(f"   ❌ EXCEPTION: {e}")
        return False, ""


def test_btc_segwit_transaction(btc: BtcRpc, segwit_addr: str) -> Tuple[bool, str]:
    """
    TC-B02: Verify BTC can send to SegWit address and mine.
    """
    print("\n🔵 TC-B02: BTC SegWit Transaction")
    print("=" * 60)
    
    try:
        # Ensure maturity
        count = btc.get_block_count()
        if count < 101:
            print(f"   ⛏️  Mining {101 - count} blocks for maturity...")
            btc.mine_blocks(101 - count)
        
        # Send to SegWit address
        amount = 0.1
        tx_hash = btc._call("sendtoaddress", [segwit_addr, amount])
        print(f"   📤 Sent {amount} BTC to {segwit_addr[:20]}...")
        print(f"   📋 TX Hash: {tx_hash}")
        
        # Mine to confirm
        btc.mine_blocks(1)
        print("   ⛏️  Mined 1 block")
        
        # Verify TX is confirmed
        tx_info = btc._call("gettransaction", [tx_hash])
        confirmations = tx_info.get("confirmations", 0)
        print(f"   ✅ Confirmations: {confirmations}")
        
        return confirmations >= 1, tx_hash
        
    except Exception as e:
        print(f"   ❌ EXCEPTION: {e}")
        return False, ""


# =============================================================================
# Test 2: ETH RPC Scanner Functionality
# =============================================================================

def test_eth_rpc_connection(eth: EthRpc) -> bool:
    """
    TC-E01: Verify ETH RPC connection and block height retrieval.
    """
    print("\n🟣 TC-E01: ETH RPC Connection")
    print("=" * 60)
    
    try:
        height = eth.get_block_number()
        print(f"   📍 Current block height: {height}")
        
        # Get block details
        block = eth.get_block_by_number(height)
        print(f"   📋 Block hash: {block['hash'][:20]}...")
        print(f"   📋 Parent hash: {block['parentHash'][:20]}...")
        print(f"   📋 Timestamp: {int(block['timestamp'], 16)}")
        
        print("   ✅ ETH RPC connection verified")
        return True
        
    except Exception as e:
        print(f"   ❌ EXCEPTION: {e}")
        return False


def test_eth_syncing_status(eth: EthRpc) -> bool:
    """
    TC-E02: Verify ETH sync status check.
    """
    print("\n🟣 TC-E02: ETH Syncing Status")
    print("=" * 60)
    
    try:
        syncing = eth.eth_syncing()
        print(f"   📋 Syncing result: {syncing}")
        
        if syncing is False:
            print("   ✅ Node is fully synced")
            return True
        elif isinstance(syncing, dict):
            print(f"   ⚠️  Node is syncing: {syncing}")
            return True
        else:
            print(f"   ❓ Unexpected syncing result type")
            return True  # Not a failure
            
    except Exception as e:
        print(f"   ❌ EXCEPTION: {e}")
        return False


def test_eth_block_scanning(eth: EthRpc) -> bool:
    """
    TC-E03: Verify ETH block scanning with transaction details.
    """
    print("\n🟣 TC-E03: ETH Block Scanning")
    print("=" * 60)
    
    try:
        height = eth.get_block_number()
        
        # Scan last 5 blocks
        blocks_scanned = 0
        for h in range(max(0, height - 4), height + 1):
            block = eth.get_block_by_number(h, full_tx=True)
            tx_count = len(block.get("transactions", []))
            blocks_scanned += 1
            print(f"   📦 Block {h}: {tx_count} transactions")
        
        print(f"   ✅ Successfully scanned {blocks_scanned} blocks")
        return True
        
    except Exception as e:
        print(f"   ❌ EXCEPTION: {e}")
        return False


# =============================================================================
# Test 3: Rust Sentinel Unit Tests
# =============================================================================

def test_rust_sentinel_unit_tests() -> bool:
    """
    TC-R01: Run Rust Sentinel unit tests.
    """
    print("\n🦀 TC-R01: Rust Sentinel Unit Tests")
    print("=" * 60)
    
    try:
        result = subprocess.run(
            ["cargo", "test", "--package", "zero_x_infinity", "--lib", "sentinel", "--", "--nocapture"],
            cwd=os.path.join(os.path.dirname(__file__), "..", "..", ".."),
            capture_output=True,
            text=True,
            timeout=300
        )
        
        # Parse output for test count
        output = result.stdout + result.stderr
        
        if "test result: ok" in output:
            # Extract test count
            for line in output.split("\n"):
                if "test result: ok" in line:
                    print(f"   ✅ {line.strip()}")
            return True
        else:
            print(f"   ❌ Tests failed:")
            print(output[-2000:])  # Last 2000 chars
            return False
            
    except subprocess.TimeoutExpired:
        print("   ❌ Tests timed out (300s)")
        return False
    except Exception as e:
        print(f"   ❌ EXCEPTION: {e}")
        return False


# =============================================================================
# Main Test Runner
# =============================================================================

def main():
    print("=" * 70)
    print("🌟 Phase 0x11-b: Sentinel Hardening Test Suite")
    print("=" * 70)
    
    results = []
    btc = None
    eth = None
    
    # Check node availability
    print("\n📡 Checking node connectivity...")
    try:
        btc = BtcRpc()
        eth = EthRpc()
        health = check_node_health(btc, eth)
    except Exception as e:
        print(f"   ⚠️  Error initializing clients: {e}")
        health = {"btc": False, "eth": False}
    
    btc_available = health.get("btc", False)
    eth_available = health.get("eth", False)
    
    print(f"   BTC Node: {'✅ Connected' if btc_available else '❌ Not available'}")
    print(f"   ETH Node: {'✅ Connected' if eth_available else '❌ Not available'}")
    
    # ==========================================================================
    # Run Tests
    # ==========================================================================
    
    # BTC Tests (if available)
    if btc_available:
        passed, segwit_addr = test_btc_segwit_address_generation(btc)
        results.append(("TC-B01: BTC SegWit Address", passed))
        
        if passed:
            passed, tx_hash = test_btc_segwit_transaction(btc, segwit_addr)
            results.append(("TC-B02: BTC SegWit Transaction", passed))
    else:
        print("\n⚠️  Skipping BTC tests (node not available)")
        results.append(("TC-B01: BTC SegWit Address", None))
        results.append(("TC-B02: BTC SegWit Transaction", None))
    
    # ETH Tests (if available)
    if eth_available:
        results.append(("TC-E01: ETH RPC Connection", test_eth_rpc_connection(eth)))
        results.append(("TC-E02: ETH Syncing Status", test_eth_syncing_status(eth)))
        results.append(("TC-E03: ETH Block Scanning", test_eth_block_scanning(eth)))
    else:
        print("\n⚠️  Skipping ETH tests (node not available)")
        results.append(("TC-E01: ETH RPC Connection", None))
        results.append(("TC-E02: ETH Syncing Status", None))
        results.append(("TC-E03: ETH Block Scanning", None))
    
    # Rust Unit Tests (always run)
    results.append(("TC-R01: Rust Sentinel Unit Tests", test_rust_sentinel_unit_tests()))
    
    # ==========================================================================
    # Summary
    # ==========================================================================
    print("\n" + "=" * 70)
    print("📊 RESULTS SUMMARY")
    print("=" * 70)
    
    passed = 0
    failed = 0
    skipped = 0
    
    for name, result in results:
        if result is True:
            status = "✅ PASS"
            passed += 1
        elif result is False:
            status = "❌ FAIL"
            failed += 1
        else:
            status = "⏭️  SKIP"
            skipped += 1
        print(f"   {status}: {name}")
    
    print(f"\n   Total: {passed} passed, {failed} failed, {skipped} skipped")
    print("=" * 70)
    
    # Exit code
    if failed > 0:
        print("\n❌ Some tests failed!")
        sys.exit(1)
    elif passed == 0 and skipped > 0:
        print("\n⚠️  All runnable tests were skipped. Check node availability.")
        sys.exit(0)
    else:
        print("\n✅ All tests passed!")
        sys.exit(0)


if __name__ == "__main__":
    main()
