#!/usr/bin/env python3
"""
QA 0x14-c: Money Safety 完整测试套件

一键运行所有 QA 独立设计的测试用例:
- Agent A (激进派): 边缘测试
- Agent B (保守派): 核心验证  
- Agent C (安全专家): 安全审计

参考格式: scripts/tests/0x14b_matching/run_all_qa_tests.py

Usage:
    python3 scripts/tests/0x14c_money_safety/run_all_tests.py

Author: QA Engineer (Multi-Agent Design)
Date: 2025-12-31
"""

import sys
import os
import subprocess
import time

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))

# Test modules - 与 0x14b 格式一致
TEST_MODULES = [
    ("🔥 Agent A - Edge Cases (P0)", "test_agent_a_edge_cases.py"),
    ("🛡️ Agent B - Core Flow (P1)", "test_agent_b_core_flow.py"),
    ("🔐 Agent C - Security (P0)", "test_agent_c_security.py"),
    ("🎯 Agent D - Advanced Precision (P0)", "test_advanced_precision.py"),
    ("🔬 Agent E - Precision Compliance (P0)", "test_precision_compliance.py"),
]


def print_header():
    print("=" * 80)
    print("🧪 QA 0x14-c: Money Safety Complete Test Suite")
    print("=" * 80)
    print()
    print("Test Agents:")
    for name, _ in TEST_MODULES:
        print(f"  • {name}")
    print()
    print("Design: Multi-Agent QA with Cross-Review")
    print("Total Test Cases: 32")
    print()


def run_test_module(name: str, script: str) -> bool:
    """
    Run a test module and return success status
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
        print("⚠️  QA 0x14-c MONEY SAFETY: FAILURES DETECTED")
        print("=" * 80)
        print()
        print("Action Required:")
        print("  1. Review failed test output above")
        print("  2. Check Gateway logs for errors")
        print("  3. Report issues to Developer")
        return 1
    
    print("=" * 80)
    print("✅ QA 0x14-c MONEY SAFETY: ALL MODULES PASSED")
    print("=" * 80)
    return 0


if __name__ == "__main__":
    sys.exit(main())
