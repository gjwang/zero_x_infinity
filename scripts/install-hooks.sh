#!/bin/bash
# Install git hooks from scripts/git-hooks to .git/hooks

HOOK_DIR=".git/hooks"
SCRIPT_DIR="$(dirname "$0")/git-hooks"

for hook in "$SCRIPT_DIR"/*; do
    hook_name=$(basename "$hook")
    target="$HOOK_DIR/$hook_name"
    
    cp "$hook" "$target"
    chmod +x "$target"
    echo "✅ Installed: $hook_name"
done

echo ""
echo "Done! Git hooks installed."
