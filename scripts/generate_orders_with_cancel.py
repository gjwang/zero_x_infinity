#!/usr/bin/env python3
"""
Generate orders with cancel actions for testing order lifecycle.

This script generates a smaller test set with:
- Place orders
- Cancel orders (canceling previous active orders)

Usage:
    python3 scripts/generate_orders_with_cancel.py
"""

import random
import csv
import os
from dataclasses import dataclass
from typing import List, Dict

# ============================================================
# CONFIGURATION
# ============================================================

NUM_ACCOUNTS = 100
NUM_PLACE_ORDERS = 1000
CANCEL_RATIO = 0.3  # 30% of placed orders will be cancelled

INITIAL_BASE_PER_ACCOUNT = 100.0  # BTC
INITIAL_QUOTE_PER_ACCOUNT = 10_000_000.0  # USDT

BASE_PRICE = 85000.00

# Precision (matching existing config)
PRICE_DISPLAY_DECIMALS = 2
QTY_DISPLAY_DECIMALS = 6
BASE_DECIMALS = 8
QUOTE_DECIMALS = 6

# Output directory
OUTPUT_DIR = "fixtures/test_with_cancel"

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
    """Generate quantity using power law distribution."""
    qty = 10 ** (random.uniform(-3, 1))  # 0.001 to 10
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
    """Generate initial balance records."""
    records = []
    
    for user_id in range(num_accounts):
        # BTC balance (asset_id=1)
        records.append(BalanceRecord(
            user_id=user_id,
            asset_id=1,
            avail=to_units(INITIAL_BASE_PER_ACCOUNT, BASE_DECIMALS),
            frozen=0,
            version=0
        ))
        
        # USDT balance (asset_id=2)
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
    random.seed(42)  # Reproducible
    
    print(f"\n=== Generating Test Set with Cancel Orders ===")
    print(f"Accounts: {NUM_ACCOUNTS}")
    print(f"Place orders: {NUM_PLACE_ORDERS}")
    print(f"Cancel ratio: {CANCEL_RATIO * 100}%")
    print()
    
    # Generate orders
    orders = generate_orders_with_cancel(NUM_PLACE_ORDERS, NUM_ACCOUNTS, CANCEL_RATIO)
    
    # Generate balances
    balances = generate_balances(NUM_ACCOUNTS)
    
    # Write files
    write_orders_csv(orders, f"{OUTPUT_DIR}/orders.csv")
    write_balances_csv(balances, f"{OUTPUT_DIR}/balances_init.csv")
    
    print(f"\nOutput directory: {OUTPUT_DIR}")
    print("Done!")

if __name__ == '__main__':
    main()
