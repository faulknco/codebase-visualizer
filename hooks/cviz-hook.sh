#!/bin/bash
# cviz agent activity hook for Claude Code
case "$CLAUDE_TOOL_NAME" in
  Read|Edit|Write) ;;
  *) exit 0 ;;
esac

FILE_PATH=$(echo "$CLAUDE_TOOL_INPUT" | jq -r '.file_path // empty' 2>/dev/null)
[ -z "$FILE_PATH" ] && exit 0

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null)
[ -z "$REPO_ROOT" ] && exit 0
REL_PATH="${FILE_PATH#$REPO_ROOT/}"

# Find any active cviz socket
SOCK=$(ls /tmp/cviz-*.sock 2>/dev/null | head -1)
[ -z "$SOCK" ] || [ ! -S "$SOCK" ] && exit 0

ACTION=$(echo "$CLAUDE_TOOL_NAME" | tr '[:upper:]' '[:lower:]')

printf '{"file":"%s","action":"%s","agent":"claude","timestamp":%s}\n' \
  "$REL_PATH" "$ACTION" "$(date +%s)" | nc -U "$SOCK" &
disown 2>/dev/null
exit 0
