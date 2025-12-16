#!/usr/bin/env python3
"""
Performance comparison script for 0xInfinity matching engine.
Compares baseline/t2_perf.txt with output/t2_perf.txt and shows percentage change.
"""

import sys
from pathlib import Path

BASELINE_FILE = "baseline/t2_perf.txt"
OUTPUT_FILE = "output/t2_perf.txt"

def parse_perf_file(path: str) -> dict:
    """Parse key=value perf file into dictionary."""
    data = {}
    try:
        with open(path) as f:
            for line in f:
                line = line.strip()
                if line.startswith('#') or '=' not in line:
                    continue
                key, value = line.split('=', 1)
                # Try to parse as number
                try:
                    data[key] = float(value)
                except ValueError:
                    data[key] = value
    except FileNotFoundError:
        return None
    return data

def format_ns(ns: float) -> str:
    """Format nanoseconds to human readable string."""
    if ns >= 1_000_000:
        return f"{ns/1_000_000:.2f}ms"
    elif ns >= 1_000:
        return f"{ns/1_000:.1f}µs"
    else:
        return f"{ns:.0f}ns"

def format_change(change: float, metric_type: str) -> str:
    """Format change percentage."""
    return f"{change:+.1f}%"

def calc_change(baseline: float, current: float) -> float:
    """Calculate percentage change."""
    if baseline == 0:
        return 0.0
    return ((current - baseline) / baseline) * 100

def print_header():
    print("╔════════════════════════════════════════════════════════════════════════╗")
    print("║                    Performance Comparison Report                       ║")
    print("╚════════════════════════════════════════════════════════════════════════╝")
    print()

def print_row(label: str, baseline, current, change_str: str = "-"):
    print(f"{label:25} {str(baseline):>15} {str(current):>15} {change_str:>12}")

def print_separator(title: str = None):
    print("─" * 75)
    if title:
        print(title)
        print()

def main():
    baseline = parse_perf_file(BASELINE_FILE)
    current = parse_perf_file(OUTPUT_FILE)
    
    if baseline is None:
        print(f"❌ Baseline not found: {BASELINE_FILE}")
        print("   Run: cargo run --release -- --baseline")
        sys.exit(1)
    
    if current is None:
        print(f"❌ Output not found: {OUTPUT_FILE}")
        print("   Run: cargo run --release")
        sys.exit(1)
    
    print_header()
    
    # Header
    print(f"{'Metric':25} {'Baseline':>15} {'Current':>15} {'Change':>12}")
    print_separator()
    
    # Orders & Trades (no change calc)
    print_row("Orders", int(baseline.get('orders', 0)), int(current.get('orders', 0)))
    print_row("Trades", int(baseline.get('trades', 0)), int(current.get('trades', 0)))
    print()
    
    # Track regressions
    regressions = []
    
    # Execution Time
    b_time = baseline.get('exec_time_ms', 0)
    c_time = current.get('exec_time_ms', 0)
    change = calc_change(b_time, c_time)
    print_row("Exec Time", f"{b_time:.2f}ms", f"{c_time:.2f}ms", format_change(change, "time"))
    if change > 5:  # time increased = regression
        regressions.append(("Exec Time", change))
    
    # Throughput
    b_ops = baseline.get('throughput_ops', 0)
    c_ops = current.get('throughput_ops', 0)
    change = calc_change(b_ops, c_ops)
    print_row("Throughput (orders)", f"{int(b_ops)}/s", f"{int(c_ops)}/s", format_change(change, "throughput"))
    if change < -5:  # throughput decreased = regression
        regressions.append(("Throughput", change))
    
    b_tps = baseline.get('throughput_tps', 0)
    c_tps = current.get('throughput_tps', 0)
    change = calc_change(b_tps, c_tps)
    print_row("Throughput (trades)", f"{int(b_tps)}/s", f"{int(c_tps)}/s", format_change(change, "throughput"))
    
    # Timing Breakdown
    print()
    print_separator("Timing Breakdown (lower is better):")
    print(f"{'Metric':25} {'Baseline':>15} {'Current':>15} {'Change':>12}")
    
    timing_metrics = [
        ("Balance Check", "balance_check_ns"),
        ("Matching Engine", "matching_ns"),
        ("Settlement", "settlement_ns"),
        ("Ledger I/O", "ledger_ns"),
    ]
    
    for label, key in timing_metrics:
        b_val = baseline.get(key, 0)
        c_val = current.get(key, 0)
        if b_val > 0:
            change = calc_change(b_val, c_val)
            print_row(label, format_ns(b_val), format_ns(c_val), format_change(change, "time"))
            if change > 10:  # >10% slower = notable regression
                regressions.append((label, change))
    
    # Latency Percentiles
    print()
    print_separator("Latency Percentiles (lower is better):")
    print(f"{'Metric':25} {'Baseline':>15} {'Current':>15} {'Change':>12}")
    
    latency_metrics = [
        ("Latency MIN", "latency_min_ns"),
        ("Latency AVG", "latency_avg_ns"),
        ("Latency P50", "latency_p50_ns"),
        ("Latency P99", "latency_p99_ns"),
        ("Latency P99.9", "latency_p999_ns"),
        ("Latency MAX", "latency_max_ns"),
    ]
    
    for label, key in latency_metrics:
        b_val = baseline.get(key, 0)
        c_val = current.get(key, 0)
        if b_val > 0:
            change = calc_change(b_val, c_val)
            print_row(label, format_ns(b_val), format_ns(c_val), format_change(change, "time"))
    
    # Final verdict
    print()
    print_separator()
    if regressions:
        print(f"❌ Found {len(regressions)} regression(s):")
        for name, pct in regressions:
            print(f"   - {name}: {pct:+.1f}%")
    else:
        print("✅ No significant regressions detected")
    print()

if __name__ == "__main__":
    main()
