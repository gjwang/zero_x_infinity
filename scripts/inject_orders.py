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
import sys
import time
import urllib.request
import urllib.error
from concurrent.futures import ThreadPoolExecutor, as_completed
from threading import Lock

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
    
    max_retries = 10
    retry_delay = 0.05  # 50ms initial, doubles each retry
    
    # Errors that are safe to retry (network/transient issues)
    RETRYABLE_ERRORS = (
        urllib.error.URLError,  # Connection issues
        socket.timeout,         # Timeout
        ConnectionRefusedError, # Gateway not ready
        ConnectionResetError,   # Connection dropped
        BrokenPipeError,        # Pipe issues
        TimeoutError,           # General timeout
        OSError,                # Other OS-level network errors
    )
    
    for attempt in range(max_retries + 1):
        try:
            data = json.dumps(payload).encode('utf-8')
            req = urllib.request.Request(url, data=data)
            req.add_header('Content-Type', 'application/json')
            req.add_header('X-User-ID', str(user_id))
            
            with urllib.request.urlopen(req, timeout=30) as response:
                result = json.loads(response.read().decode())
                return result.get('code') == 0, None
                
        except urllib.error.HTTPError as e:
            # HTTP errors: only retry 503 (backpressure)
            if e.code == 503 and attempt < max_retries:
                print(f"  ⏳ Retry {attempt+1}/{max_retries}: HTTP 503 (backpressure)")
                time.sleep(retry_delay)
                retry_delay *= 2
                continue
            # Non-retryable HTTP error
            try:
                body = e.read().decode()[:200]
            except:
                body = ""
            return False, f"HTTP {e.code}: {body}"
            
        except RETRYABLE_ERRORS as e:
            # Network error - retry with logging
            if attempt < max_retries:
                print(f"  ⏳ Retry {attempt+1}/{max_retries}: {type(e).__name__}")
                time.sleep(retry_delay)
                retry_delay *= 2
                continue
            return False, f"{type(e).__name__}: {e}"
        
        except KeyboardInterrupt:
            # Gateway blocking during socket.connect() - retry like network error
            if attempt < max_retries:
                print(f"  ⏳ Retry {attempt+1}/{max_retries}: Gateway blocked (socket)")
                time.sleep(retry_delay)
                retry_delay *= 2
                continue
            return False, "Gateway blocked (max retries)"
            
        except Exception as e:
            # Unknown error - exit immediately, don't hide it
            print(f"\n❌ FATAL: {type(e).__name__}: {e}")
            raise  # Re-raise to trigger traceback and exit
    
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
        if stats['failed'] > 0 and last_error:
            print(f"Last error:    {last_error}")
        print(f"Time:          {elapsed:.2f} seconds")
        print(f"Rate:          {rate:.0f} orders/sec")
    
    return stats['failed'] == 0


def main():
    parser = argparse.ArgumentParser(description='Inject orders through Gateway API')
    parser.add_argument('--input', '-i', required=True, help='Input orders CSV file')
    parser.add_argument('--workers', '-w', type=int, default=10, help='Number of concurrent workers')
    parser.add_argument('--rate', '-r', type=int, default=0, help='Rate limit (orders/sec, 0=unlimited)')
    parser.add_argument('--limit', '-l', type=int, default=0, help='Max orders to inject (0=all)')
    parser.add_argument('--quiet', '-q', action='store_true', help='Suppress progress output')
    args = parser.parse_args()
    
    print("╔════════════════════════════════════════════════════════════╗")
    print("║    Gateway Order Injection                                ║")
    print("╚════════════════════════════════════════════════════════════╝")
    print()
    
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
