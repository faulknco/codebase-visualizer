#!/bin/bash
# cviz agent activity hook for Claude Code
# Reads tool event from stdin (JSON), sends file activity to cviz socket

# Read hook input from stdin
INPUT=$(cat)

# Extract tool name and file path
TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // empty' 2>/dev/null)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty' 2>/dev/null)

# Only process Read/Edit/Write with a file path
case "$TOOL_NAME" in
  Read|Edit|Write) ;;
  *) exit 0 ;;
esac
[ -z "$FILE_PATH" ] && exit 0

# Get repo root to make path relative
REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null)
[ -z "$REPO_ROOT" ] && exit 0
REL_PATH="${FILE_PATH#$REPO_ROOT/}"

# Find any active cviz socket
SOCK=$(ls /tmp/cviz-*.sock 2>/dev/null | head -1)
[ -z "$SOCK" ] || [ ! -S "$SOCK" ] && exit 0

ACTION=$(echo "$TOOL_NAME" | tr '[:upper:]' '[:lower:]')

printf '{"file":"%s","action":"%s","agent":"claude","timestamp":%s}\n' \
  "$REL_PATH" "$ACTION" "$(date +%s)" | nc -U "$SOCK" &
disown 2>/dev/null
exit 0
