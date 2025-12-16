#!/usr/bin/env python3
"""Generate performance report in Markdown format for GitHub Pages."""

import os
import sys
from datetime import datetime

def read_perf_file(path):
    """Read perf file into dict."""
    data = {}
    if not os.path.exists(path):
        return data
    with open(path) as f:
        for line in f:
            line = line.strip()
            if '=' in line:
                key, value = line.split('=', 1)
                try:
                    if '.' in value:
                        data[key] = float(value)
                    else:
                        data[key] = int(value)
                except ValueError:
                    data[key] = value
    return data

def format_ns(ns):
    """Format nanoseconds to human readable."""
    if ns >= 1e9:
        return f"{ns/1e9:.2f}s"
    elif ns >= 1e6:
        return f"{ns/1e6:.2f}ms"
    elif ns >= 1e3:
        return f"{ns/1e3:.1f}µs"
    else:
        return f"{ns:.0f}ns"

def calc_change(baseline, current):
    """Calculate percentage change."""
    if baseline == 0:
        return 0.0
    return ((current - baseline) / baseline) * 100

def main():
    baseline_path = "baseline/t2_perf.txt"
    current_path = "output/t2_perf.txt"
    
    baseline = read_perf_file(baseline_path)
    current = read_perf_file(current_path)
    
    now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    
    print("# Performance Report")
    print()
    print(f"**Generated**: {now}")
    print()
    
    # Summary
    print("## Summary")
    print()
    print("| Metric | Baseline | Current | Change |")
    print("|--------|----------|---------|--------|")
    
    orders = int(current.get('orders', 0))
    trades = int(current.get('trades', 0))
    print(f"| Orders | {int(baseline.get('orders', 0)):,} | {orders:,} | - |")
    print(f"| Trades | {int(baseline.get('trades', 0)):,} | {trades:,} | - |")
    
    b_time = baseline.get('exec_time_ms', 0)
    c_time = current.get('exec_time_ms', 0)
    change = calc_change(b_time, c_time)
    print(f"| Exec Time | {b_time:.2f}ms | {c_time:.2f}ms | {change:+.1f}% |")
    
    b_ops = baseline.get('throughput_ops', 0)
    c_ops = current.get('throughput_ops', 0)
    change = calc_change(b_ops, c_ops)
    print(f"| Throughput | {int(b_ops):,}/s | {int(c_ops):,}/s | {change:+.1f}% |")
    print()
    
    # Timing Breakdown
    print("## Timing Breakdown")
    print()
    print("| Component | Time | OPS | % of Total |")
    print("|-----------|------|-----|------------|")
    
    total_ns = sum([
        current.get('balance_check_ns', 0),
        current.get('matching_ns', 0),
        current.get('settlement_ns', 0),
        current.get('ledger_ns', 0),
    ])
    
    components = [
        ("Balance Check", "balance_check_ns"),
        ("Matching Engine", "matching_ns"),
        ("Settlement", "settlement_ns"),
        ("Ledger I/O", "ledger_ns"),
    ]
    
    for name, key in components:
        ns = current.get(key, 0)
        ops = int(orders / (ns / 1e9)) if ns > 0 else 0
        ops_str = f"{ops/1e6:.1f}M" if ops >= 1e6 else f"{ops/1e3:.0f}K"
        pct = (ns / total_ns * 100) if total_ns > 0 else 0
        print(f"| {name} | {format_ns(ns)} | {ops_str} | {pct:.1f}% |")
    print()
    
    # Latency Percentiles
    print("## Latency Percentiles")
    print()
    print("| Percentile | Value |")
    print("|------------|-------|")
    
    latencies = [
        ("MIN", "latency_min_ns"),
        ("AVG", "latency_avg_ns"),
        ("P50", "latency_p50_ns"),
        ("P99", "latency_p99_ns"),
        ("P99.9", "latency_p999_ns"),
        ("MAX", "latency_max_ns"),
    ]
    
    for name, key in latencies:
        ns = current.get(key, 0)
        print(f"| {name} | {format_ns(ns)} |")
    print()
    
    # Verdict
    regressions = []
    
    b_time = baseline.get('exec_time_ms', 0)
    c_time = current.get('exec_time_ms', 0)
    if b_time > 0 and calc_change(b_time, c_time) > 5:
        regressions.append(("Exec Time", calc_change(b_time, c_time)))
    
    b_ops = baseline.get('throughput_ops', 0)
    c_ops = current.get('throughput_ops', 0)
    if b_ops > 0 and calc_change(b_ops, c_ops) < -5:
        regressions.append(("Throughput", calc_change(b_ops, c_ops)))
    
    print("## Verdict")
    print()
    if regressions:
        print(f"❌ **{len(regressions)} regression(s) detected**")
        print()
        for name, pct in regressions:
            print(f"- {name}: {pct:+.1f}%")
    else:
        print("✅ **No significant regressions detected**")

if __name__ == "__main__":
    main()
