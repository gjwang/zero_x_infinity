#!/usr/bin/env python3
"""
inject_orders.py - Inject orders from CSV through Gateway HTTP API
==================================================================

PURPOSE:
    Read orders from fixtures CSV and submit them through Gateway HTTP API.
    This enables end-to-end testing where data flows through Gateway to TDengine.

USAGE:
    # Inject 100K orders
    python3 scripts/inject_orders.py --input fixtures/orders.csv
    
    # Inject 1.3M orders
    python3 scripts/inject_orders.py --input fixtures/test_with_cancel_highbal/orders.csv

    # With rate limiting
    python3 scripts/inject_orders.py --input fixtures/orders.csv --rate 1000
"""

import argparse
import csv
import json
import socket
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from threading import Lock

try:
    import requests
    from requests.adapters import HTTPAdapter
    from urllib3.util.retry import Retry
    USE_REQUESTS = True
except ImportError:
    import urllib.request
    import urllib.error
    USE_REQUESTS = False
    print("⚠️  requests library not found, using urllib (slower)")

# Configuration
GATEWAY_URL = "http://localhost:8080"
SYMBOL_MAP = {1: "BTC_USDT", 2: "ETH_USDT"}  # Map symbol_id to symbol name

# Stats
stats = {
    "submitted": 0,
    "accepted": 0,
    "failed": 0,
}
stats_lock = Lock()

# Global HTTP session for connection reuse (Keep-Alive)
_session = None

def get_session():
    """Get or create HTTP session with connection pooling."""
    global _session
    if _session is None and USE_REQUESTS:
        _session = requests.Session()
        # Configure retry strategy
        adapter = HTTPAdapter(
            pool_connections=10,
            pool_maxsize=10,
            max_retries=0  # We handle retries manually
        )
        _session.mount('http://', adapter)
        _session.mount('https://', adapter)
    return _session


def safe_sleep(seconds: float) -> bool:
    """Sleep that handles KeyboardInterrupt. Returns False if interrupted."""
    try:
        time.sleep(seconds)
        return True
    except KeyboardInterrupt:
        print(f"  ⚠️  Interrupted during retry sleep")
        return False

