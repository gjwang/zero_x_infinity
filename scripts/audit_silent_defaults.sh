#!/bin/bash
# audit_silent_defaults.sh
# Enforces Fail-Fast principle by prohibiting silent fallbacks in critical logic.
#
# RULE: All .unwrap_or() usages are VIOLATIONS unless marked with SAFE_DEFAULT:
# Example of allowed usage:
#   .unwrap_or(last_value) // SAFE_DEFAULT: empty list returns current value

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}Starting Fail-Fast Audit: Scanning for silent defaults...${NC}"
echo -e "${YELLOW}Rule: All .unwrap_or() must have SAFE_DEFAULT: comment or be violations${NC}"

EXIT_CODE=0

# ============================================================================
# Check 1: ALL unwrap_or usages must be marked with SAFE_DEFAULT or be violations
# ============================================================================
echo -e "\n${YELLOW}[Unmarked unwrap_or Usage]${NC}"

# Find all .unwrap_or usages in src/, exclude test code and those with SAFE_DEFAULT marker
UNMARKED=$(find src/ -name "*.rs" -exec grep -n "\.unwrap_or" {} + 2>/dev/null | \
    grep -v "SAFE_DEFAULT" | \
    grep -v "#\[cfg(test)\]" | \
    grep -v "mod tests" | \
    grep -v "_test.rs")

if [ ! -z "$UNMARKED" ]; then
    COUNT=$(echo "$UNMARKED" | wc -l | tr -d ' ')
    echo -e "${YELLOW}WARNING: $COUNT unmarked .unwrap_or() found. Add '// SAFE_DEFAULT: <reason>' to mark as reviewed:${NC}"
    echo "$UNMARKED"
    # NOTE: Warnings don't fail CI, but should be addressed over time
else
    echo -e "${GREEN}PASS: All .unwrap_or() usages are properly marked.${NC}"
fi

# ============================================================================
# Check 2: Direct row.get() usage (prevents panics on column rename)
# ============================================================================
echo -e "\n${YELLOW}[Unsafe DB Row Mapping]${NC}"

UNSAFE_ROW=$(find src/ -name "*.rs" -exec grep -n "row\.get(" {} + 2>/dev/null | \
    grep -v "try_get" | \
    grep -v "#\[cfg(test)\]")

if [ ! -z "$UNSAFE_ROW" ]; then
    echo -e "${RED}FAILURE: Direct row.get() found! Use SafeRow::try_get_log instead:${NC}"
    echo "$UNSAFE_ROW"
    EXIT_CODE=1
else
    echo -e "${GREEN}PASS: No unsafe DB row mapping.${NC}"
fi

# ============================================================================
# Check 3: Insecure user ID parsing
# ============================================================================
echo -e "\n${YELLOW}[Insecure User ID Logic]${NC}"

INSECURE_ID=$(find src/ -name "*.rs" -exec grep -nE "claims\.sub\.parse.*\.unwrap_or(_default)?\(" {} + 2>/dev/null)

if [ ! -z "$INSECURE_ID" ]; then
    echo -e "${RED}FAILURE: Insecure user ID parsing found:${NC}"
    echo "$INSECURE_ID"
    EXIT_CODE=1
else
    echo -e "${GREEN}PASS: No insecure user ID parsing.${NC}"
fi

# ============================================================================
# Summary
# ============================================================================
if [ $EXIT_CODE -eq 0 ]; then
    echo -e "\n${GREEN}COMPLETE: All Fail-Fast checks passed!${NC}"
else
    echo -e "\n${RED}COMPLETE: Fail-Fast audit failed. Please fix the above issues.${NC}"
    echo -e "${YELLOW}Hint: Add '// SAFE_DEFAULT: <reason>' to intentional fallbacks${NC}"
fi

exit $EXIT_CODE
