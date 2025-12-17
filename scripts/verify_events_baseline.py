#!/usr/bin/env python3
"""
verify_events_baseline.py

Verifies that the current event log matches the baseline.

Since events may be produced in different orders (pipeline non-determinism),
we verify by:
1. Grouping events by type (lock, settle, deposit)
2. For each type, sorting by a canonical key
3. Comparing the sorted sets

This handles interleaving differences while ensuring deterministic final state.
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


def canonical_key(event: dict) -> tuple:
    """
    Generate canonical key for sorting events.
    
    For lock events: (source_id, user_id, asset_id)
    For settle events: (source_id, user_id, asset_id)
    For deposit events: (user_id, asset_id)
    """
    event_type = event['event_type']
    user_id = int(event['user_id'])
    asset_id = int(event['asset_id'])
    source_id = int(event['source_id'])
    
    if event_type == 'deposit':
        return (event_type, user_id, asset_id)
    else:
        return (event_type, source_id, user_id, asset_id)


def canonical_value(event: dict) -> tuple:
    """
    Extract canonical value for comparison.
    
    With separated version spaces (lock_version vs settle_version),
    version is now deterministic and should be strictly compared.
    """
    return (
        event['event_type'],
        int(event['user_id']),
        int(event['asset_id']),
        event['source_type'],
        int(event['source_id']),
        int(event['version']),    # Now included: deterministic with separated spaces
        int(event['delta']),
        int(event['avail_after']),
        int(event['frozen_after']),
    )


def compare_events(output_events: list[dict], baseline_events: list[dict]) -> tuple[bool, list[str]]:
    """
    Compare events, ignoring order and version differences.
    
    Returns (is_match, differences)
    """
    differences = []
    
    # Group by event type
    output_by_type = defaultdict(list)
    baseline_by_type = defaultdict(list)
    
    for e in output_events:
        output_by_type[e['event_type']].append(e)
    for e in baseline_events:
        baseline_by_type[e['event_type']].append(e)
    
    # Check each type
    all_types = set(output_by_type.keys()) | set(baseline_by_type.keys())
    
    for event_type in sorted(all_types):
        output_list = output_by_type.get(event_type, [])
        baseline_list = baseline_by_type.get(event_type, [])
        
        print(f"  {event_type}: output={len(output_list)}, baseline={len(baseline_list)}", end=" ")
        
        if len(output_list) != len(baseline_list):
            differences.append(f"{event_type} count mismatch: {len(output_list)} vs {len(baseline_list)}")
            print("❌")
            continue
        
        # Sort by canonical key and compare values
        output_sorted = sorted(output_list, key=canonical_key)
        baseline_sorted = sorted(baseline_list, key=canonical_key)
        
        mismatches = 0
        for i, (o, b) in enumerate(zip(output_sorted, baseline_sorted)):
            o_val = canonical_value(o)
            b_val = canonical_value(b)
            if o_val != b_val:
                mismatches += 1
                if mismatches <= 3:
                    differences.append(f"{event_type}[{i}] mismatch: {o_val} vs {b_val}")
        
        if mismatches == 0:
            print("✅")
        else:
            print(f"❌ ({mismatches} mismatches)")
            if mismatches > 3:
                differences.append(f"... and {mismatches - 3} more {event_type} mismatches")
    
    return len(differences) == 0, differences


def main():
    print("╔" + "═" * 60 + "╗")
    print("║     Events Baseline Verification                          ║")
    print("╚" + "═" * 60 + "╝")
    print()
    
    output_path = "output/t2_events.csv"
    baseline_path = "baseline/t2_events.csv"
    
    # Check files exist
    if not Path(output_path).exists():
        print(f"❌ Output file not found: {output_path}")
        print("   Run with --ubscore mode first:")
        print("   cargo run --release -- --ubscore")
        sys.exit(1)
    
    if not Path(baseline_path).exists():
        print(f"❌ Baseline file not found: {baseline_path}")
        print("   Create baseline with:")
        print("   cp output/t2_events.csv baseline/t2_events.csv")
        sys.exit(1)
    
    # Load events
    print("Loading events...")
    output_events = load_events(output_path)
    baseline_events = load_events(baseline_path)
    
    print(f"  Output: {len(output_events)} events")
    print(f"  Baseline: {len(baseline_events)} events")
    print()
    
    # Quick count check
    if len(output_events) != len(baseline_events):
        print(f"❌ Event count mismatch: {len(output_events)} vs {len(baseline_events)}")
        sys.exit(1)
    
    # Compare events
    print("Comparing by event type...")
    is_match, differences = compare_events(output_events, baseline_events)
    
    print()
    if is_match:
        print("╔" + "═" * 60 + "╗")
        print("║     ✅ Events match baseline!                             ║")
        print("╚" + "═" * 60 + "╝")
        sys.exit(0)
    else:
        print("╔" + "═" * 60 + "╗")
        print("║     ❌ Events differ from baseline!                       ║")
        print("╚" + "═" * 60 + "╝")
        print("\nDifferences:")
        for diff in differences:
            print(f"  - {diff}")
        sys.exit(1)


if __name__ == "__main__":
    main()
