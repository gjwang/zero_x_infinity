#!/usr/bin/env python3
"""
verify_baseline_equivalence.py

Verifies that the new baseline (with separated version spaces) is
equivalent to the old baseline (with single version).

Equivalence means: avail and frozen values are IDENTICAL
Only the 'version' column is allowed to differ.
"""

import csv
import subprocess
import sys
from pathlib import Path


def get_old_baseline(ref: str = "v0.8b-ubscore-implementation") -> list[dict]:
    """Extract old baseline from a git reference."""
    result = subprocess.run(
        ["git", "show", f"{ref}:baseline/t2_balances_final.csv"],
        capture_output=True,
        text=True,
        check=True,
    )
    lines = result.stdout.strip().split("\n")
    reader = csv.DictReader(lines)
    return list(reader)


def load_current_baseline(path: str = "baseline/t2_balances_final.csv") -> list[dict]:
    """Load current baseline from file."""
    with open(path, "r") as f:
        reader = csv.DictReader(f)
        return list(reader)


def compare_baselines(old: list[dict], new: list[dict]) -> tuple[bool, list[str]]:
    """
    Compare two baselines, ignoring version column.
    
    Returns (is_equivalent, differences)
    """
    if len(old) != len(new):
        return False, [f"Row count differs: old={len(old)}, new={len(new)}"]
    
    differences = []
    
    # Key fields that must be identical
    key_fields = ["user_id", "asset_id", "avail", "frozen"]
    
    for i, (old_row, new_row) in enumerate(zip(old, new)):
        for field in key_fields:
            if old_row.get(field) != new_row.get(field):
                differences.append(
                    f"Row {i+1}: {field} differs - "
                    f"old={old_row.get(field)}, new={new_row.get(field)}"
                )
    
    return len(differences) == 0, differences


def show_version_samples(old: list[dict], new: list[dict], n: int = 5):
    """Show sample version differences."""
    print("Format: user_id, asset_id | old_version -> new_version")
    print("-" * 50)
    
    for i, (old_row, new_row) in enumerate(zip(old[:n], new[:n])):
        user_id = old_row["user_id"]
        asset_id = old_row["asset_id"]
        old_ver = old_row["version"]
        new_ver = new_row["version"]
        print(f"  {user_id:>4}, {asset_id:>2} | {old_ver:>5} -> {new_ver:>5}")
    
    print("  ...")


def main():
    print("╔" + "═" * 60 + "╗")
    print("║     Baseline Equivalence Verification                      ║")
    print("╚" + "═" * 60 + "╝")
    print()
    
    ref = "v0.8b-ubscore-implementation"
    
    # Step 1: Load baselines
    print(f"=== Step 1: Extract old baseline from {ref} ===")
    try:
        old_baseline = get_old_baseline(ref)
        print(f"Old baseline: {len(old_baseline)} rows")
    except subprocess.CalledProcessError as e:
        print(f"❌ Failed to extract old baseline: {e}")
        sys.exit(1)
    
    print()
    print("=== Step 2: Load current baseline ===")
    try:
        new_baseline = load_current_baseline()
        print(f"New baseline: {len(new_baseline)} rows")
    except FileNotFoundError:
        print("❌ Current baseline not found")
        sys.exit(1)
    
    # Step 3: Compare
    print()
    print("=== Step 3: Compare avail and frozen values ===")
    is_equivalent, differences = compare_baselines(old_baseline, new_baseline)
    
    if is_equivalent:
        print("✅ EQUIVALENT: avail and frozen values are IDENTICAL")
        print()
        
        print("=== Sample version differences (expected) ===")
        show_version_samples(old_baseline, new_baseline)
        print()
        
        print("=== Explanation ===")
        print("Old version = all operations (lock + settle + deposit)")
        print("New version = lock_version only (lock + unlock + deposit)")
        print()
        print("The settle operations now increment a separate settle_version field.")
        print()
        
        print("╔" + "═" * 60 + "╗")
        print("║     ✅ Baseline equivalence verified!                      ║")
        print("╚" + "═" * 60 + "╝")
        sys.exit(0)
    else:
        print("❌ NOT EQUIVALENT: avail or frozen values differ!")
        print()
        print("=== Differences ===")
        for diff in differences[:10]:
            print(f"  {diff}")
        if len(differences) > 10:
            print(f"  ... and {len(differences) - 10} more")
        print()
        
        print("╔" + "═" * 60 + "╗")
        print("║     ❌ Baseline equivalence FAILED!                        ║")
        print("╚" + "═" * 60 + "╝")
        sys.exit(1)


if __name__ == "__main__":
    main()
