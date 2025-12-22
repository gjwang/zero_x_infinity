#!/usr/bin/env python3
"""
inject_orders.py - Inject orders from CSV through Gateway HTTP API
==================================================================

PURPOSE:
    Read orders from fixtures CSV and submit them through Gateway HTTP API.
    This enables end-to-end testing where data flows through Gateway to TDengine.
    
    Uses Ed25519 signature authentication via lib/auth.py.

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
import os
import socket
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from threading import Lock

# Add scripts directory to path for lib imports
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, SCRIPT_DIR)

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

# Import Ed25519 auth from shared library
try:
    from lib.api_auth import ApiClient, TEST_API_KEY, TEST_PRIVATE_KEY_HEX
    USE_AUTH = True
except ImportError:
    print("⚠️  lib/auth.py not found, falling back to X-User-ID (legacy mode)")
    USE_AUTH = False

# Configuration
GATEWAY_URL = os.environ.get("GATEWAY_URL", "http://localhost:8080")
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

# Global ApiClient for Ed25519 auth
_api_client = None

def get_api_client():
    """Get or create ApiClient for authenticated requests."""
    global _api_client
    if _api_client is None and USE_AUTH:
        _api_client = ApiClient(
            api_key=TEST_API_KEY,
            private_key_hex=TEST_PRIVATE_KEY_HEX,
            base_url=GATEWAY_URL
        )
    return _api_client

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

def submit_order(order_data: dict) -> tuple:
    """
    Submit a single order through Gateway API with Ed25519 authentication.
    
    Handles two CSV formats:
    - 100K: order_id,user_id,side,price,qty
    - 1.3M: order_id,user_id,action,side,price,qty
    
    Returns:
        (success: bool, error: str or None)
    """
    # Detect cancel vs place
    action = order_data.get("action", "place").lower()
    is_cancel = action == "cancel"
    
    user_id = order_data.get("user_id", "0")
    
    if is_cancel:
        # Cancel order - use /api/v1/private/cancel
        path = f"/api/v1/private/cancel?user_id={user_id}"
        payload = {
            "order_id": int(order_data.get("order_id", 0)),
            "symbol": "BTC_USDT",
        }
    else:
        # Place order - use /api/v1/private/order
        path = f"/api/v1/private/order?user_id={user_id}"
        
        # Parse side: 'buy'/'sell' or 'BUY'/'SELL'
        side_raw = order_data.get("side", "buy").lower()
        side = "BUY" if side_raw == "buy" else "SELL"
        
        payload = {
            "symbol": "BTC_USDT",
            "side": side,
            "order_type": "LIMIT",
            "price": order_data.get("price", "0"),
            "qty": order_data.get("qty", "0"),
        }
    
    max_retries = 50
    max_delay = 5.0    # Cap delay at 5 seconds
    retry_delay = 1.0  # 1000ms initial, doubles each retry (capped at max_delay)
    
    for attempt in range(max_retries + 1):
        try:
            if USE_AUTH:
                # Use Ed25519 authenticated request + X-User-ID header
                client = get_api_client()
                extra_headers = {'X-User-ID': str(user_id)}
                response = client.post(path, payload, headers=extra_headers)
            elif USE_REQUESTS:
                # Legacy: Use X-User-ID header
                url = f"{GATEWAY_URL}{path}"
                headers = {
                    'Content-Type': 'application/json',
                    'X-User-ID': str(user_id)
                }
                session = get_session()
                response = session.post(url, json=payload, headers=headers, timeout=30)
            else:
                # Fallback to urllib (legacy X-User-ID)
                url = f"{GATEWAY_URL}{path}"
                data = json.dumps(payload).encode('utf-8')
                req = urllib.request.Request(url, data=data)
                req.add_header('Content-Type', 'application/json')
                req.add_header('X-User-ID', str(user_id))
                with urllib.request.urlopen(req, timeout=30) as resp:
                    result = json.loads(resp.read().decode())
                    return result.get('code') == 0, None
            
            if response.status_code == 503 and attempt < max_retries:
                print(f"  ⏳ Retry {attempt+1}/{max_retries}: HTTP 503 (backpressure)")
                if not safe_sleep(retry_delay):
                    return False, "Interrupted during retry"
                retry_delay = min(retry_delay * 2, max_delay)
                continue
            
            # Auth failure - don't retry
            if response.status_code == 401:
                return False, f"HTTP 401: Auth failed - {response.text[:100]}"
            
            if response.status_code >= 400:
                return False, f"HTTP {response.status_code}: {response.text[:200]}"
            
            result = response.json()
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


def clean_start(args):
    """
    Clean start: Reset everything and start fresh Gateway.
    1. Kill any running Gateway
    2. Clear TDengine data and confirm
    3. Start Gateway and wait for it to be ready
    Returns True if all steps succeed.
    """
    print("[CLEAN_START] Preparing fresh environment...")
    
    # 1. Kill any running Gateway
    print("[1/4] Stopping any running Gateway...")
    try:
        subprocess.run(["pkill", "-f", "zero_x_infinity.*--gateway"], 
                      capture_output=True, timeout=5)
        time.sleep(2)
        print("    ✅ Gateway stopped")
    except:
        print("    ✅ No Gateway was running")
    
    # 2. Clear TDengine data
    print("[2/4] Clearing TDengine data...")
    try:
        result = subprocess.run(
            ["docker", "exec", "tdengine", "taos", "-s",
             "DELETE FROM trading.orders; DELETE FROM trading.trades; DELETE FROM trading.balances;"],
            capture_output=True, text=True, timeout=30
        )
        if result.returncode != 0:
            print(f"    ❌ Failed: {result.stderr}")
            return False
        print("    ✅ Data cleared")
    except Exception as e:
        print(f"    ❌ Failed: {e}")
        return False
    
    # 3. Confirm data is cleared
    print("[3/4] Verifying data cleared...")
    try:
        result = subprocess.run(
            ["docker", "exec", "tdengine", "taos", "-s", "SELECT COUNT(*) FROM trading.orders"],
            capture_output=True, text=True, timeout=10
        )
        if "0 |" not in result.stdout and "|                     0 |" not in result.stdout:
            print(f"    ❌ Data not cleared properly")
            return False
        print("    ✅ Confirmed: 0 orders")
    except Exception as e:
        print(f"    ⚠️ Could not verify: {e}")
    
    # 4. Start Gateway
    print("[4/4] Starting Gateway...")
    try:
        import os
        if not os.path.exists("./target/release/zero_x_infinity"):
            print("    ❌ Binary not found. Run: cargo build --release")
            return False
        
        input_dir = os.path.dirname(args.input) if args.input else "fixtures"
        if not input_dir: input_dir = "."
        
        subprocess.Popen(
            ["./target/release/zero_x_infinity", "--gateway", "--port", "8080", "--input", input_dir],
            stdout=open("/tmp/gw.log", "w"),
            stderr=subprocess.STDOUT,
            start_new_session=True
        )
        time.sleep(3)
        
        # Verify Gateway is running
        import socket
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.settimeout(2)
        if sock.connect_ex(('localhost', 8080)) != 0:
            print("    ❌ Gateway failed to start")
            sock.close()
            return False
        sock.close()
        print("    ✅ Gateway started and ready")
    except Exception as e:
        print(f"    ❌ Failed: {e}")
        return False
    
    print()
    print("[CLEAN_START] ✅ Environment ready, starting injection...")
    print()
    return True


def main():
    parser = argparse.ArgumentParser(description='Inject orders through Gateway API')
    parser.add_argument('--input', '-i', required=True, help='Input orders CSV file')
    parser.add_argument('--workers', '-w', type=int, default=10, help='Number of concurrent workers')
    parser.add_argument('--rate', '-r', type=int, default=0, help='Rate limit (orders/sec, 0=unlimited)')
    parser.add_argument('--limit', '-l', type=int, default=0, help='Max orders to inject (0=all)')
    parser.add_argument('--quiet', '-q', action='store_true', help='Suppress progress output')
    parser.add_argument('--clean_start', action='store_true',
                       help='Clean start: pkill Gateway, clear DB, start fresh Gateway, then inject')
    args = parser.parse_args()
    
    print("╔════════════════════════════════════════════════════════════╗")
    print("║    Gateway Order Injection                                ║")
    print("╚════════════════════════════════════════════════════════════╝")
    print()
    
    # --clean_start: Do everything from scratch
    if args.clean_start:
        if not clean_start(args):
            print("❌ Clean start failed, aborting")
            return 1
        # Continue to injection below...
    
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
