#!/bin/bash
# check_regression_skip.sh - Check if the last successful full regression was on the same commit

set -e

# Configuration
WORKFLOW_NAME="Full Regression"
GITHUB_TOKEN="${GITHUB_TOKEN:-}"

if [ -z "$GITHUB_TOKEN" ]; then
    echo "Error: GITHUB_TOKEN is not set."
    exit 2
fi

# Get current commit SHA
CURRENT_SHA=$(git rev-parse HEAD)

# Get the last successful run's head SHA for this workflow
# Use gh CLI (installed in GitHub Actions runners)
echo "Checking last successful run for workflow: $WORKFLOW_NAME"
LAST_SUCCESS_SHA=$(gh run list --workflow "$WORKFLOW_NAME" --status success --limit 1 --json headSha --jq '.[0].headSha' 2>/dev/null || echo "")

if [ -z "$LAST_SUCCESS_SHA" ]; then
    echo "No previous successful runs found. PROCEED."
    echo "should_skip=false"
    exit 0
fi

echo "Current SHA: $CURRENT_SHA"
echo "Last Success SHA: $LAST_SUCCESS_SHA"

if [ "$CURRENT_SHA" == "$LAST_SUCCESS_SHA" ]; then
    echo "Commit has not changed since last success. SKIP."
    echo "should_skip=true"
else
    echo "Commit is new. PROCEED."
    echo "should_skip=false"
fi
