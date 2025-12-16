#!/usr/bin/env python3
"""
Generate realistic BTC/USDT order data for matching engine testing.

Configuration is read from:
- fixtures/assets_config.csv (asset definitions)
- fixtures/symbols_config.csv (trading pair definitions)

This ensures a single source of truth for all configuration.
"""

import csv
import random
import argparse
from dataclasses import dataclass
from typing import List, Dict
import os
import math

# ============================================================
# CONFIGURATION LOADING
# ============================================================

@dataclass
class AssetConfig:
    """
    Asset configuration from CSV.
    
    Decimal Precision Design:
    ========================
    
    | Field            | Mutable      | Purpose                    | Example          |
    |------------------|--------------|----------------------------|------------------|
    | decimals         | ⚠️ IMMUTABLE | Internal storage precision | BTC=8 (satoshi)  |
    | display_decimals | ✅ Dynamic   | Client-facing precision    | BTC=2 (0.01 BTC) |
    
    Key Rules:
    ----------
    1. `decimals` - Set once, NEVER change
       - Defines minimum unit (e.g., 1 satoshi = 10^-8 BTC)
       - Internal balances/orders use this precision
       - Changing would corrupt all existing data
    
    2. `display_decimals` - Can be adjusted anytime
       - Client sees prices/quantities with this precision  
       - Orders from clients use this format
       - Example: Client submits price "84907.12" (2 decimals)
    """
    asset_id: int
    asset: str
    decimals: int          # ⚠️ IMMUTABLE - internal storage precision
    display_decimals: int  # ✅ Dynamic - client-facing precision

@dataclass
class SymbolConfig:
    """Symbol configuration from CSV."""
    symbol_id: int
    symbol: str
    base_asset_id: int
    quote_asset_id: int
    price_decimal: int
    price_display_decimal: int

def load_assets_config(filepath: str) -> Dict[int, AssetConfig]:
    """Load assets from CSV."""
    assets = {}
    with open(filepath, 'r') as f:
        reader = csv.DictReader(f)
        for row in reader:
            asset = AssetConfig(
                asset_id=int(row['asset_id']),
                asset=row['asset'],
                decimals=int(row['decimals']),
                display_decimals=int(row['display_decimals']),
            )
            assets[asset.asset_id] = asset
    return assets

def load_symbols_config(filepath: str) -> List[SymbolConfig]:
    """Load symbols from CSV."""
    symbols = []
    with open(filepath, 'r') as f:
        reader = csv.DictReader(f)
        for row in reader:
            symbols.append(SymbolConfig(
                symbol_id=int(row['symbol_id']),
                symbol=row['symbol'],
                base_asset_id=int(row['base_asset_id']),
                quote_asset_id=int(row['quote_asset_id']),
                price_decimal=int(row['price_decimal']),
                price_display_decimal=int(row['price_display_decimal']),
            ))
    return symbols

# ============================================================
# RUNTIME CONFIGURATION
# ============================================================

DEFAULT_BASE_PRICE = 85000.00
DEFAULT_NUM_ACCOUNTS = 1000
DEFAULT_NUM_ORDERS = 1_000_000

# Price distribution parameters - tuned for more trades
# About 30% of orders should be "crossing" (marketable)
CROSSING_RATIO = 0.30      # 30% of orders cross the spread
SPREAD_PCT = 0.0005        # 0.05% spread (tighter)
PRICE_RANGE_PCT = 0.02     # 2% price range (narrower)

# Quantity distribution
MIN_QTY = 0.0001
MAX_QTY = 10.0
TYPICAL_QTY = 0.1

# Order type distribution
BUY_RATIO = 0.50

# Account initial balances
INITIAL_BASE_PER_ACCOUNT = 100.0
INITIAL_QUOTE_PER_ACCOUNT = 10_000_000.0

# ============================================================
# DATA STRUCTURES
# ============================================================

@dataclass
class Order:
    order_id: int
    user_id: int
    side: str
    price: str      # String format for client (e.g., "85000.12")
    qty: str      # String format for client (e.g., "0.12345678")

@dataclass
class BalanceRecord:
    """Single balance record (row-based like DB dump, supports N assets)"""
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

def generate_price(side: str, base_price: float, decimals: int) -> float:
    """Generate price with ~30% crossing orders for higher trade rate."""
    is_crossing = random.random() < CROSSING_RATIO
    
    if is_crossing:
        # Crossing order: price crosses the spread (will match immediately)
        cross_depth = random.expovariate(1 / 0.002)  # How much to cross
        cross_depth = min(cross_depth, 0.01)  # Cap at 1%
        
        if side == "buy":
            # Aggressive buy: above mid-price
            price = base_price * (1 + cross_depth)
        else:
            # Aggressive sell: below mid-price
            price = base_price * (1 - cross_depth)
    else:
        # Passive order: price on the correct side of spread
        distance_pct = random.expovariate(1 / 0.003)  # Tighter clustering
        distance_pct = min(distance_pct, PRICE_RANGE_PCT)
        
        if side == "buy":
            price = base_price * (1 - SPREAD_PCT/2 - distance_pct)
        else:
            price = base_price * (1 + SPREAD_PCT/2 + distance_pct)
    
    
    return round(price, decimals)

def generate_quantity(decimals: int) -> float:
    qty = random.lognormvariate(mu=math.log(TYPICAL_QTY), sigma=1.5)
    qty = max(MIN_QTY, min(MAX_QTY, qty))
    return round(qty, decimals)

# ============================================================
# GENERATION
# ============================================================

