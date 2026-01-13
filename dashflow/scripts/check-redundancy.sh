#!/usr/bin/env bash
# check-redundancy.sh - Detect when code reimplements DashFlow functionality
#
# Usage: ./scripts/check-redundancy.sh [files...]
#
# This script checks for patterns that suggest reimplementation of
# functionality that DashFlow already provides.
#
# Exit codes:
#   0 - No redundancies found
#   1 - Potential redundancies detected (warnings)
#
# Install as pre-commit hook:
#   cp scripts/check-redundancy.sh .git/hooks/pre-commit
#   chmod +x .git/hooks/pre-commit

set -euo pipefail

# Colors for output
RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

WARNINGS=0

warn() {
    echo -e "${YELLOW}WARNING:${NC} $1"
    WARNINGS=$((WARNINGS + 1))
}

info() {
    echo -e "${GREEN}INFO:${NC} $1"
}

# =============================================================================
# Helper: Check if a file uses DashFlow wrappers (legitimate usage)
# =============================================================================

# Check if file imports any DashFlow LLM crate
uses_dashflow_llm() {
    local file="$1"
    grep -qE "(dashflow_openai|dashflow_anthropic|dashflow_ollama|dashflow_bedrock|dashflow_gemini)" "$file" 2>/dev/null
}

# Check if file imports any DashFlow checkpointer crate
uses_dashflow_checkpointer() {
    local file="$1"
    grep -qE "dashflow_(postgres|redis|s3|dynamodb)_checkpointer" "$file" 2>/dev/null
}

# Check if file imports any DashFlow vector store crate
uses_dashflow_vectorstore() {
    local file="$1"
    grep -qE "(dashflow_qdrant|dashflow_pinecone|dashflow_pgvector|dashflow_chroma|dashflow_weaviate)" "$file" 2>/dev/null
}

# Check if file imports any DashFlow tool crate
uses_dashflow_tool() {
    local file="$1"
    grep -qE "(dashflow_shell_tool|dashflow_file_tool|dashflow_http_requests)" "$file" 2>/dev/null
}

# Get files to check (from args or git staged files)
if [ $# -gt 0 ]; then
    FILES="$@"
else
    FILES=$(git diff --cached --name-only --diff-filter=ACM 2>/dev/null | grep -E '\.(rs|toml)$' || true)
fi

if [ -z "$FILES" ]; then
    exit 0
fi

echo "Checking for DashFlow redundancies..."
echo ""

# =============================================================================
# Check 1: Direct use of underlying libraries when DashFlow wrappers exist
# =============================================================================

# Check for underlying crates that have DashFlow wrappers
check_wrapper() {
    local file="$1"
    local underlying="$2"
    local wrapper="$3"

    if grep -q "^${underlying}[[:space:]]*=" "$file" 2>/dev/null || \
       grep -q "\"${underlying}\"" "$file" 2>/dev/null; then
        warn "$file uses '$underlying' directly. Consider using '$wrapper' instead."
        echo "      DashFlow wrapper provides: retry logic, error handling, integration with StateGraph"
        echo ""
    fi
}

for file in $FILES; do
    if [[ "$file" == *.toml ]]; then
        check_wrapper "$file" "async-openai" "dashflow-openai"
        check_wrapper "$file" "anthropic" "dashflow-anthropic"
        check_wrapper "$file" "ollama-rs" "dashflow-ollama"
        check_wrapper "$file" "qdrant-client" "dashflow-qdrant"
        check_wrapper "$file" "pinecone" "dashflow-pinecone"
        check_wrapper "$file" "redis" "dashflow-redis or dashflow-redis-checkpointer"
        check_wrapper "$file" "tokio-postgres" "dashflow-postgres-checkpointer"
    fi
done

# =============================================================================
# Check 2: Common reimplementation patterns in Rust code
# =============================================================================

check_pattern() {
    local file="$1"
    local pattern="$2"
    local msg="$3"
    local category="$4"  # llm, checkpointer, vectorstore, tool, or empty

    if grep -qE "$pattern" "$file" 2>/dev/null; then
        # Skip if file legitimately uses DashFlow wrapper for this category
        case "$category" in
            llm)
                if uses_dashflow_llm "$file"; then
                    return 0  # Legitimate wrapper usage
                fi
                ;;
            checkpointer)
                if uses_dashflow_checkpointer "$file"; then
                    return 0
                fi
                ;;
            vectorstore)
                if uses_dashflow_vectorstore "$file"; then
                    return 0
                fi
                ;;
            tool)
                if uses_dashflow_tool "$file"; then
                    return 0
                fi
                ;;
        esac

        warn "$file matches pattern indicating possible reimplementation"
        echo "      Pattern: $pattern"
        echo "      Suggestion: $msg"
        echo "      See: AI_AGENT_GUIDE.md (in repo root)"
        echo ""
    fi
}