def submit_order(order_data: dict) -> bool:
    """
    Submit a single order through Gateway API.
    
    Handles two CSV formats:
    - 100K: order_id,user_id,side,price,qty
    - 1.3M: order_id,user_id,action,side,price,qty
    """
    # Detect cancel vs place
    action = order_data.get("action", "place").lower()
    is_cancel = action == "cancel"
    
    user_id = order_data.get("user_id", "0")
    
    if is_cancel:
        # Cancel order - use the order_id from CSV
        # Note: This assumes Gateway's order_id matches CSV order_id (sequential injection)
        url = f"{GATEWAY_URL}/api/v1/cancel_order"
        payload = {
            "order_id": int(order_data.get("order_id", 0)),
            "symbol": "BTC_USDT",  # Default symbol
        }
    else:
        # Place order
        url = f"{GATEWAY_URL}/api/v1/create_order"
        
        # Parse side: 'buy'/'sell' or 'BUY'/'SELL'
        side_raw = order_data.get("side", "buy").lower()
        side = "BUY" if side_raw == "buy" else "SELL"
        
        payload = {
            "symbol": "BTC_USDT",  # All test data uses BTC_USDT
            "side": side,
            "order_type": "LIMIT",
            "price": order_data.get("price", "0"),
            "qty": order_data.get("qty", "0"),
        }
    
    max_retries = 50
    max_delay = 5.0    # Cap delay at 5 seconds
    retry_delay = 1 # 1000ms initial, doubles each retry (capped at max_delay)
    
    # Errors that are safe to retry (network/transient issues)
    RETRYABLE_ERRORS = (
        socket.timeout,         # Timeout
        ConnectionRefusedError, # Gateway not ready
        ConnectionResetError,   # Connection dropped
        BrokenPipeError,        # Pipe issues
        TimeoutError,           # General timeout
        OSError,                # Other OS-level network errors
    )
    
    for attempt in range(max_retries + 1):
        try:
            headers = {
                'Content-Type': 'application/json',
                'X-User-ID': str(user_id)
            }
            
            if USE_REQUESTS:
                # Use requests with session for Keep-Alive
                session = get_session()
                response = session.post(url, json=payload, headers=headers, timeout=30)
                
                if response.status_code == 503 and attempt < max_retries:
                    print(f"  ⏳ Retry {attempt+1}/{max_retries}: HTTP 503 (backpressure)")
                    if not safe_sleep(retry_delay):
                        return False, "Interrupted during retry"
                    retry_delay = min(retry_delay * 2, max_delay)
                    continue
                
                if response.status_code >= 400:
                    return False, f"HTTP {response.status_code}: {response.text[:200]}"
                
                result = response.json()
                return result.get('code') == 0, None
            else:
                # Fallback to urllib
                data = json.dumps(payload).encode('utf-8')
                req = urllib.request.Request(url, data=data)
                req.add_header('Content-Type', 'application/json')
                req.add_header('X-User-ID', str(user_id))
                
                with urllib.request.urlopen(req, timeout=30) as response:
                    result = json.loads(response.read().decode())
                    return result.get('code') == 0, None
                
        except Exception as e:
            # Handle request exceptions - check if retryable
            error_name = type(e).__name__
            is_retryable = (
                isinstance(e, (socket.timeout, ConnectionRefusedError, 
                              ConnectionResetError, BrokenPipeError, 
                              TimeoutError, OSError)) or
                (USE_REQUESTS and hasattr(requests, 'exceptions') and 
                 isinstance(e, requests.exceptions.RequestException)) or
                (not USE_REQUESTS and hasattr(e, 'code') and e.code == 503)
            )
            
            if is_retryable and attempt < max_retries:
                print(f"  ⏳ Retry {attempt+1}/{max_retries}: {error_name}")
                if not safe_sleep(retry_delay):
                    return False, "Interrupted during retry"
                retry_delay = min(retry_delay * 2, max_delay)
                continue
            
            # Non-retryable error or max retries exceeded
            if isinstance(e, KeyboardInterrupt):
                return False, "Gateway blocked (max retries)"
            return False, f"{error_name}: {e}"
    
    return False, "Max retries exceeded"




def inject_orders(input_file: str, rate_limit: int = 0, limit: int = 0, quiet: bool = False):
    """
    Inject orders from CSV through Gateway - SEQUENTIAL (preserves order).
    
    IMPORTANT: Must be single-threaded to preserve order determinism!
    Multi-threaded injection would change order → different matching results.
    
    Args:
        input_file: Path to orders CSV
        rate_limit: Max orders per second (0 = unlimited)
        limit: Max orders to inject (0 = all)
        quiet: Suppress progress output
    """
    global stats
    
    # Read orders from CSV
    orders = []
    with open(input_file, 'r') as f:
        reader = csv.DictReader(f)
        for row in reader:
            orders.append(row)
            if limit > 0 and len(orders) >= limit:
                break
    
    total = len(orders)
    if not quiet:
        print(f"Loaded {total} orders from {input_file}")
        print(f"Mode: SEQUENTIAL (single-threaded to preserve order)")
        if rate_limit > 0:
            print(f"Rate limit: {rate_limit} orders/sec")
    
    start_time = time.time()
    
    # Sequential injection - MUST preserve order!
    # Any failure after retries = exit (order sequence critical)
    
    try:
        for i, order in enumerate(orders):
            # Rate limiting
            if rate_limit > 0:
                expected_time = start_time + (i / rate_limit)
                sleep_time = expected_time - time.time()
                if sleep_time > 0:
                    time.sleep(sleep_time)
            
            # Submit order with retry
            success, error = submit_order(order)
            
            stats["submitted"] += 1
            if success:
                stats["accepted"] += 1
            else:
                stats["failed"] += 1
                print(f"  ❌ Order {i+1} failed after retries: {error}")
                print(f"\n❌ FATAL: Order sequence must be maintained. Exiting.")
                break  # Exit immediately - order sequence is critical
            
            # Progress logging
            if not quiet and (i + 1) % 1000 == 0:
                elapsed = time.time() - start_time
                rate = (i + 1) / elapsed if elapsed > 0 else 0
                print(f"  Progress: {i + 1}/{total} ({100*(i+1)//total}%) - {rate:.0f} orders/sec")
    
    except KeyboardInterrupt:
        import traceback
        print(f"\n\n⚠️  Interrupted at {stats['submitted']}/{total} orders")
        print("Traceback:")
        traceback.print_exc()
        # Continue to print summary
    
    elapsed = time.time() - start_time
    rate = total / elapsed if elapsed > 0 else 0
    
    if not quiet:
        print()
        print("=" * 60)
        print("Injection Results")
        print("=" * 60)
        print(f"Total orders:  {total}")
        print(f"Submitted:     {stats['submitted']}")
        print(f"Accepted:      {stats['accepted']}")
        print(f"Failed:        {stats['failed']}")
        print(f"Time:          {elapsed:.2f} seconds")
        print(f"Rate:          {rate:.0f} orders/sec")
    
    return stats['failed'] == 0


