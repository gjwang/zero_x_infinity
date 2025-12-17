#!/usr/bin/env python3
"""
Generate orders with cancel actions - HIGH BALANCE version.

This ensures NO insufficient balance rejects by:
- Giving users 10x more initial balance
- Limiting max order size to 1 BTC (was 10)

Usage:
    python3 scripts/generate_orders_with_cancel_highbal.py --orders 1000000
"""

import random
import csv
import os
from dataclasses import dataclass
from typing import List, Dict

# ============================================================
# CONFIGURATION - HIGH BALANCE
# ============================================================

NUM_ACCOUNTS = 100
NUM_PLACE_ORDERS = 1000000  # 1M default
CANCEL_RATIO = 0.3  # 30% of placed orders will be cancelled

# HIGH BALANCE: 10x more than original
INITIAL_BASE_PER_ACCOUNT = 1000.0  # 1000 BTC (was 100)
INITIAL_QUOTE_PER_ACCOUNT = 100_000_000.0  # 100M USDT (was 10M)

BASE_PRICE = 85000.00

# Precision (matching existing config)
PRICE_DISPLAY_DECIMALS = 2
QTY_DISPLAY_DECIMALS = 6
BASE_DECIMALS = 8
QUOTE_DECIMALS = 6

# Output directory - NEW location, don't overwrite original
OUTPUT_DIR = "fixtures/test_with_cancel_highbal"

# ============================================================
# DATA STRUCTURES
# ============================================================

@dataclass
class Order:
    order_id: int
    user_id: int
    action: str  # "place" or "cancel"
    side: str    # "buy" or "sell" (empty for cancel)
    price: str   # price string (empty for cancel)
    qty: str     # qty string (empty for cancel)

@dataclass
class BalanceRecord:
    user_id: int
    asset_id: int
    avail: int
    frozen: int
    version: int

# ============================================================
# HELPERS
# ============================================================

def to_units(value: float, decimals: int) -> int:
    return int(round(value * (10 ** decimals)))

def generate_price(side: str, base_price: float, decimals: int) -> str:
    """Generate price with ~30% crossing for liquidity."""
    if side == "buy":
        offset = random.gauss(0, 0.005)  # ±0.5% std dev
        price = base_price * (1 + offset - 0.001)  # Slight bias down
    else:
        offset = random.gauss(0, 0.005)
        price = base_price * (1 + offset + 0.001)  # Slight bias up
    
    return f"{price:.{decimals}f}"

def generate_quantity(decimals: int) -> str:
    """Generate smaller quantity to avoid insufficient balance."""
    # Limit max to 1 BTC (was 10)
    qty = 10 ** (random.uniform(-3, 0))  # 0.001 to 1.0 BTC
    return f"{qty:.{decimals}f}"

# ============================================================
# GENERATION
# ============================================================

def generate_orders_with_cancel(num_place: int, num_accounts: int, 
                                 cancel_ratio: float) -> List[Order]:
    orders = []
    active_orders: Dict[int, List[int]] = {i: [] for i in range(num_accounts)}  # user_id -> [order_ids]
    
    order_id = 1
    cancelled_count = 0
    target_cancels = int(num_place * cancel_ratio)
    
    for _ in range(num_place):
        user_id = random.randint(0, num_accounts - 1)
        side = random.choice(["buy", "sell"])
        price = generate_price(side, BASE_PRICE, PRICE_DISPLAY_DECIMALS)
        qty = generate_quantity(QTY_DISPLAY_DECIMALS)
        
        # Place order
        orders.append(Order(
            order_id=order_id,
            user_id=user_id,
            action="place",
            side=side,
            price=price,
            qty=qty
        ))
        active_orders[user_id].append(order_id)
        order_id += 1
        
        # Randomly cancel previous order
        if cancelled_count < target_cancels and random.random() < cancel_ratio:
            # Find a user with active orders to cancel
            candidates = [(uid, oids) for uid, oids in active_orders.items() if oids]
            if candidates:
                cancel_user, cancel_oids = random.choice(candidates)
                cancel_oid = random.choice(cancel_oids)
                
                orders.append(Order(
                    order_id=cancel_oid,
                    user_id=cancel_user,
                    action="cancel",
                    side="",
                    price="",
                    qty=""
                ))
                active_orders[cancel_user].remove(cancel_oid)
                cancelled_count += 1
    
    print(f"Generated {len(orders)} orders:")
    print(f"  Place: {num_place}")
    print(f"  Cancel: {cancelled_count}")
    
    return orders

