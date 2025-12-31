#!/bin/bash
# audit_silent_defaults.sh
# Enforces Fail-Fast principle by prohibiting silent fallbacks in critical logic.

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}Starting Fail-Fast Audit: Scanning for silent defaults...${NC}"

# Define high-risk patterns
# 1. .unwrap_or(0) or .unwrap_or_default() in financial/config logic
# 2. .unwrap_or(2) or .unwrap_or(8) for scales/precisions
# 3. .unwrap_or("?") or .unwrap_or_else(|| format!(...)) for critical names

EXIT_CODE=0

# High-risk directories (limiting to .rs files to avoid false positives in .md or logs)
TARGET_DIRS="src/ market/ sentinel/ websocket/ funding/ exchange_info/ gateway/"

function check_pattern() {
    local label=$1
    local pattern=$2
    local paths=$3
    echo -e "\n${YELLOW}[$label]${NC}"
    # Use find to get only .rs files
    local results=$(find $paths -name "*.rs" -exec grep -rnE "$pattern" {} + 2>/dev/null)
    if [ ! -z "$results" ]; then
        echo -e "${RED}FAILURE: $label issues found!${NC}"
        echo "$results"
        return 1
    else
        echo -e "${GREEN}PASS: $label audit clear.${NC}"
        return 0
    fi
}

# 1. Check for hardcoded precision/scale defaults
check_pattern "Hardcoded Precision" "\.unwrap_or\(([2468])\)" "$TARGET_DIRS" || EXIT_CODE=1

# 2. Check for balance/cost defaults in core logic
SENSITIVE_CORE="src/pipeline_services.rs src/funding/ src/ubscore.rs src/internal_transfer/"
check_pattern "Silent Financial Defaults" "\.unwrap_or(_default)?\(0?\)" "$SENSITIVE_CORE" || EXIT_CODE=1

# 3. Check for security-critical ID defaults
check_pattern "Insecure User ID Logic" "claims\.sub\.parse.*\.unwrap_or(_default)?\(" "src/" || EXIT_CODE=1

# 4. Check for critical metadata defaults in WebSocket broadcast
check_pattern "Metadata Display Defaults" "\.unwrap_or(_else)?\(.*format!.*SYMBOL.*\)" "src/websocket/" || EXIT_CODE=1

if [ $EXIT_CODE -eq 0 ]; then
    echo -e "\n${GREEN}COMPLETE: All Fail-Fast checks passed!${NC}"
else
    echo -e "\n${RED}COMPLETE: Fail-Fast audit failed. Please fix the above issues.${NC}"
fi

exit $EXIT_CODE
