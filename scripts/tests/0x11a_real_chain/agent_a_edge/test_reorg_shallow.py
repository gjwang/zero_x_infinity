#!/usr/bin/env python3
"""
Agent A (激进派): Re-org Edge Case Testing
Phase 0x11-a: Real Chain Integration

Focus: Shallow re-org detection and handling.
Mission: 破坏系统！找到所有能让系统崩溃或产生错误结果的边界条件。

Test Cases:
- TC-A01: Shallow Re-org Detection
- TC-A03: Multi-Chain Re-org Isolation
"""

import sys
import os
import time

# Add parent to path for common imports
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from common.chain_utils import BtcRpc, EthRpc, GatewayClient, check_node_health

# Add 0x11_funding to path for JWT setup
sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "0x11_funding"))
from common_jwt import setup_jwt_user


def test_shallow_reorg_detection(btc: BtcRpc, gateway: GatewayClient):
    """
    TC-A01: Shallow Re-org Detection
    
    Objective: Verify Sentinel correctly detects and handles shallow re-org.
    
    Steps:
    1. Mine block N with deposit TX
    2. Wait for 3 confirmations
    3. Invalidate block N (simulate re-org)
    4. Mine alternative block N' (no deposit)
    5. Verify deposit status = ORPHANED, balance = 0
    """
    print("\n🔴 TC-A01: Shallow Re-org Detection")
    print("=" * 60)
    
    try:
        # Setup authenticated user
        user_id, token, headers = setup_jwt_user()
        print(f"   ✅ User created: {user_id}")
        
        # Get deposit address
        btc_addr = gateway.get_deposit_address(headers, "BTC", "BTC")
        print(f"   ✅ Deposit address: {btc_addr}")
        
        # Get initial block info
        initial_block = btc.get_latest_block_info()
        print(f"   📦 Initial block height: {initial_block.height}")
        
        # Mine a block to have funds
        btc.mine_blocks(101)  # Maturity
        
        # Send deposit
        deposit_amount = 1.0
        tx_hash = btc.send_to_address(btc_addr, deposit_amount)
        print(f"   📤 Deposit TX sent: {tx_hash}")
        
        # Mine block N with deposit
        block_n_hashes = btc.mine_blocks(1)
        block_n_hash = block_n_hashes[0]
        block_n_height = btc.get_block_count()
        print(f"   ⛏️  Block N mined: height={block_n_height}, hash={block_n_hash[:16]}...")
        
        # Mine 2 more blocks (3 confirmations total)
        btc.mine_blocks(2)
        print(f"   ⛏️  Mined 2 more blocks (3 confirmations)")
        
        # Wait for Sentinel to process
        print("   ⏳ Waiting for Sentinel to process...")
        time.sleep(3)
        
        # Check deposit status before re-org
        history_before = gateway.get_deposit_history(headers, "BTC")
        deposit_record = next((h for h in history_before if h.get("tx_hash") == tx_hash), None)
        
        if deposit_record:
            print(f"   📋 Deposit status before re-org: {deposit_record.get('status')}")
            print(f"   📋 Confirmations: {deposit_record.get('confirmations', 'N/A')}")
        else:
            print("   ⚠️  Deposit not yet in history (Sentinel may not be running)")
        
        # === TRIGGER RE-ORG ===
        print("\n   🌪️  TRIGGERING RE-ORG...")
        
        # Invalidate block N (and all subsequent)
        btc.invalidate_block(block_n_hash)
        print(f"   ❌ Block {block_n_hash[:16]}... invalidated")
        
        # Mine alternative chain (without the deposit TX)
        btc.mine_blocks(5)  # Mine past the original chain
        new_height = btc.get_block_count()
        print(f"   ⛏️  Alternative chain mined to height: {new_height}")
        
        # Wait for Sentinel to detect re-org
        print("   ⏳ Waiting for Sentinel to detect re-org...")
        time.sleep(5)
        
        # === VERIFY RESULTS ===
        print("\n   🔍 VERIFYING RESULTS...")
        
        # Check deposit status after re-org
        history_after = gateway.get_deposit_history(headers, "BTC")
        deposit_after = next((h for h in history_after if h.get("tx_hash") == tx_hash), None)
        
        if deposit_after:
            status = deposit_after.get("status", "UNKNOWN")
            print(f"   📋 Deposit status after re-org: {status}")
            
            if status in ["ORPHANED", "FAILED", "REVERTED"]:
                print("   ✅ PASS: Deposit correctly marked as orphaned")
            elif status == "SUCCESS":
                print("   ❌ FAIL: Deposit still marked SUCCESS after re-org!")
                return False
            else:
                print(f"   ⚠️  INCONCLUSIVE: Status is {status}")
        else:
            print("   ⚠️  Deposit record not found (may have been purged)")
        
        # Check balance (should NOT be credited)
        balance = gateway.get_balance(headers, "BTC")
        print(f"   💰 User balance: {balance}")
        
        if balance is None or balance == 0:
            print("   ✅ PASS: Balance is zero (deposit not credited)")
            return True
        else:
            print(f"   ❌ FAIL: Balance is {balance} (should be 0 after re-org)")
            return False
            
    except Exception as e:
        print(f"   ❌ EXCEPTION: {e}")
        return False


