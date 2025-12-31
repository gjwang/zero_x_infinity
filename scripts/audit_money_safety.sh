#!/bin/bash
# =============================================================================
# Money Safety Audit Script
# =============================================================================
# Purpose: Detect 10u64.pow usage outside allowed locations
# Allowed: money.rs, #[cfg(test)] blocks
# 
# Usage: ./scripts/audit_money_safety.sh
# Exit: 0 = pass, 1 = violations found
# =============================================================================

set -e

echo "🔍 Money Safety Audit"
echo "====================="
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

echo "📁 Scanning: $PROJECT_ROOT/src/"
echo ""

# =============================================================================
# Rule 1: 10u64.pow must only appear in money.rs or test code
# =============================================================================

echo "Rule 1: Checking 10u64.pow usage..."

# Find all 10u64.pow occurrences
ALL_POW=$(grep -rn "10u64\.pow" --include="*.rs" src/ 2>/dev/null || true)

if [ -z "$ALL_POW" ]; then
    echo -e "${GREEN}✅ No 10u64.pow found in codebase${NC}"
else
    # Filter out allowed locations
    # Allowed: money.rs (any line)
    # Allowed: lines within test modules (heuristic: file contains #[cfg(test)] before the line)
    
    VIOLATIONS=""
    
    while IFS= read -r line; do
        FILE=$(echo "$line" | cut -d: -f1)
        LINENUM=$(echo "$line" | cut -d: -f2)
        
        # Skip money.rs - it's the allowed core
        if [[ "$FILE" == *"money.rs" ]]; then
            continue
        fi
        
        # Check if line is within a test module
        # Heuristic: Check if there's #[cfg(test)] or #[test] nearby
        CONTEXT=$(sed -n "1,${LINENUM}p" "$FILE" | tail -50 | grep -c "#\[cfg(test)\]\|#\[test\]" || true)
        
        if [ "$CONTEXT" -gt 0 ]; then
            # Likely in test code - allowed
            continue
        fi
        
        # This is a violation
        VIOLATIONS="${VIOLATIONS}${line}\n"
        
    done <<< "$ALL_POW"
    
    if [ -n "$VIOLATIONS" ]; then
        echo -e "${RED}❌ VIOLATIONS FOUND:${NC}"
        echo -e "$VIOLATIONS"
        echo ""
        echo "These files contain 10u64.pow outside allowed locations."
        echo "Please refactor to use money::unit_amount() or intent-based API."
        exit 1
    else
        echo -e "${GREEN}✅ All 10u64.pow usage is in allowed locations${NC}"
    fi
fi

echo ""

# =============================================================================
# Rule 2: Direct money:: calls should only be in core modules (informational)
# =============================================================================

echo "Rule 2: Checking direct money:: calls (informational)..."

# This is informational for now - strict enforcement in Phase 4
MONEY_CALLS=$(grep -rn "money::" --include="*.rs" src/ \
    | grep -v "money.rs" \
    | grep -v "symbol_manager.rs" \
    | grep -v "// @money-delegate" \
    | grep -v "#\[cfg(test)\]" \
    2>/dev/null || true)

if [ -n "$MONEY_CALLS" ]; then
    echo -e "${YELLOW}⚠️  Direct money:: calls found (Phase 4 migration):${NC}"
    echo "$MONEY_CALLS" | head -20
    TOTAL=$(echo "$MONEY_CALLS" | wc -l | tr -d ' ')
    if [ "$TOTAL" -gt 20 ]; then
        echo "... and $((TOTAL - 20)) more"
    fi
    echo ""
fi

echo ""

# =============================================================================
# Rule 3: FORBIDDEN - unwrap_or with hardcoded amount values
# Section 4.5.1: 绝不允许未经定义的默认值
# =============================================================================

echo "Rule 3: Checking for hardcoded fallback values..."

# Pattern: .unwrap_or(100_000_000) or similar hardcoded amounts (6+ digits)
# Only catches money-sized constants, not limit/pagination values
FALLBACK_VIOLATIONS=$(grep -rn "unwrap_or(100_000_000\|unwrap_or(1_000_000\|unwrap_or(10_000_000" --include="*.rs" src/ \
    | grep -v "#\[cfg(test)\]" \
    | grep -v "#\[test\]" \
    2>/dev/null || true)

if [ -n "$FALLBACK_VIOLATIONS" ]; then
    echo -e "${RED}❌ FORBIDDEN: Hardcoded fallback values found!${NC}"
    echo -e "${RED}   See money-type-safety.md Section 4.5.1${NC}"
    echo ""
    echo "$FALLBACK_VIOLATIONS"
    echo ""
    echo "Fix: Use fail-fast error handling instead of hardcoded defaults."
    echo "Example:"
    echo "  let qty_unit = symbol_info.ok_or(Error::SymbolNotFound)?;"
    echo "  NOT: symbol_info.map(...).unwrap_or(100_000_000)"
    HAS_VIOLATIONS=true
else
    echo -e "${GREEN}✅ No hardcoded fallback values found${NC}"
fi

echo ""

# =============================================================================
# Rule 4: WARNING - f64 arithmetic on money amounts
# =============================================================================

echo "Rule 4: Checking for f64 money arithmetic..."

# Pattern: as f64 / 100_000_000 (or similar divisions with money constants)
F64_MONEY=$(grep -rn "as f64.*100" --include="*.rs" src/ \
    | grep -v "#\[cfg(test)\]" \
    | grep -v "perf.rs" \
    | grep -v "bench" \
    2>/dev/null || true)

if [ -n "$F64_MONEY" ]; then
    echo -e "${YELLOW}⚠️  Potential f64 money arithmetic found:${NC}"
    echo "$F64_MONEY"
    echo ""
    echo "Warning: Using f64 for money calculations can cause precision loss."
    echo "Fix: Use Decimal arithmetic: Decimal::from(sat) / Decimal::from(100_000_000)"
fi

echo ""

# =============================================================================
# Summary
# =============================================================================

echo "====================="
if [ "$HAS_VIOLATIONS" = true ]; then
    echo -e "${RED}❌ Money Safety Audit FAILED${NC}"
    exit 1
else
    echo -e "${GREEN}✅ Money Safety Audit PASSED${NC}"
fi
echo ""
