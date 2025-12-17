#!/usr/bin/env python3
"""
verify_order_events.py

Verifies the correctness of order events in t2_order_events.csv.

Verification checks:
1. Accepted events matches Expected Count (from orders vs summary)
2. Cancelled events count
3. Order Lifecycle Consistency (Accepted -> Filled/Cancelled)
"""

import csv
import sys
from collections import defaultdict
from pathlib import Path

def load_events(path: str) -> list[dict]:
    with open(path, 'r') as f:
        reader = csv.DictReader(f)
        return list(reader)

def load_summary(path: str) -> dict:
    result = {}
    with open(path, 'r') as f:
        for line in f:
            if ':' in line:
                parts = line.split(':')
                if len(parts) >= 2:
                    key = parts[0].strip()
                    value_part = parts[1].strip()
                    if value_part:
                        words = value_part.split()
                        if words:
                            result[key] = words[0]
    return result

def verify_order_events(events_path: str, summary_path: str) -> tuple[bool, list[str]]:
    issues = []
    events = load_events(events_path)
    summary = load_summary(summary_path)

    print(f"Loaded {len(events)} order events")

    accepted_evts = [e for e in events if e['event_type'] == 'accepted']
    cancelled_evts = [e for e in events if e['event_type'] == 'cancelled']
    filled_evts = [e for e in events if e['event_type'] == 'filled']
    partial_evts = [e for e in events if e['event_type'] == 'partial_filled']
    rejected_evts = [e for e in events if e['event_type'] == 'rejected']

    print(f"  Accepted: {len(accepted_evts)}")
    print(f"  Cancelled: {len(cancelled_evts)}")
    print(f"  Filled: {len(filled_evts)}")
    print(f"  PartialFilled: {len(partial_evts)}")
    print(f"  Rejected: {len(rejected_evts)}")

    # Check 1: Accepted count vs Summary
    accepted_summary = int(summary.get('Accepted', 0))
    if len(accepted_evts) == accepted_summary:
        print(f"✅ Accepted events ({len(accepted_evts)}) matches Summary")
    else:
        issues.append(f"Accepted events ({len(accepted_evts)}) != Summary ({accepted_summary})")
        print(f"❌ {issues[-1]}")

    # Check 2: All events have order_id and user_id
    missing_ids = [e for e in events if not e['order_id'] or not e['user_id']]
    if not missing_ids:
        print("✅ All events have order_id and user_id")
    else:
        issues.append(f"{len(missing_ids)} events missing order_id or user_id")
        print(f"❌ {issues[-1]}")

    # Check 3: Cancelled orders should not be subsequently filled
    # (Simplified check: group by order, check terminal state)
    events_by_order = defaultdict(list)
    for e in events:
        events_by_order[e['order_id']].append(e)

    lifecycle_issues = 0
    for order_id, evts in events_by_order.items():
        types = [e['event_type'] for e in evts]
        
        # Check duplicates of terminal states
        if types.count('cancelled') > 1:
            lifecycle_issues += 1
            if lifecycle_issues <= 3: issues.append(f"Order {order_id} cancelled multiple times")
        if types.count('filled') > 1:
            lifecycle_issues += 1
            if lifecycle_issues <= 3: issues.append(f"Order {order_id} filled multiple times (possible if multiple fills logged as filled? No, should be partial)")

        # Check sequence: Cancelled then Filled -> Impossible
        # Find index of first cancel
        try:
            cancel_idx = types.index('cancelled')
            # Check if any fill comes after
            for i in range(cancel_idx + 1, len(types)):
                if types[i] in ('filled', 'partial_filled'):
                    lifecycle_issues += 1
                    if lifecycle_issues <= 3: issues.append(f"Order {order_id} filled after cancellation")
        except ValueError:
            pass # No cancel

        # Check sequence: Filled then Cancelled -> Impossible
        try:
            filled_idx = types.index('filled')
            # Check if any cancel comes after
            for i in range(filled_idx + 1, len(types)):
                if types[i] == 'cancelled':
                     lifecycle_issues += 1
                     if lifecycle_issues <= 3: issues.append(f"Order {order_id} cancelled after fill")
        except ValueError:
            pass

    if lifecycle_issues == 0:
        print(f"✅ Order lifecycle consistency checks passed ({len(events_by_order)} orders)")
    else:
        print(f"❌ {lifecycle_issues} lifecycle issues found")

    return len(issues) == 0, issues

def main():
    print("╔" + "═" * 60 + "╗")
    print("║     Order Events Verification                             ║")
    print("╚" + "═" * 60 + "╝")
    print()
    
    events_path = "output/t2_order_events.csv"
    summary_path = "output/t2_summary.txt"
    
    if not Path(events_path).exists():
        print(f"❌ Events file not found: {events_path}")
        sys.exit(1)
        
    is_valid, issues = verify_order_events(events_path, summary_path)
    
    print()
    if is_valid:
        print("✅ SUCCESS: All order event checks passed")
        sys.exit(0)
    else:
        print("❌ FAILURE: Order event checks failed")
        for i in issues:
            print(f"  - {i}")
        sys.exit(1)

if __name__ == "__main__":
    main()
