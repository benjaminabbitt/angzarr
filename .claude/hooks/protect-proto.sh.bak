#!/bin/bash
# Block modifications to proto files without explicit human approval.
# This hook runs on PreToolUse for Edit|Write operations.
# It blocks even when bypass permissions are enabled.

set -euo pipefail

INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // .tool_input.content // empty')

# Check if the file is a proto file or in the proto directory
if [[ "$FILE_PATH" == *.proto ]] || [[ "$FILE_PATH" == */proto/* ]]; then
    echo "Proto file modification blocked: $FILE_PATH" >&2
    echo "Proto files require explicit human review. Please approve this change manually." >&2
    exit 2
fi

exit 0