for file in $FILES; do
    if [[ "$file" == *.rs ]]; then
        check_pattern "$file" "impl.*Tool.*for" "Check if dashflow-*-tool crates already provide this tool" "tool"
        check_pattern "$file" "ChatCompletion|CreateChatCompletion" "Check dashflow-openai ChatOpenAI" "llm"
        check_pattern "$file" "struct.*LlmClient|struct.*ChatClient" "Check dashflow-openai, dashflow-anthropic, etc." "llm"
        check_pattern "$file" "impl.*Checkpointer" "Check dashflow-*-checkpointer crates" "checkpointer"
        check_pattern "$file" "similarity_search|vector.*search" "Check dashflow-qdrant, dashflow-pgvector, etc." "vectorstore"
    fi
done

# =============================================================================
# Check 3: New files that might duplicate DashFlow functionality
# =============================================================================

check_filename() {
    local file="$1"
    local name="$2"
    local msg="$3"
    local category="$4"  # llm, checkpointer, vectorstore, tool, or empty
    local basename
    basename=$(basename "$file")

    if [[ "$basename" == "$name" ]]; then
        # Skip if file legitimately uses DashFlow wrapper for this category
        case "$category" in
            llm)
                if uses_dashflow_llm "$file"; then
                    return 0  # Legitimate wrapper usage
                fi
                ;;
            checkpointer)
                if uses_dashflow_checkpointer "$file"; then
                    return 0
                fi
                ;;
            vectorstore)
                if uses_dashflow_vectorstore "$file"; then
                    return 0
                fi
                ;;
            tool)
                if uses_dashflow_tool "$file"; then
                    return 0
                fi
                ;;
        esac

        warn "File '$file' might duplicate DashFlow functionality"
        echo "      Suggestion: $msg"
        echo "      See: AI_AGENT_GUIDE.md (in repo root)"
        echo ""
    fi
}

for file in $FILES; do
    check_filename "$file" "llm.rs" "LLM client - check dashflow-openai" "llm"
    check_filename "$file" "openai.rs" "OpenAI integration - check dashflow-openai" "llm"
    check_filename "$file" "anthropic.rs" "Anthropic integration - check dashflow-anthropic" "llm"
    check_filename "$file" "shell.rs" "Shell tool - check dashflow-shell-tool" "tool"
    check_filename "$file" "file_tool.rs" "File tool - check dashflow-file-tool" "tool"
    check_filename "$file" "checkpointer.rs" "Checkpointing - check dashflow-*-checkpointer" "checkpointer"
    check_filename "$file" "vector_store.rs" "Vector store - check dashflow-qdrant, dashflow-pgvector" "vectorstore"
    check_filename "$file" "embeddings.rs" "Embeddings - check dashflow-openai OpenAIEmbeddings" "llm"
done

# =============================================================================
# Summary
# =============================================================================

echo ""
if [ $WARNINGS -gt 0 ]; then
    echo -e "${YELLOW}Found $WARNINGS potential redundancy warning(s)${NC}"
    echo ""
    echo "Before implementing custom solutions, check:"
    echo "  1. AI_AGENT_GUIDE.md (in repo root) - lists all available crates"
    echo "  2. crates/ directory - browse available integrations"
    echo ""
    echo "If DashFlow doesn't have what you need:"
    echo "  1. Consider adding it to DashFlow as a platform improvement"
    echo "  2. Create a feature branch in this repository"
    echo "  3. Use [PLATFORM] prefix for commits"
    echo ""
    exit 1
else
    echo -e "${GREEN}No redundancy warnings found${NC}"
    exit 0
fi
