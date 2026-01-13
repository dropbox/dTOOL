#!/bin/bash
set -euo pipefail  # Strict mode for initial setup
# Test Infrastructure Validation Script
# Checks what API keys and services are available for running tests
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

set +eu  # Don't exit on errors or unset vars - we're checking what's available

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Counters
TOTAL_CHECKS=0
AVAILABLE=0
MISSING=0

echo "======================================"
echo "Test Infrastructure Validation"
echo "======================================"
echo ""

# Load .env if it exists
if [ -f .env ]; then
    echo "✓ Found .env file"
    export $(grep -v '^#' .env | xargs)
else
    echo "⚠ No .env file found (copy .env.test.template to .env)"
fi
echo ""

# Function to check API key
check_api_key() {
    local key_name=$1
    local key_value=${!key_name}
    local description=$2

    TOTAL_CHECKS=$((TOTAL_CHECKS + 1))

    if [ -n "$key_value" ] && [ "$key_value" != "..." ]; then
        echo -e "${GREEN}✓${NC} $key_name - $description"
        AVAILABLE=$((AVAILABLE + 1))
        return 0
    else
        echo -e "${RED}✗${NC} $key_name - $description"
        MISSING=$((MISSING + 1))
        return 1
    fi
}

# Function to check docker service
check_docker_service() {
    local service_name=$1
    local port=$2
    local description=$3

    TOTAL_CHECKS=$((TOTAL_CHECKS + 1))

    if nc -z localhost $port 2>/dev/null; then
        echo -e "${GREEN}✓${NC} $service_name (localhost:$port) - $description"
        AVAILABLE=$((AVAILABLE + 1))
        return 0
    else
        echo -e "${RED}✗${NC} $service_name (localhost:$port) - $description"
        MISSING=$((MISSING + 1))
        return 1
    fi
}

echo "=== API Keys (Required for LLM/Embedding Tests) ==="
echo ""

check_api_key "OPENAI_API_KEY" "OpenAI (80 tests)"
check_api_key "ANTHROPIC_API_KEY" "Anthropic Claude (80 tests)"
check_api_key "GROQ_API_KEY" "Groq (40 tests)"
check_api_key "MISTRAL_API_KEY" "Mistral AI (43 tests: 40 chat + 3 embeddings)"
check_api_key "COHERE_API_KEY" "Cohere (40 tests)"
check_api_key "FIREWORKS_API_KEY" "Fireworks AI (4 embedding tests)"
check_api_key "NOMIC_API_KEY" "Nomic Embeddings (4 tests)"
check_api_key "DEEPSEEK_API_KEY" "DeepSeek (40 tests)"
check_api_key "XAI_API_KEY" "xAI Grok (40 tests)"
check_api_key "PERPLEXITY_API_KEY" "Perplexity AI (40 tests)"

echo ""
echo "=== Docker Services (Required for Vector Store Tests) ==="
echo ""

# Check if docker is installed
if command -v docker &> /dev/null; then
    echo -e "${GREEN}✓${NC} Docker installed"
    DOCKER_AVAILABLE=true
else
    echo -e "${RED}✗${NC} Docker not installed"
    DOCKER_AVAILABLE=false
fi
echo ""

if [ "$DOCKER_AVAILABLE" = true ]; then
    check_docker_service "Chroma" 8000 "Chroma vector store (36 tests)"
    check_docker_service "Qdrant" 6333 "Qdrant vector store (63 tests)"
    check_docker_service "Weaviate" 8080 "Weaviate vector store (33 tests)"
    check_docker_service "Elasticsearch" 9200 "Elasticsearch (33 tests)"
    check_docker_service "PostgreSQL/PGVector" 5432 "PGVector (33 tests)"
    check_docker_service "MongoDB" 27017 "MongoDB Atlas Vector Search (30 tests)"
    check_docker_service "Neo4j" 7687 "Neo4j graph database (30 tests)"
    check_docker_service "Cassandra" 9042 "Cassandra (30 tests)"
    check_docker_service "OpenSearch" 9200 "OpenSearch (33 tests)"  # Port 9200 is HTTP API, 9600 is Performance Analyzer
    check_docker_service "Redis" 6379 "Redis (for chat history tests)"
    check_docker_service "Ollama" 11434 "Ollama local LLM (51 tests)"
else
    echo "Skipping docker service checks (docker not available)"
    MISSING=$((MISSING + 11))
    TOTAL_CHECKS=$((TOTAL_CHECKS + 11))
fi

echo ""
echo "=== Cloud Vector Stores (Alternative to Docker) ==="
echo ""

check_api_key "PINECONE_API_KEY" "Pinecone (33 tests, free tier)"

echo ""
echo "=== Optional API Keys ==="
echo ""

check_api_key "GITHUB_TOKEN" "GitHub tool (11 tests)"
check_api_key "GITLAB_TOKEN" "GitLab tool"
check_api_key "HUGGINGFACE_API_KEY" "HuggingFace (39 tests, but API unreliable - see README)"

echo ""
echo "======================================"
echo "Summary"
echo "======================================"
echo ""
echo "Total checks: $TOTAL_CHECKS"
echo -e "${GREEN}Available: $AVAILABLE${NC}"
echo -e "${RED}Missing: $MISSING${NC}"
echo ""

# Calculate percentage
PERCENTAGE=$((AVAILABLE * 100 / TOTAL_CHECKS))
echo "Infrastructure readiness: $PERCENTAGE%"
echo ""

# Estimate test coverage
if [ $PERCENTAGE -ge 80 ]; then
    echo -e "${GREEN}✓ Excellent! You can run most tests (~85-90%)${NC}"
elif [ $PERCENTAGE -ge 50 ]; then
    echo -e "${YELLOW}⚠ Good! You can run many tests (~50-70%)${NC}"
    echo "  Consider adding more API keys or starting docker services"
else
    echo -e "${RED}⚠ Limited! You can run basic tests only (~30-50%)${NC}"
    echo "  Add API keys and start docker services for full coverage"
fi

echo ""
echo "To start docker services:"
echo "  docker-compose -f docker-compose.test.yml up -d"
echo ""
echo "To get API keys, see links in .env.test.template"
echo ""