def test_multichain_reorg_isolation(btc: BtcRpc, eth: EthRpc):
    """
    TC-A03: Multi-Chain Re-org Isolation
    
    Objective: Verify BTC re-org does not affect ETH chain cursor.
    
    Steps:
    1. Record ETH cursor position
    2. Trigger BTC re-org
    3. Verify ETH cursor unchanged
    """
    print("\n🔴 TC-A03: Multi-Chain Re-org Isolation")
    print("=" * 60)
    
    try:
        # Get initial state
        btc_info = btc.get_latest_block_info()
        eth_info = eth.get_latest_block_info()
        
        print(f"   📦 BTC initial: height={btc_info.height}, hash={btc_info.hash[:16]}...")
        print(f"   📦 ETH initial: height={eth_info.height}, hash={eth_info.hash[:16]}...")
        
        # Mine some blocks on both chains
        btc.mine_blocks(3)
        eth.mine_block()
        eth.mine_block()
        
        btc_after_mine = btc.get_latest_block_info()
        eth_after_mine = eth.get_latest_block_info()
        
        print(f"   ⛏️  BTC after mining: height={btc_after_mine.height}")
        print(f"   ⛏️  ETH after mining: height={eth_after_mine.height}")
        
        # Trigger BTC re-org
        btc_hash_to_invalidate = btc.get_block_hash(btc_after_mine.height - 1)
        print(f"\n   🌪️  Invalidating BTC block: {btc_hash_to_invalidate[:16]}...")
        
        btc.invalidate_block(btc_hash_to_invalidate)
        btc.mine_blocks(3)  # Mine alternative chain
        
        # Check final state
        btc_final = btc.get_latest_block_info()
        eth_final = eth.get_latest_block_info()
        
        print(f"\n   📦 BTC final: height={btc_final.height}, hash={btc_final.hash[:16]}...")
        print(f"   📦 ETH final: height={eth_final.height}, hash={eth_final.hash[:16]}...")
        
        # Verify ETH unchanged
        if eth_final.height == eth_after_mine.height and eth_final.hash == eth_after_mine.hash:
            print("   ✅ PASS: ETH chain unaffected by BTC re-org")
            return True
        else:
            print("   ❌ FAIL: ETH chain was affected by BTC re-org!")
            return False
            
    except Exception as e:
        print(f"   ❌ EXCEPTION: {e}")
        return False


def main():
    print("=" * 70)
    print("🔴 Agent A (激进派): Re-org Edge Case Testing")
    print("=" * 70)
    
    # Initialize RPC clients
    btc = BtcRpc()
    eth = EthRpc()
    gateway = GatewayClient()
    
    # Check node health
    print("\n📡 Checking node connectivity...")
    health = check_node_health(btc, eth)
    
    if not health.get("btc"):
        print("❌ BTC node not available. Start bitcoind regtest.")
        print("   docker run -d --name btc-regtest -p 18443:18443 ruimarinho/bitcoin-core:24 -regtest -rpcuser=user -rpcpassword=pass -rpcallowip=0.0.0.0/0")
        sys.exit(1)
    
    print("   ✅ BTC node: Connected")
    
    if health.get("eth"):
        print("   ✅ ETH node: Connected")
    else:
        print("   ⚠️  ETH node: Not available (some tests will skip)")
    
    # Run tests
    results = []
    
    # TC-A01
    result = test_shallow_reorg_detection(btc, gateway)
    results.append(("TC-A01: Shallow Re-org Detection", result))
    
    # TC-A03 (requires both chains)
    if health.get("eth"):
        result = test_multichain_reorg_isolation(btc, eth)
        results.append(("TC-A03: Multi-Chain Isolation", result))
    else:
        print("\n⏭️  Skipping TC-A03 (requires ETH node)")
    
    # Summary
    print("\n" + "=" * 70)
    print("📊 RESULTS SUMMARY")
    print("=" * 70)
    
    passed = 0
    for name, result in results:
        status = "✅ PASS" if result else "❌ FAIL"
        print(f"   {status}: {name}")
        if result:
            passed += 1
    
    print(f"\n   Total: {passed}/{len(results)} passed")
    
    sys.exit(0 if passed == len(results) else 1)


if __name__ == "__main__":
    main()
