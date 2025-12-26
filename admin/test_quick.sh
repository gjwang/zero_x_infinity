#!/bin/bash
# Quick Unit Test - No server required
# Usage: ./test_quick.sh

cd "$(dirname "$BASH_SOURCE[0]")"

if [ -d "venv" ]; then
    source venv/bin/activate
elif [ -d ".venv" ]; then
    source .venv/bin/activate
fi

echo "🧪 Running Quick Unit Tests..."
pytest tests/ -m "not e2e" --ignore=tests/e2e -q --tb=short

echo ""
echo "📝 Status API Tests..."
pytest tests/test_ux08_status_strings.py -v --tb=short
