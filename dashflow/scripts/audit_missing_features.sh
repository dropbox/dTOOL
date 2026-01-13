#!/bin/bash
set -euo pipefail
# Comprehensive audit script to find missing features
# Compares Python baseline to Rust implementation
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

PYTHON_COMMUNITY="$HOME/dashflow_community/dashflow_community"
RUST_CRATES="$HOME/dashflow/crates"
OUTPUT_FILE="$HOME/dashflow/MISSING_FEATURES_AUDIT.md"

echo "# COMPREHENSIVE MISSING FEATURES AUDIT" > "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"
echo "Generated: $(date)" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

# Function to count features
count_python_features() {
    local category=$1
    find "$PYTHON_COMMUNITY/$category" -name "*.py" -type f 2>/dev/null | grep -v __pycache__ | grep -v __init__.py | wc -l | tr -d ' '
}

count_rust_features() {
    local category=$1
    # Search for .rs files in relevant crates
    find "$RUST_CRATES" -name "*.rs" -type f 2>/dev/null | grep -i "$category" | grep -v test | wc -l | tr -d ' '
}

# Categories to audit
categories=(
    "chains"
    "agents"
    "tools"
    "memory"
    "retrievers"
    "document_loaders"
    "document_transformers"
    "vectorstores"
    "embeddings"
    "chat_models"
    "llms"
    "output_parsers"
    "callbacks"
    "chat_loaders"
    "graph_vectorstores"
    "utilities"
    "agent_toolkits"
    "docstore"
    "document_compressors"
    "example_selectors"
    "graphs"
    "indexes"
    "storage"
    "query_constructors"
)

echo "## Summary by Category" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"
echo "| Category | Python Files | Rust Files | Status |" >> "$OUTPUT_FILE"
echo "|----------|--------------|------------|--------|" >> "$OUTPUT_FILE"

total_python=0
total_rust=0

for category in "${categories[@]}"; do
    py_count=$(count_python_features "$category")
    rust_count=$(count_rust_features "$category")

    total_python=$((total_python + py_count))
    total_rust=$((total_rust + rust_count))

    if [ "$py_count" -gt 0 ]; then
        if [ "$rust_count" -eq 0 ]; then
            status="MISSING"
        elif [ "$rust_count" -lt "$py_count" ]; then
            status="PARTIAL"
        else
            status="IMPLEMENTED"
        fi
        echo "| $category | $py_count | $rust_count | $status |" >> "$OUTPUT_FILE"
    fi
done

echo "" >> "$OUTPUT_FILE"
echo "**Total Python Features:** $total_python" >> "$OUTPUT_FILE"
echo "**Total Rust Files Found:** $total_rust" >> "$OUTPUT_FILE"
echo "**Estimated Missing:** $((total_python - total_rust))" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

# Now do detailed analysis per category
for category in "${categories[@]}"; do
    py_count=$(count_python_features "$category")
    if [ "$py_count" -gt 0 ]; then
        echo "## Category: $category" >> "$OUTPUT_FILE"
        echo "" >> "$OUTPUT_FILE"
        echo "Python files found: $py_count" >> "$OUTPUT_FILE"
        echo "" >> "$OUTPUT_FILE"

        # List all Python files
        find "$PYTHON_COMMUNITY/$category" -name "*.py" -type f 2>/dev/null | grep -v __pycache__ | grep -v __init__.py | while read pyfile; do
            basename=$(basename "$pyfile" .py)
            dirname=$(dirname "$pyfile" | sed "s|$PYTHON_COMMUNITY/$category||" | sed 's|^/||')

            # Check if exists in Rust
            if [ -n "$dirname" ]; then
                search_term="${dirname}/${basename}"
            else
                search_term="$basename"
            fi

            # Search for corresponding Rust implementation
            rust_found=$(find "$RUST_CRATES" -name "*.rs" -o -name "Cargo.toml" | xargs grep -l "$basename" 2>/dev/null | head -1)

            if [ -z "$rust_found" ]; then
                echo "- [ ] **$search_term** (Python: $pyfile) - NOT FOUND in Rust" >> "$OUTPUT_FILE"
            else
                echo "- [x] **$search_term** (Python: $pyfile) - Found: $rust_found" >> "$OUTPUT_FILE"
            fi
        done

        echo "" >> "$OUTPUT_FILE"
    fi
done

echo "" >> "$OUTPUT_FILE"
echo "## Analysis Complete" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"
echo "This audit compared Python DashFlow Community package against Rust implementation." >> "$OUTPUT_FILE"
echo "Next steps: Review missing features and prioritize implementation." >> "$OUTPUT_FILE"

echo "Audit complete. Results saved to: $OUTPUT_FILE"
