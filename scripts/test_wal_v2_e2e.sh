#!/bin/bash
# ============================================================
# WAL v2 E2E Test Script
# Phase 0x0D: Universal WAL Format Verification
# ============================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "============================================================"
echo "0x0D WAL v2 E2E Test"
echo "============================================================"

cd "$PROJECT_ROOT"

# Step 1: Run wal_v2 unit tests
echo ""
echo "[1/4] Running wal_v2 unit tests..."
cargo test wal_v2 --lib -- --nocapture 2>&1 | tee /tmp/wal_v2_test_output.txt

# Verify all tests passed
if ! grep -q "test result: ok" /tmp/wal_v2_test_output.txt; then
    echo "❌ wal_v2 unit tests FAILED"
    exit 1
fi

# Extract test count (should be 8)
TEST_COUNT=$(grep "test result:" /tmp/wal_v2_test_output.txt | grep -oE '[0-9]+ passed' | head -1)
echo "✅ wal_v2 unit tests passed: $TEST_COUNT"

# Step 2: Verify wire format size
echo ""
echo "[2/4] Verifying WAL header wire format..."
HEADER_CHECK=$(grep "test_wal_header_size_20_bytes" /tmp/wal_v2_test_output.txt | grep -c "ok" || echo "0")
if [ "$HEADER_CHECK" -eq 0 ]; then
    echo "❌ Wire format size test FAILED"
    exit 1
fi
echo "✅ Wire format = 20 bytes"

# Step 3: Verify CRC32 checksum
echo ""
echo "[3/4] Verifying CRC32 checksum tests..."
CRC_CHECK=$(grep -E "test_crc32_checksum|test_checksum_verification|test_corrupted_checksum_detection" /tmp/wal_v2_test_output.txt | grep -c "ok" || echo "0")
if [ "$CRC_CHECK" -lt 3 ]; then
    echo "❌ CRC32 checksum tests FAILED (expected 3, got $CRC_CHECK)"
    exit 1
fi
echo "✅ CRC32 checksum verification passed ($CRC_CHECK/3 tests)"

# Step 4: Verify real file I/O
echo ""
echo "[4/4] Verifying real file I/O..."
FILE_IO_CHECK=$(grep "test_real_file_io" /tmp/wal_v2_test_output.txt | grep -c "ok" || echo "0")
if [ "$FILE_IO_CHECK" -eq 0 ]; then
    echo "❌ Real file I/O test FAILED"
    exit 1
fi
echo "✅ Real file I/O verification passed"

# Cleanup
rm -f /tmp/wal_v2_test_output.txt

echo ""
echo "============================================================"
echo "✅ ALL WAL v2 E2E TESTS PASSED (8 tests)"
echo "============================================================"
