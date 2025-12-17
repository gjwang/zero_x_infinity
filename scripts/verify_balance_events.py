#!/usr/bin/env python3
"""
verify_balance_events.py

Verifies the correctness of balance events in t2_events.csv.

Verification checks:
1. Lock events count = accepted orders count
2. Settle events count = trades * 4 (buyer: 2, seller: 2)
3. Lock version continuity per user
4. Settle version continuity per user
5. Balance conservation: sum of deltas = 0 for settle events
"""

import csv
import sys
from collections import defaultdict
from pathlib import Path


def load_events(path: str) -> list[dict]:
    """Load events from CSV file."""
    with open(path, 'r') as f:
        reader = csv.DictReader(f)
        return list(reader)


def load_summary(path: str) -> dict:
    """Load summary from text file."""
    result = {}
    with open(path, 'r') as f:
        for line in f:
            if ':' in line:
                parts = line.split(':')
                if len(parts) >= 2:
                    key = parts[0].strip()
                    value_part = parts[1].strip()
                    if value_part:
                        # Get first word/number
                        words = value_part.split()
                        if words:
                            result[key] = words[0]
    return result


def verify_events(events_path: str, summary_path: str) -> tuple[bool, list[str]]:
    """
    Verify balance events correctness.
    
    Returns (is_valid, issues)
    """
    issues = []
    
    # Load data
    events = load_events(events_path)
    summary = load_summary(summary_path)
    
    print(f"Loaded {len(events)} events")
    
    # Count by event type
    lock_events = [e for e in events if e['event_type'] == 'lock']
    settle_events = [e for e in events if e['event_type'] == 'settle']
    unlock_events = [e for e in events if e['event_type'] == 'unlock']
    deposit_events = [e for e in events if e['event_type'] == 'deposit']
    
    print(f"  Lock events: {len(lock_events)}")
    print(f"  Settle events: {len(settle_events)}")
    print(f"  Unlock events: {len(unlock_events)}")
    print(f"  Deposit events: {len(deposit_events)}")
    
    # ========================================
    # Check 1: Lock events count = Accepted orders
    # ========================================
    print("\n=== Check 1: Lock events vs Accepted orders ===")
    accepted = int(summary.get('Accepted', 0))
    if len(lock_events) == accepted:
        print(f"✅ Lock events ({len(lock_events)}) = Accepted orders ({accepted})")
    else:
        issues.append(f"Lock events ({len(lock_events)}) != Accepted orders ({accepted})")
        print(f"❌ {issues[-1]}")
    
    # ========================================
    # Check 2: Settle events count = Trades * 4
    # ========================================
    print("\n=== Check 2: Settle events vs Trades ===")
    total_trades = int(summary.get('Total Trades', 0))
    expected_settle = total_trades * 4  # 2 per buyer, 2 per seller
    if len(settle_events) == expected_settle:
        print(f"✅ Settle events ({len(settle_events)}) = Trades * 4 ({expected_settle})")
    else:
        issues.append(f"Settle events ({len(settle_events)}) != Trades * 4 ({expected_settle})")
        print(f"❌ {issues[-1]}")
    
    # ========================================
    # Check 3: Lock version continuity
    # ========================================
    print("\n=== Check 3: Lock version continuity ===")
    lock_by_user_asset = defaultdict(list)
    for e in lock_events:
        key = (e['user_id'], e['asset_id'])
        lock_by_user_asset[key].append(int(e['version']))
    
    lock_version_issues = 0
    for (user_id, asset_id), versions in lock_by_user_asset.items():
        # Versions should be increasing (not necessarily consecutive due to deposits)
        for i in range(1, len(versions)):
            if versions[i] <= versions[i-1]:
                lock_version_issues += 1
                if lock_version_issues <= 3:  # Only show first 3
                    issues.append(f"Lock version not increasing: user={user_id}, asset={asset_id}, versions={versions[i-1]}→{versions[i]}")
    
    if lock_version_issues == 0:
        print(f"✅ All lock versions are increasing ({len(lock_by_user_asset)} user-asset pairs)")
    else:
        print(f"❌ {lock_version_issues} lock version issues found")
    
    # ========================================
    # Check 4: Settle version continuity
    # ========================================
    print("\n=== Check 4: Settle version continuity ===")
    settle_by_user_asset = defaultdict(list)
    for e in settle_events:
        key = (e['user_id'], e['asset_id'])
        settle_by_user_asset[key].append(int(e['version']))
    
    settle_version_issues = 0
    for (user_id, asset_id), versions in settle_by_user_asset.items():
        # Versions should be increasing
        for i in range(1, len(versions)):
            if versions[i] < versions[i-1]:  # Allow equal (same trade, multiple ops)
                settle_version_issues += 1
                if settle_version_issues <= 3:
                    issues.append(f"Settle version not increasing: user={user_id}, asset={asset_id}")
    
    if settle_version_issues == 0:
        print(f"✅ All settle versions are increasing ({len(settle_by_user_asset)} user-asset pairs)")
    else:
        print(f"❌ {settle_version_issues} settle version issues found")
    
    # ========================================
    # Check 5: Balance conservation (settle events)
    # ========================================
    print("\n=== Check 5: Settle delta conservation by trade ===")
    settle_by_trade = defaultdict(list)
    for e in settle_events:
        trade_id = e['source_id']
        settle_by_trade[trade_id].append(int(e['delta']))
    
    conservation_issues = 0
    for trade_id, deltas in settle_by_trade.items():
        if sum(deltas) != 0:
            conservation_issues += 1
            if conservation_issues <= 3:
                issues.append(f"Trade {trade_id} delta sum = {sum(deltas)} (should be 0)")
    
    if conservation_issues == 0:
        print(f"✅ All trades have zero sum delta ({len(settle_by_trade)} trades)")
    else:
        print(f"❌ {conservation_issues} trades have non-zero delta sum")
    
    # ========================================
    # Check 6: Source type consistency
    # ========================================
    print("\n=== Check 6: Source type consistency ===")
    lock_sources = set(e['source_type'] for e in lock_events)
    settle_sources = set(e['source_type'] for e in settle_events)
    
    if lock_sources == {'order'}:
        print(f"✅ All lock events have source_type='order'")
    else:
        issues.append(f"Lock events have unexpected source_types: {lock_sources}")
        print(f"❌ {issues[-1]}")
    
    if settle_sources == {'trade'}:
        print(f"✅ All settle events have source_type='trade'")
    else:
        issues.append(f"Settle events have unexpected source_types: {settle_sources}")
        print(f"❌ {issues[-1]}")
    
    return len(issues) == 0, issues


def main():
    print("╔" + "═" * 60 + "╗")
    print("║     Balance Events Verification                           ║")
    print("╚" + "═" * 60 + "╝")
    print()
    
    # Check if files exist
    events_path = "output/t2_events.csv"
    summary_path = "output/t2_summary.txt"
    
    if not Path(events_path).exists():
        print(f"❌ Events file not found: {events_path}")
        print("   Run with --ubscore mode first:")
        print("   cargo run --release -- --ubscore")
        sys.exit(1)
    
    if not Path(summary_path).exists():
        print(f"❌ Summary file not found: {summary_path}")
        sys.exit(1)
    
    # Run verification
    is_valid, issues = verify_events(events_path, summary_path)
    
    print()
    if is_valid:
        print("╔" + "═" * 60 + "╗")
        print("║     ✅ All balance event checks passed!                   ║")
        print("╚" + "═" * 60 + "╝")
        sys.exit(0)
    else:
        print("╔" + "═" * 60 + "╗")
        print("║     ❌ Some checks failed!                                ║")
        print("╚" + "═" * 60 + "╝")
        print("\nIssues found:")
        for issue in issues:
            print(f"  - {issue}")
        sys.exit(1)


if __name__ == "__main__":
    main()