def generate_balances(num_accounts: int) -> List[BalanceRecord]:
    """Generate initial balance records with HIGH BALANCE."""
    records = []
    
    for user_id in range(num_accounts):
        # BTC balance (asset_id=1) - HIGH BALANCE
        records.append(BalanceRecord(
            user_id=user_id,
            asset_id=1,
            avail=to_units(INITIAL_BASE_PER_ACCOUNT, BASE_DECIMALS),
            frozen=0,
            version=0
        ))
        
        # USDT balance (asset_id=2) - HIGH BALANCE
        records.append(BalanceRecord(
            user_id=user_id,
            asset_id=2,
            avail=to_units(INITIAL_QUOTE_PER_ACCOUNT, QUOTE_DECIMALS),
            frozen=0,
            version=0
        ))
    
    return records

# ============================================================
# CSV OUTPUT
# ============================================================

def write_orders_csv(orders: List[Order], filepath: str):
    os.makedirs(os.path.dirname(filepath), exist_ok=True)
    with open(filepath, 'w', newline='') as f:
        writer = csv.writer(f)
        writer.writerow(['order_id', 'user_id', 'action', 'side', 'price', 'qty'])
        for o in orders:
            writer.writerow([o.order_id, o.user_id, o.action, o.side, o.price, o.qty])
    print(f"Written: {filepath}")

def write_balances_csv(records: List[BalanceRecord], filepath: str):
    os.makedirs(os.path.dirname(filepath), exist_ok=True)
    with open(filepath, 'w', newline='') as f:
        writer = csv.writer(f)
        writer.writerow(['user_id', 'asset_id', 'avail', 'frozen', 'version'])
        for r in records:
            writer.writerow([r.user_id, r.asset_id, r.avail, r.frozen, r.version])
    print(f"Written: {filepath}")

# ============================================================
# MAIN
# ============================================================

def main():
    import argparse
    parser = argparse.ArgumentParser(description='Generate test orders with cancellations (HIGH BALANCE)')
    parser.add_argument('--orders', type=int, default=NUM_PLACE_ORDERS, help='Number of place orders to generate')
    parser.add_argument('--accounts', type=int, default=NUM_ACCOUNTS, help='Number of user accounts')
    
    args = parser.parse_args()
    
    random.seed(42)  # Reproducible
    
    print(f"\n=== Generating HIGH BALANCE Test Set with Cancel Orders ===")
    print(f"Accounts: {args.accounts}")
    print(f"Place orders: {args.orders}")
    print(f"Cancel ratio: {CANCEL_RATIO * 100}%")
    print(f"Initial BTC per account: {INITIAL_BASE_PER_ACCOUNT}")
    print(f"Initial USDT per account: {INITIAL_QUOTE_PER_ACCOUNT:,.0f}")
    print(f"Max order size: 1 BTC")
    print()
    
    # Generate orders
    orders = generate_orders_with_cancel(args.orders, args.accounts, CANCEL_RATIO)
    
    # Generate balances
    balances = generate_balances(args.accounts)
    
    # Write files
    write_orders_csv(orders, f"{OUTPUT_DIR}/orders.csv")
    write_balances_csv(balances, f"{OUTPUT_DIR}/balances_init.csv")
    
    print(f"\nOutput directory: {OUTPUT_DIR}")
    print("Done!")

if __name__ == '__main__':
    main()