def generate_orders(num_orders: int, num_accounts: int, base_price: float,
                    base_display_decimals: int, quote_display_decimals: int) -> List[Order]:
    """Generate orders using display_decimals (client-facing format)."""
    orders = []
    current_price = base_price
    price_drift = 0.0001
    
    for i in range(num_orders):
        order_id = i + 1
        user_id = random.randint(1, num_accounts)
        side = "buy" if random.random() < BUY_RATIO else "sell"
        
        price = generate_price(side, current_price, quote_display_decimals)
        qty = generate_quantity(base_display_decimals)
        
        orders.append(Order(
            order_id=order_id,
            user_id=user_id,
            side=side,
            price=f"{price:.{quote_display_decimals}f}",
            qty=f"{qty:.{base_display_decimals}f}"
        ))
        
        if i % 1000 == 0:
            current_price *= (1 + random.gauss(0, price_drift))
        
        if (i + 1) % 100000 == 0:
            print(f"Generated {i + 1:,} / {num_orders:,} orders...")
    
    return orders

def generate_balances(num_accounts: int, symbol: SymbolConfig, 
                       base_decimals: int, quote_decimals: int) -> List[BalanceRecord]:
    """Generate initial balance records (one row per user per asset)."""
    records = []
    for user_id in range(1, num_accounts + 1):
        # Base asset (e.g., BTC)
        records.append(BalanceRecord(
            user_id=user_id,
            asset_id=symbol.base_asset_id,
            avail=to_units(INITIAL_BASE_PER_ACCOUNT, base_decimals),
            frozen=0,
            version=0
        ))
        # Quote asset (e.g., USDT)
        records.append(BalanceRecord(
            user_id=user_id,
            asset_id=symbol.quote_asset_id,
            avail=to_units(INITIAL_QUOTE_PER_ACCOUNT, quote_decimals),
            frozen=0,
            version=0
        ))
    return records

# ============================================================
# CSV OUTPUT
# ============================================================

def write_orders_csv(orders: List[Order], filepath: str):
    with open(filepath, 'w', newline='') as f:
        writer = csv.writer(f)
        writer.writerow(['order_id', 'user_id', 'side', 'price', 'qty'])
        for order in orders:
            writer.writerow([order.order_id, order.user_id, order.side, order.price, order.qty])
    print(f"Wrote {len(orders):,} orders to {filepath}")

def write_balances_csv(records: List[BalanceRecord], filepath: str):
    """Write balance records in row format (like DB dump)."""
    with open(filepath, 'w', newline='') as f:
        writer = csv.writer(f)
        # Row-based format: one row per (user_id, asset_id)
        writer.writerow(['user_id', 'asset_id', 'avail', 'frozen', 'version'])
        for r in records:
            writer.writerow([r.user_id, r.asset_id, r.avail, r.frozen, r.version])
    print(f"Wrote {len(records):,} balance records to {filepath}")

# ============================================================
# MAIN
# ============================================================

def main():
    parser = argparse.ArgumentParser(description='Generate test orders')
    parser.add_argument('--orders', type=int, default=DEFAULT_NUM_ORDERS)
    parser.add_argument('--accounts', type=int, default=DEFAULT_NUM_ACCOUNTS)
    parser.add_argument('--price', type=float, default=DEFAULT_BASE_PRICE)
    parser.add_argument('--fixtures-dir', type=str, default='fixtures')
    parser.add_argument('--seed', type=int, default=42)
    
    args = parser.parse_args()
    
    # Load configuration (single source of truth)
    assets_csv = os.path.join(args.fixtures_dir, 'assets_config.csv')
    symbols_csv = os.path.join(args.fixtures_dir, 'symbols_config.csv')
    
    if not os.path.exists(assets_csv) or not os.path.exists(symbols_csv):
        print(f"Error: Config files not found in {args.fixtures_dir}/")
        print("Required: assets_config.csv, symbols_config.csv")
        return
    
    assets = load_assets_config(assets_csv)
    symbols = load_symbols_config(symbols_csv)
    
    # Use first symbol
    symbol = symbols[0]
    base_asset = assets[symbol.base_asset_id]
    quote_asset = assets[symbol.quote_asset_id]
    
    random.seed(args.seed)
    
    print(f"\n=== Order Generator ===")
    print(f"Symbol: {symbol.symbol}")
    print(f"Base: {base_asset.asset} (decimals={base_asset.decimals})")
    print(f"Quote: {quote_asset.asset} (decimals={quote_asset.decimals})")
    print(f"Orders: {args.orders:,}")
    print(f"Accounts: {args.accounts:,}")
    print(f"Base Price: ${args.price:,.2f}")
    print(f"Seed: {args.seed}")
    print()
    
    os.makedirs(args.fixtures_dir, exist_ok=True)
    
    print("Generating orders (using display_decimals for client format)...")
    orders = generate_orders(args.orders, args.accounts, args.price,
                             base_asset.display_decimals, quote_asset.display_decimals)
    
    print("Generating balances...")
    balances = generate_balances(args.accounts, symbol, base_asset.decimals, quote_asset.decimals)
    
    print("\nWriting CSV files...")
    write_orders_csv(orders, os.path.join(args.fixtures_dir, 'orders.csv'))
    write_balances_csv(balances, os.path.join(args.fixtures_dir, 'balances_init.csv'))
    
    # Summary
    buy_orders = sum(1 for o in orders if o.side == "buy")
    avg_price = sum(float(o.price) for o in orders) / len(orders)
    avg_qty = sum(float(o.qty) for o in orders) / len(orders)
    
    print(f"\n=== Summary ===")
    print(f"Buy/Sell: {buy_orders:,} / {len(orders) - buy_orders:,}")
    print(f"Avg Price: ${avg_price:.2f}")
    print(f"Avg Qty: {avg_qty:.8f} {base_asset.asset}")
    print(f"\nDone!")

if __name__ == '__main__':
    main()
