# Development Scripts

This directory contains development helper scripts for the DashFlow Rust project.

## Python Scripts (`python/`)

**IMPORTANT:** `json_to_text.py` is PROTECTED and must not be deleted. It converts Claude JSON stream output to readable text and is essential for the Claude Code development workflow.

### Parity Testing
- `test_agent_execution_parity.py` - Compare agent execution between Rust and Python
- `test_chroma_crud_parity.py` - Test Chroma vector store CRUD parity
- `test_openai_parity.py` - Test OpenAI API parity
- `test_python_parity.py` - General Python/Rust parity tests
- `test_qdrant_crud_parity.py` - Test Qdrant vector store CRUD parity
- `test_rag_chain_parity.py` - Test RAG chain parity
- `test_text_splitter_parity.py` - Test text splitter parity
- `test_tool_calling_parity.py` - Test tool calling parity

### Analysis & Utilities
- `analyze_mocks.py` - Analyze mock usage in tests
- `json_to_text.py` - **PROTECTED** - Convert Claude JSON to text (DO NOT DELETE)
- `benchmark_results_python.json` - Python benchmark baseline data
- `pyproject.toml` - Python project configuration

## Shell Scripts

### Auditing & Analysis
- `audit_missing_features.sh` - Comprehensive audit to find missing features vs Python baseline
- `check_mutation_progress.sh` - Check mutation testing progress
- `check_test_infrastructure.sh` - Validate test infrastructure
- `update_chatmodel_signatures.sh` - Update ChatModel trait implementations

### Performance & Testing
- `performance-baseline.sh` - Run performance baseline tests
- `run-load-test.sh` - Execute load tests
- `update-readme-stats.sh` - Update README with current statistics

### Python Utilities (from `scripts/`)
- `count_clones.py` - Count clone usage
- `count_test_assertions.py` - Count test assertions
- `python_benchmarks.py` - Python benchmarking scripts
- `validate_text_splitters.py` - Validate text splitter implementations

### Security
- `protect_critical_files.sh` - Git hook to protect critical files (json_to_text.py)

## Notes

- All Python scripts are **development helpers only** - not used in runtime/production
- The final Rust binary has zero Python dependencies
- Python scripts are used for validation, testing, and development tooling
- Shell scripts automate development tasks and CI/CD workflows
