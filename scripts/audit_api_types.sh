#!/bin/bash
# =============================================================================
# API Type Safety Audit Script
# =============================================================================
# Purpose: Detect API type safety violations in gateway code
# 
# Checks:
# 1. u64/i64 amount fields in DTOs
# 2. Direct .parse::<u64>() in gateway
# 3. Direct Decimal::from_str bypassing StrictDecimal
#
# Usage: ./scripts/audit_api_types.sh
# Exit: 0 = pass, 1 = violations found
# =============================================================================

set -e

echo "🔍 API Type Safety Audit"
echo "========================"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Change to project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_ROOT"

echo "📁 Scanning: $PROJECT_ROOT/src/gateway/"
echo ""

VIOLATIONS=0

# =============================================================================
# Rule 1: No u64/i64 amount fields in DTOs
# =============================================================================

echo "Rule 1: Checking for u64/i64 amount fields in DTOs..."

# Amount field patterns
AMOUNT_PATTERNS="amount|quantity|price|balance|volume|size|qty|fee"

# Check for direct u64 amount fields (excluding order_id which is valid)
U64_AMOUNTS=$(grep -rn "pub\s\+\(${AMOUNT_PATTERNS}\)\s*:\s*u64" --include="*.rs" src/gateway/ 2>/dev/null || true)

if [ -n "$U64_AMOUNTS" ]; then
    echo -e "${RED}❌ FAIL: Found u64 amount fields in API DTO${NC}"
    echo "$U64_AMOUNTS"
    echo "   → Should use StrictDecimal type instead"
    VIOLATIONS=$((VIOLATIONS + 1))
else
    echo -e "${GREEN}✅ No u64 amount fields found${NC}"
fi

# Check for i64 amount fields
I64_AMOUNTS=$(grep -rn "pub\s\+\(${AMOUNT_PATTERNS}\)\s*:\s*i64" --include="*.rs" src/gateway/ 2>/dev/null || true)

if [ -n "$I64_AMOUNTS" ]; then
    echo -e "${RED}❌ FAIL: Found i64 amount fields in API DTO${NC}"
    echo "$I64_AMOUNTS"
    echo "   → Should use StrictDecimal type instead"
    VIOLATIONS=$((VIOLATIONS + 1))
else
    echo -e "${GREEN}✅ No i64 amount fields found${NC}"
fi

echo ""

# =============================================================================
# Rule 2: No direct .parse::<u64>() in gateway (outside tests)
# =============================================================================

echo "Rule 2: Checking for direct u64 parsing..."

# Exclude:
# - test code
# - // safe: comments
# - user_id parsing (not a money amount)
DIRECT_PARSE=$(grep -rn "\.parse::<u64>()" --include="*.rs" src/gateway/ 2>/dev/null | grep -v "#\[cfg(test)\]" | grep -v "// safe:" | grep -v "user_id" | grep -v "order_id" || true)

if [ -n "$DIRECT_PARSE" ]; then
    echo -e "${RED}❌ FAIL: Found direct u64 parsing in gateway${NC}"
    echo "$DIRECT_PARSE"
    echo "   → Should use SymbolManager.parse_qty() instead"
    VIOLATIONS=$((VIOLATIONS + 1))
else
    echo -e "${GREEN}✅ No direct u64 parsing found${NC}"
fi

echo ""

# =============================================================================
# Rule 3: Direct Decimal::from_str should use StrictDecimal (warning)
# =============================================================================

echo "Rule 3: Checking for Decimal::from_str usage (informational)..."

# This is informational - we check handlers.rs specifically
DECIMAL_PARSE=$(grep -rn "Decimal::from_str" --include="*.rs" src/gateway/handlers.rs 2>/dev/null | grep -v "// safe:" | grep -v "#\[cfg(test)\]" || true)

if [ -n "$DECIMAL_PARSE" ]; then
    echo -e "${YELLOW}⚠️  INFO: Direct Decimal::from_str found in handlers.rs${NC}"
    echo "$DECIMAL_PARSE"
    echo "   → Consider using StrictDecimal if this is user input"
    # Not a violation, just informational
else
    echo -e "${GREEN}✅ No direct Decimal::from_str in handlers${NC}"
fi

echo ""

# =============================================================================
# Rule 4: No f64 in DTOs (forbidden in financial systems)
# =============================================================================

echo "Rule 4: Checking for f64 in DTOs (forbidden)..."

F64_FIELDS=$(grep -rn "pub.*:\s*f64" --include="*.rs" src/gateway/ 2>/dev/null | grep -v "#\[cfg(test)\]" | grep -v "// safe:" || true)

if [ -n "$F64_FIELDS" ]; then
    echo -e "${RED}❌ FAIL: Found f64 fields in API DTO (forbidden in financial systems)${NC}"
    echo "$F64_FIELDS"
    echo "   → Should use DisplayAmount for output, StrictDecimal for input"
    VIOLATIONS=$((VIOLATIONS + 1))
else
    echo -e "${GREEN}✅ No f64 fields found (financial safety)${NC}"
fi

echo ""

# =============================================================================
# Rule 5: Raw Decimal in Response DTOs (informational)
# =============================================================================

echo "Rule 5: Checking for raw Decimal in Response DTOs (informational)..."

# This is informational for now - strict enforcement in Phase 2b
RAW_DECIMAL=$(grep -rn "pub.*:\s*Decimal\s*[,}]" --include="*.rs" src/gateway/types.rs 2>/dev/null | grep -v "StrictDecimal" | grep -v "#\[cfg(test)\]" || true)

if [ -n "$RAW_DECIMAL" ]; then
    echo -e "${YELLOW}⚠️  INFO: Raw Decimal found in types.rs${NC}"
    echo "$RAW_DECIMAL"
    echo "   → Consider using DisplayAmount for responses (Phase 2b)"
    # Not a violation, just informational
else
    echo -e "${GREEN}✅ No raw Decimal in response types${NC}"
fi

echo ""

# =============================================================================
# Summary
# =============================================================================

echo "========================"
if [ "$VIOLATIONS" -gt 0 ]; then
    echo -e "${RED}❌ API Type Safety Audit FAILED: $VIOLATIONS violations${NC}"
    exit 1
else
    echo -e "${GREEN}✅ API Type Safety Audit PASSED${NC}"
fi
echo ""
