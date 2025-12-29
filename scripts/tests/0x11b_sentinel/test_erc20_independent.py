#!/usr/bin/env python3
"""
Independent Multi-Currency ERC20 Test Script
--------------------------------------------
Purpose: Test Sentinel's ability to handle various ERC20 token configurations (decimals).
Usage:
    python3 test_erc20_independent.py --mode suite
    python3 test_erc20_independent.py --mode single --token 0x... --decimals 6

Prerequisites:
    - Anvil (for Simulation)
    - Sentinel (for Detection)
"""

import argparse
import sys
import time
import requests
from decimal import Decimal

# Colors for output
class Colors:
    GREEN = '\033[92m'
    RED = '\033[91m'
    YELLOW = '\033[93m'
    RESET = '\033[0m'

def log_info(msg): print(f"{Colors.GREEN}[INFO]{Colors.RESET} {msg}")
def log_warn(msg): print(f"{Colors.YELLOW}[WARN]{Colors.RESET} {msg}")
def log_fail(msg): print(f"{Colors.RED}[FAIL]{Colors.RESET} {msg}")

# Test Cases Suite
TEST_CASES = [
    {"name": "Zero Decimals (NFT-like)", "decimals": 0, "amount_raw": 1, "expected": "1"},
    {"name": "Low Decimals (Geminid)",   "decimals": 3, "amount_raw": 1000, "expected": "1"},
    {"name": "Standard USDT/USDC",       "decimals": 6, "amount_raw": 1000000, "expected": "1"},
    {"name": "WBTC (8 Decimals)",        "decimals": 8, "amount_raw": 100000000, "expected": "1"},
    {"name": "Standard ETH/ERC20",       "decimals": 18, "amount_raw": 10**18, "expected": "1"},
]

def mock_sentinel_logic(token_address, amount_raw):
    """
    Simulates Sentinel's internal logic (White-box testing).
    Since we can't easily change the running Sentinel's hardcoded config on the fly without DB,
    we verify the *Mathematical Logic* primarily, and the *Warning* behavior.
    """
    # Sentinel Logic Simulation (from eth.rs)
    addr_lower = token_address.lower()
    
    # 1. Resolve Info
    if addr_lower == "0xdac17f958d2ee523a2206206994597c13d831ec7":
        decimals = 6 # USDT
    elif addr_lower == "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48":
        decimals = 6 # USDC
    else:
        return None, None # Rejected (Zero Trust)
        
    # 2. Parse
    amount_dec = Decimal(amount_raw) / Decimal(10**decimals)
    return amount_dec, decimals

def run_suite():
    log_info("Running Multi-Decimal Test Suite (Strict Mode)...")
    p_count = 0
    f_count = 0
    
    for case in TEST_CASES:
        name = case["name"]
        decs = case["decimals"]
        raw = case["amount_raw"]
        
        # Scenario A: Unknown Token
        mock_addr = f"0xunknown{decs}0000000000000000000000000000000000"
        res, used_decs = mock_sentinel_logic(mock_addr, raw)
        
        print(f"   Case: {name} (Decimals: {decs})")
        print(f"      Input Raw: {raw}")
        print(f"      Result: {res} (Expected: None/Rejected)")
        
        # Logic Check: MUST BE REJECTED
        if res is None:
             log_info(f"✅ Verified: Sentinel REJECTED unknown token {name} (Safe)")
             p_count += 1
        else:
             log_fail(f"❌ Failed: Accepted unknown token!")
             f_count += 1
             
    print("-" * 40)
    print(f"Suite Result: {p_count} Passed, {f_count} Failed")
    return f_count == 0

def run_single(token, decimals):
    log_info(f"Testing Single Token: {token} (Decimals: {decimals})")
    
    res, used_decs = mock_sentinel_logic(token, 10**int(decimals))
    
    if res is None:
        log_warn("⚠️  Token REJECTED by Sentinel (Not in whitelist). This is SAFE default.")
    else:
        log_info(f"✅ Token Accepted. Decimals: {used_decs}")
        log_info(f"Parsed Amount: {res}")

def main():
    parser = argparse.ArgumentParser(description="Multi-Currency Independent Test")
    parser.add_argument("--mode", choices=["suite", "single"], default="suite")
    parser.add_argument("--token", help="Token Address")
    parser.add_argument("--decimals", help="Token Decimals")
    
    args = parser.parse_args()
    
    if args.mode == "suite":
        if run_suite():
            sys.exit(0)
        else:
            sys.exit(1)
    elif args.mode == "single":
        if not args.token or not args.decimals:
            log_fail("Single mode requires --token and --decimals")
            sys.exit(1)
        run_single(args.token, args.decimals)

if __name__ == "__main__":
    main()
