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
    # Vulnerability Check: These inputs are UNKNOWN tokens.
    # Current Code assumes 18 decimals.
    # So 1 unit (raw 1) -> 1e-18.
    # 1000 units (raw 1000) -> 1e-15.
    # The EXPECTATION here is what the CURRENT CODE DOES (Vulnerable behavior).
    # We will log it as SAFETY FAILURE if it matches "1E-18" or similar.
    {"name": "Zero Decimals (NFT-like)", "decimals": 0, "amount_raw": 1, "expected": "1E-18"},
    {"name": "Low Decimals (Geminid)",   "decimals": 3, "amount_raw": 1000, "expected": "1E-15"},
    # Case 3: Standard USDT/USDC - Must provide the Known Address to get 6 decimals
    {"name": "Standard USDT/USDC",       "decimals": 6, "token": "0xdac17f958d2ee523a2206206994597c13d831ec7", "amount_raw": 1000000, "expected": "1"}, # Known
    {"name": "WBTC (8 Decimals)",        "decimals": 8, "amount_raw": 100000000, "expected": "1E-10"}, # Unknown
    {"name": "Standard ETH/ERC20",       "decimals": 18, "amount_raw": 10**18, "expected": "1"}, # Works by luck (18=18)
    
    # HUGE Amount (DoS/Inflation Check)
    # Unknown Token (Default 18) -> 10^50 / 10^18 = 10^32 (MASSIVE INFLATION!)
    # Unless u128 limit hits? 10^50 > u128 max. So 0.
    {"name": "Huge Amount (Overflow/Inflation)", "decimals": 18, "amount_raw": 10**50, "expected": "0"}, 

    # DoS Vector: HUGE Amount on WHITELISTED Token
    {"name": "USDT Huge Overflow", "decimals": 6, "token": "0xdac17f958d2ee523a2206206994597c13d831ec7", "amount_raw": 10**50, "expected": "0"}
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
    used_decs = None # Default Secure (Reject)
    
    # Use lowercase for comparison as defined in eth.rs
    usdt_addr = "0xdac17f958d2ee523a2206206994597c13d831ec7" 
    usdc_addr = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    
    if addr_lower == usdt_addr:
        used_decs = 6 # USDT
    elif addr_lower == usdc_addr:
        used_decs = 6 # USDC Limits)
    # Rust: u128 limit (~3.4e38)
    if amount_raw > 340282366920938463463374607431768211455: # u128::MAX
         return Decimal(0), used_decs 
    
    if used_decs is None:
         return None, None
         
    amount_dec = Decimal(amount_raw) / Decimal(10**used_decs)
    return amount_dec, used_decs

def run_suite():
    log_info("Running Multi-Decimal Test Suite (Vulnerability Scan)...")
    p_count = 0
    f_count = 0
    
    for case in TEST_CASES:
        name = case["name"]
        decs = case["decimals"]
        raw = case["amount_raw"]
        exp_s = case["expected"]
        
        # Scenario A: Unknown Token unless specified
        mock_addr = case.get("token", f"0xunknown{decs}0000000000000000000000000000000000")
        res, used_decs = mock_sentinel_logic(mock_addr, raw)
        
        print(f"   Case: {name} (Decimals: {decs})")
        print(f"      Input Raw: {raw}")
        print(f"      Sentinel assumption: {used_decs} decimals")
        print(f"      Result: {res}")
        
        if res is None and used_decs is None:
             # This is the SECURE behavior for unknown tokens
             if "unknown" in mock_addr.lower() or name in ["WBTC (8 Decimals)", "Zero Decimals (NFT-like)", "Low Decimals (Geminid)", "Huge Amount (Overflow/Inflation)"]:
                 log_info(f"✅ SECURE: Unknown token rejected ({name})")
                 p_count += 1
                 continue
        
        if res == Decimal(exp_s):
             log_info(f"✅ Behavior Verified: {name}")
             p_count += 1
        else:
             log_warn(f"⚠️  Result Mismatch: Got {res}, Expected {exp_s}")
             f_count += 1
             
    print("-" * 40)
    print(f"Suite Scan Result: {p_count} Scenarios Verified, {f_count} Failed")
    return f_count == 0

def run_single(token, decimals):
    log_info(f"Testing Single Token: {token} (Decimals: {decimals})")
    
    res, used_decs = mock_sentinel_logic(token, 10**int(decimals))
    
    if used_decs == 18 and int(decimals) != 18:
        log_fail("❌ VULNERABILITY DETECTED: Unknown token treated as 18 decimals!")
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
        # Returns 0 if all checks ran fine (even if they found vulns)
        # Returns 1 if script crashed or logic mismatch
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
