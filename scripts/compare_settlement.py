#!/usr/bin/env python3
"""
compare_settlement.py - Compare Two Balance CSV Files
======================================================

PURPOSE:
    Compare Pipeline output CSV with TDengine dump CSV.
    Performs 100% field-level comparison for settlement verification.

INPUT FILES:
    - Pipeline output: user_id,asset_id,avail,frozen,version
    - TDengine dump:   user_id,asset_id,avail,frozen,lock_version,settle_version

USAGE:
    python3 scripts/compare_settlement.py \\
        --pipeline output/t2_balances_final.csv \\
        --db db_balances.csv

EXIT CODES:
    0 = All fields match 100%
    1 = Comparison failed (mismatches found)
    2 = File/setup error
"""

import argparse
import csv
import sys
from typing import Dict, List, Tuple, Any

# Colors
class Colors:
    RED = '\033[0;31m'
    GREEN = '\033[0;32m'
    YELLOW = '\033[1;33m'
    NC = '\033[0m'

def disable_colors():
    Colors.RED = ''
    Colors.GREEN = ''
    Colors.YELLOW = ''
    Colors.NC = ''


def load_pipeline_csv(path: str) -> Dict[Tuple[int, int], Dict[str, Any]]:
    """
    Load Pipeline output CSV.
    Format: user_id,asset_id,avail,frozen,version
    """
    data = {}
    with open(path, 'r') as f:
        reader = csv.DictReader(f)
        for row in reader:
            key = (int(row['user_id']), int(row['asset_id']))
            data[key] = {
                'avail': int(row['avail']),
                'frozen': int(row['frozen']),
                'version': int(row['version']),
            }
    return data


def load_db_csv(path: str) -> Dict[Tuple[int, int], Dict[str, Any]]:
    """
    Load TDengine dump CSV.
    Format: user_id,asset_id,avail,frozen,lock_version,settle_version
    """
    data = {}
    with open(path, 'r') as f:
        reader = csv.DictReader(f)
        for row in reader:
            key = (int(row['user_id']), int(row['asset_id']))
            data[key] = {
                'avail': int(row['avail']),
                'frozen': int(row['frozen']),
                'lock_version': int(row['lock_version']),
                'settle_version': int(row['settle_version']),
            }
    return data


def compare_balances(
    pipeline: Dict[Tuple[int, int], Dict[str, Any]],
    db: Dict[Tuple[int, int], Dict[str, Any]],
    verbose: bool = False
) -> Tuple[int, int, int, List[str]]:
    """
    Compare Pipeline vs DB balances.
    
    Returns: (matched, mismatched, missing, errors)
    """
    matched = 0
    mismatched = 0
    missing = 0
    errors = []
    
    for key, p_data in pipeline.items():
        user_id, asset_id = key
        
        if key not in db:
            missing += 1
            errors.append(f"MISSING in DB: user={user_id}, asset={asset_id}")
            continue
        
        db_data = db[key]
        
        # Compare fields
        field_errors = []
        
        # avail comparison
        if p_data['avail'] != db_data['avail']:
            field_errors.append(f"avail: {p_data['avail']} != {db_data['avail']}")
        
        # frozen comparison
        if p_data['frozen'] != db_data['frozen']:
            field_errors.append(f"frozen: {p_data['frozen']} != {db_data['frozen']}")
        
        # version comparison (Pipeline.version == DB.lock_version)
        if p_data['version'] != db_data['lock_version']:
            field_errors.append(f"version: {p_data['version']} != lock_version: {db_data['lock_version']}")
        
        if field_errors:
            mismatched += 1
            errors.append(f"MISMATCH user={user_id}, asset={asset_id}: {'; '.join(field_errors)}")
        else:
            matched += 1
            if verbose:
                print(f"  ✓ user={user_id}, asset={asset_id}")
    
    # Check for extra records in DB not in Pipeline
    for key in db:
        if key not in pipeline:
            user_id, asset_id = key
            errors.append(f"EXTRA in DB: user={user_id}, asset={asset_id}")
    
    return matched, mismatched, missing, errors


def main():
    parser = argparse.ArgumentParser(description='Compare Pipeline CSV with TDengine CSV')
    parser.add_argument('--pipeline', '-p', required=True, help='Pipeline output CSV')
    parser.add_argument('--db', '-d', required=True, help='TDengine dump CSV')
    parser.add_argument('--verbose', '-v', action='store_true', help='Show all matches')
    parser.add_argument('--no-color', action='store_true', help='Disable colored output')
    parser.add_argument('--max-errors', type=int, default=20, help='Max errors to display')
    args = parser.parse_args()
    
    if args.no_color or not sys.stdout.isatty():
        disable_colors()
    
    print("╔════════════════════════════════════════════════════════════╗")
    print("║    Settlement Comparison: Pipeline CSV vs DB CSV          ║")
    print("╚════════════════════════════════════════════════════════════╝")
    print()
    
    # Load files
    print(f"[1] Loading Pipeline CSV: {args.pipeline}")
    try:
        pipeline = load_pipeline_csv(args.pipeline)
        print(f"    {Colors.GREEN}✓{Colors.NC} Loaded {len(pipeline)} records")
    except Exception as e:
        print(f"    {Colors.RED}✗{Colors.NC} Error: {e}")
        return 2
    
    print(f"[2] Loading DB CSV: {args.db}")
    try:
        db = load_db_csv(args.db)
        print(f"    {Colors.GREEN}✓{Colors.NC} Loaded {len(db)} records")
    except Exception as e:
        print(f"    {Colors.RED}✗{Colors.NC} Error: {e}")
        return 2
    
    # Compare
    print(f"\n[3] Comparing {len(pipeline)} Pipeline records vs {len(db)} DB records...")
    matched, mismatched, missing, errors = compare_balances(pipeline, db, args.verbose)
    
    # Results
    print("\n" + "=" * 60)
    print("Comparison Results")
    print("=" * 60)
    print(f"\nPipeline records: {len(pipeline)}")
    print(f"DB records:       {len(db)}")
    print()
    print(f"Matched:    {matched}")
    print(f"Mismatched: {mismatched}")
    print(f"Missing:    {missing}")
    
    if errors:
        print(f"\nFirst {min(args.max_errors, len(errors))} issues:")
        for err in errors[:args.max_errors]:
            print(f"  {Colors.RED}•{Colors.NC} {err}")
        if len(errors) > args.max_errors:
            print(f"  ... and {len(errors) - args.max_errors} more")
    
    print()
    
    if mismatched == 0 and missing == 0:
        print(f"{Colors.GREEN}╔════════════════════════════════════════════════════════════╗{Colors.NC}")
        print(f"{Colors.GREEN}║       ✅ 100% FIELD-LEVEL MATCH                           ║{Colors.NC}")
        print(f"{Colors.GREEN}╚════════════════════════════════════════════════════════════╝{Colors.NC}")
        return 0
    else:
        print(f"{Colors.RED}╔════════════════════════════════════════════════════════════╗{Colors.NC}")
        print(f"{Colors.RED}║       ❌ COMPARISON FAILED                                 ║{Colors.NC}")
        print(f"{Colors.RED}╚════════════════════════════════════════════════════════════╝{Colors.NC}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
