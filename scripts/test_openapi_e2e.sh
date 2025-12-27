#!/bin/bash
set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export GATEWAY_URL="http://localhost:8080"

# Use uv run if available, otherwise fallback (though CI should have uv)
if command -v uv >/dev/null 2>&1; then
    uv run python3 "$SCRIPT_DIR/test_openapi_e2e.py" "$@"
else
    python3 "$SCRIPT_DIR/test_openapi_e2e.py" "$@"
fi
