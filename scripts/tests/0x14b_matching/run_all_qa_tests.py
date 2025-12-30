#!/usr/bin/env python3
"""
QA 0x14-b: Order Commands 完整测试套件

一键运行所有 QA 独立设计的测试用例:
- IOC 测试 (7个)
- ReduceOrder 测试 (5个)
- MoveOrder 测试 (7个)
- GTC/Cancel 基线测试 (9个)
- 边界条件测试 (8个)

总计: 36 个功能性测试用例

Usage:
    python3 scripts/tests/0x14b_matching/run_all_qa_tests.py

Author: QA Engineer (Independent Design)
Date: 2025-12-30
"""

import sys
import os
import subprocess
import time

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))

# Test modules
TEST_MODULES = [
    ("IOC Tests (P0)", "test_ioc_qa.py"),
    ("MoveOrder Tests (P0)", "test_move_qa.py"),
    ("ReduceOrder Tests (P1)", "test_reduce_qa.py"),
    ("GTC/Cancel Baseline (P2)", "test_gtc_cancel_qa.py"),
    ("Edge Cases (P2)", "test_edge_cases_qa.py"),
]


def print_header():
    print("=" * 80)
    print("🧪 QA 0x14-b: Order Commands Complete Functional Test Suite")
    print("=" * 80)
    print()
    print("Test Modules:")
    for name, _ in TEST_MODULES:
        print(f"  • {name}")
    print()
    print("Focus: Functional Correctness (NOT Performance)")
    print()


def run_test_module(name: str, script: str) -> tuple:
    """
    Run a test module and return (passed, failed, skipped, errors)
    """
    script_path = os.path.join(SCRIPT_DIR, script)
    
    print()
    print("-" * 80)
    print(f"📦 Running: {name}")
    print("-" * 80)
    
    try:
        result = subprocess.run(
            [sys.executable, script_path],
            capture_output=False,
            timeout=300
        )
        return result.returncode == 0
    except subprocess.TimeoutExpired:
        print(f"  ⚠️ TIMEOUT: {name}")
        return False
    except Exception as e:
        print(f"  ⚠️ ERROR: {e}")
        return False


def main():
    print_header()
    
    start_time = time.time()
    
    results = []
    for name, script in TEST_MODULES:
        success = run_test_module(name, script)
        results.append((name, success))
    
    elapsed = time.time() - start_time
    
    # Final Summary
    print()
    print("=" * 80)
    print("📊 FINAL TEST SUMMARY")
    print("=" * 80)
    
    passed_modules = 0
    failed_modules = 0
    
    for name, success in results:
        if success:
            print(f"  ✅ {name}")
            passed_modules += 1
        else:
            print(f"  ❌ {name}")
            failed_modules += 1
    
    print()
    print(f"Modules: {passed_modules}/{len(results)} passed")
    print(f"Elapsed: {elapsed:.1f}s")
    print()
    
    if failed_modules > 0:
        print("=" * 80)
        print("⚠️  QA 0x14-b TEST SUITE: FAILURES DETECTED")
        print("=" * 80)
        print()
        print("Action Required:")
        print("  1. Review failed test output above")
        print("  2. Check Gateway logs for errors")
        print("  3. Report issues to Developer")
        return 1
    
    print("=" * 80)
    print("✅ QA 0x14-b TEST SUITE: ALL MODULES PASSED")
    print("=" * 80)
    return 0


if __name__ == "__main__":
    sys.exit(main())
