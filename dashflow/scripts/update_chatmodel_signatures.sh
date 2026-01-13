#!/bin/bash
# Script to update all ChatModel trait implementations and call sites
# for the new signature with tools/tool_choice parameters
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

set -euo pipefail

echo "Updating ChatModel trait implementations..."

# Find all files with ChatModel implementations (excluding dashflow which is done)
for file in $(find crates -name "*.rs" -type f | grep -v "dashflow/src/core/language_models.rs"); do
    if grep -q "async fn _generate" "$file" && grep -q "impl.*ChatModel" "$file"; then
        echo "Updating trait impl in: $file"
        # Update _generate signature
        sed -i '' 's/async fn _generate(\n        \&self,\n        messages: \&\[BaseMessage\],\n        _stop: Option<\&\[String\]>,\n        run_manager: Option<\&CallbackManager>,\n    ) -> Result<ChatResult>/async fn _generate(\n        \&self,\n        messages: \&[BaseMessage],\n        _stop: Option<\&[String]>,\n        _tools: Option<\&[ToolDefinition]>,\n        _tool_choice: Option<\&[ToolChoice]>,\n        run_manager: Option<\&CallbackManager>,\n    ) -> Result<ChatResult>/g' "$file"
    fi
done

echo "Updating generate() call sites..."

# Update all .generate() calls (3 args -> 5 args)
for file in $(find crates -name "*.rs" -type f); do
    if grep -q "\.generate(&.*None, None).await" "$file"; then
        echo "Updating calls in: $file"
        # This regex replaces .generate(&..., None, None).await with .generate(&..., None, None, None, None).await
        sed -i '' 's/\.generate(\([^)]*\), None, None)\.await/.generate(\1, None, None, None, None).await/g' "$file"
    fi
done

echo "Done!"