def clear_tdengine():
    """Clear all data from TDengine trading tables."""
    print("[CLEAN] Clearing TDengine data...")
    try:
        result = subprocess.run(
            ["docker", "exec", "tdengine", "taos", "-s",
             "DELETE FROM trading.orders; DELETE FROM trading.trades; DELETE FROM trading.balances;"],
            capture_output=True, text=True, timeout=30
        )
        if result.returncode == 0:
            print("[CLEAN] ✅ Data cleared successfully")
            return True
        else:
            print(f"[CLEAN] ⚠️ Warning: {result.stderr}")
            return False
    except Exception as e:
        print(f"[CLEAN] ❌ Failed to clear data: {e}")
        return False


def main():
    parser = argparse.ArgumentParser(description='Inject orders through Gateway API')
    parser.add_argument('--input', '-i', required=True, help='Input orders CSV file')
    parser.add_argument('--workers', '-w', type=int, default=10, help='Number of concurrent workers')
    parser.add_argument('--rate', '-r', type=int, default=0, help='Rate limit (orders/sec, 0=unlimited)')
    parser.add_argument('--limit', '-l', type=int, default=0, help='Max orders to inject (0=all)')
    parser.add_argument('--quiet', '-q', action='store_true', help='Suppress progress output')
    parser.add_argument('--clean', '-c', action='store_true', default=True, 
                       help='Clear TDengine data before injection (default: True)')
    parser.add_argument('--no-clean', dest='clean', action='store_false',
                       help='Do NOT clear TDengine data before injection')
    args = parser.parse_args()
    
    print("╔════════════════════════════════════════════════════════════╗")
    print("║    Gateway Order Injection                                ║")
    print("╚════════════════════════════════════════════════════════════╝")
    print()
    
    # Clear TDengine data if requested
    if args.clean:
        if not clear_tdengine():
            print("⚠️ Warning: Failed to clear TDengine data, continuing anyway...")
        time.sleep(1)  # Wait for TDengine to sync
    
    # Check Gateway is reachable (use OPTIONS on root or try create_order endpoint)
    try:
        import socket
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.settimeout(2)
        result = sock.connect_ex(('localhost', 8080))
        sock.close()
        if result == 0:
            print(f"✓ Gateway reachable at {GATEWAY_URL}")
        else:
            print(f"✗ Gateway not reachable at {GATEWAY_URL}")
            return 1
    except Exception as e:
        print(f"✗ Gateway check failed: {e}")
        return 1
    
    # Inject orders
    success = inject_orders(
        args.input,
        rate_limit=args.rate,
        limit=args.limit,
        quiet=args.quiet
    )
    
    if success:
        print()
        print("✅ Injection complete - all orders processed")
        return 0
    else:
        print()
        print("❌ Some orders failed")
        return 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        print("\n⚠️  Interrupted by user")
        sys.exit(130)
    except Exception as e:
        import traceback
        print(f"\n❌ FATAL ERROR: {e}")
        traceback.print_exc()
        sys.exit(1)
