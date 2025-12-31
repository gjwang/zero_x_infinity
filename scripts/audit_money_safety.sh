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

# =============================================================================
# Summary
# =============================================================================

echo "====================="
echo -e "${GREEN}✅ Money Safety Audit PASSED${NC}"
echo ""
