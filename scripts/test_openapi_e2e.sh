#!/bin/bash
set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export GATEWAY_URL="http://localhost:8080"
export PYTHONPATH="$SCRIPT_DIR:$PYTHONPATH"

# Clean any pycache to force re-import
rm -rf "$SCRIPT_DIR/lib/__pycache__"

# Use uv to explicitly manage environment (ensuring pynacl/requests are available)
if command -v uv >/dev/null 2>&1; then
    uv run --with requests --with pynacl python3 - "$@" << 'EOF'
import sys
import os

# Add script directory to path to find lib.api_auth
script_dir = os.environ.get("SCRIPT_DIR")
if script_dir:
    sys.path.insert(0, script_dir)
    sys.path.insert(0, os.path.join(script_dir, "lib"))

# Import actual test script as module or execute its logic
# Since test_openapi_e2e.py is designed to be run as __main__, we can execute it via runpy
import runpy
import sys

# Pass arguments
sys.argv = ["test_openapi_e2e.py"] + sys.argv[1:]

try:
    # Execute the file at the computed path
    runpy.run_path(os.path.join(script_dir, "test_openapi_e2e.py"), run_name="__main__")
except SystemExit as e:
    sys.exit(e.code)
except Exception as e:
    print(f"Error running test script: {e}")
    sys.exit(1)
EOF
else
    # Fallback to system python
    python3 "$SCRIPT_DIR/test_openapi_e2e.py" "$@"
fi
