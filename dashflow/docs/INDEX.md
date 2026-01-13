# DashFlow Documentation Index

**Last Updated:** 2026-01-04 (Worker #2431 - Fix stale AI_PARTS_CATALOG.md file size (554KB→324KB))

This is the single source of truth for DashFlow documentation. Each topic lives in ONE place; other documents link here to avoid duplication.

---

## Quick Links

| Need | Document |
|------|----------|
| First-time setup | [GOLDEN_PATH.md](GOLDEN_PATH.md) |
| API reference | [CLI_REFERENCE.md](CLI_REFERENCE.md) |
| Common recipes | [COOKBOOK.md](COOKBOOK.md) |
| Architecture overview | [ARCHITECTURE.md](ARCHITECTURE.md) |
| Troubleshooting | [TROUBLESHOOTING.md](TROUBLESHOOTING.md) |

---

## Getting Started

| Document | Purpose |
|----------|---------|
| [GOLDEN_PATH.md](GOLDEN_PATH.md) | Step-by-step guide for new users |
| [CONFIGURATION.md](CONFIGURATION.md) | Environment variables, config files, feature flags |
| [EXAMPLE_APPS.md](EXAMPLE_APPS.md) | 2 working example applications + shared utilities library |
| [APP_ARCHITECTURE_GUIDE.md](APP_ARCHITECTURE_GUIDE.md) | How to structure DashFlow applications |

---

## Core Concepts

| Document | Purpose |
|----------|---------|
| [ARCHITECTURE.md](ARCHITECTURE.md) | System architecture, data flows |
| [CRATE_ARCHITECTURE.md](CRATE_ARCHITECTURE.md) | 108 crates: categories, sizes, dependencies |
| [DASHSTREAM_PROTOCOL.md](DASHSTREAM_PROTOCOL.md) | Streaming protocol design |
| [SERIALIZATION_DESIGN.md](SERIALIZATION_DESIGN.md) | State serialization patterns |
| [work_stealing_scheduler_design.md](work_stealing_scheduler_design.md) | Scheduler implementation |

---

## CLI & API

| Document | Purpose |
|----------|---------|
| [CLI_REFERENCE.md](CLI_REFERENCE.md) | All CLI commands with examples |
| [API_OVERVIEW.md](API_OVERVIEW.md) | High-level API summary |
| [API_STABILITY.md](API_STABILITY.md) | API stability guarantees |

---

## Development Guides

| Document | Purpose |
|----------|---------|
| [COOKBOOK.md](COOKBOOK.md) | Common recipes and patterns |
| [BEST_PRACTICES.md](BEST_PRACTICES.md) | Recommended practices |
| [ADVANCED_AGENT_PATTERNS.md](ADVANCED_AGENT_PATTERNS.md) | Advanced multi-agent patterns |
| [DASHOPTIMIZE_GUIDE.md](DASHOPTIMIZE_GUIDE.md) | Prompt optimization guide |
| [DEVELOPER_EXPERIENCE.md](DEVELOPER_EXPERIENCE.md) | DX improvements and tools |

---

## Evaluation & Testing

| Document | Purpose |
|----------|---------|
| [EVALUATION_GUIDE.md](EVALUATION_GUIDE.md) | How to evaluate agents |
| [EVALUATION_TUTORIAL.md](EVALUATION_TUTORIAL.md) | Step-by-step evaluation tutorial |
| [EVALUATION_BEST_PRACTICES.md](EVALUATION_BEST_PRACTICES.md) | Evaluation recommendations |
| [EVALUATION_TROUBLESHOOTING.md](EVALUATION_TROUBLESHOOTING.md) | Common evaluation issues |
| [TESTING.md](TESTING.md) | Testing strategy and patterns |
| [TEST_PHILOSOPHY.md](TEST_PHILOSOPHY.md) | Testing philosophy |
| [TEST_COVERAGE_STRATEGY.md](TEST_COVERAGE_STRATEGY.md) | Coverage goals and approach |
| [INTEGRATION_TESTING.md](INTEGRATION_TESTING.md) | Crate-level integration test conventions (testcontainers, wiremock) |
| [INTEGRATION_TEST_EXECUTION_GUIDE.md](INTEGRATION_TEST_EXECUTION_GUIDE.md) | Running integration tests |

---

## Observability & Monitoring

| Document | Purpose |
|----------|---------|
| [OBSERVABILITY.md](OBSERVABILITY.md) | Overview of observability features |
| [OBSERVABILITY_INFRASTRUCTURE.md](OBSERVABILITY_INFRASTRUCTURE.md) | Prometheus, Grafana, tracing setup |
| [OBSERVABILITY_RUNBOOK.md](OBSERVABILITY_RUNBOOK.md) | Operational procedures |
| [TESTING_OBSERVABILITY.md](TESTING_OBSERVABILITY.md) | Testing observability features |
| [DISTRIBUTED_TRACING.md](DISTRIBUTED_TRACING.md) | Distributed tracing guide |

---

## Production & Deployment

| Document | Purpose |
|----------|---------|
| [PRODUCTION_DEPLOYMENT.md](PRODUCTION_DEPLOYMENT.md) | Production deployment overview |
| [PRODUCTION_DEPLOYMENT_GUIDE.md](PRODUCTION_DEPLOYMENT_GUIDE.md) | Detailed deployment guide |
| [PRODUCTION_RUNBOOK.md](PRODUCTION_RUNBOOK.md) | Operational runbook |
| [QUICK_START_PRODUCTION.md](QUICK_START_PRODUCTION.md) | Fast path to production |

---

## Providers & Integrations

| Document | Purpose |
|----------|---------|
| [EMBEDDING_PROVIDERS_COMPARISON.md](EMBEDDING_PROVIDERS_COMPARISON.md) | Comparing embedding providers |
| [AI_PARTS_CATALOG.md](AI_PARTS_CATALOG.md) | Comprehensive parts catalog (324KB) |
| [DEPENDENCY_MAPPING.md](DEPENDENCY_MAPPING.md) | External dependency tracking |

---

## Error Handling & Troubleshooting

| Document | Purpose |
|----------|---------|
| [ERROR_CATALOG.md](ERROR_CATALOG.md) | **Searchable error catalog** - Quick lookup by error pattern with root causes and resolution guides |
| [TROUBLESHOOTING.md](TROUBLESHOOTING.md) | Common issues and solutions |
| [ERROR_TYPES.md](ERROR_TYPES.md) | All error types and handling |
| [ERROR_MESSAGE_STYLE.md](ERROR_MESSAGE_STYLE.md) | Error message conventions |

---

## Contributing & Style

| Document | Purpose |
|----------|---------|
| [CONTRIBUTING_DOCS.md](CONTRIBUTING_DOCS.md) | Documentation contribution guide |
| [CLI_OUTPUT_POLICY.md](CLI_OUTPUT_POLICY.md) | CLI output conventions |

---

## Architecture Decision Records

| Document | Purpose |
|----------|---------|
| [adr/README.md](adr/README.md) | ADR index and overview |
| [adr/0000-template.md](adr/0000-template.md) | Template for new ADRs |
| [adr/0001-single-telemetry-system.md](adr/0001-single-telemetry-system.md) | All telemetry through ExecutionTrace |
| [adr/0002-streaming-is-optional-transport.md](adr/0002-streaming-is-optional-transport.md) | Local analysis never needs infrastructure |
| [adr/0003-single-introspection-api.md](adr/0003-single-introspection-api.md) | Unified platform discovery |
| [adr/0004-rust-only-implementation.md](adr/0004-rust-only-implementation.md) | No Python runtime dependency |
| [adr/0005-non-exhaustive-public-enums.md](adr/0005-non-exhaustive-public-enums.md) | Semver-safe enum extensions |

---

## Security

| Document | Purpose |
|----------|---------|
| [SECURITY_AUDIT.md](SECURITY_AUDIT.md) | Security audit results |
| [SECURITY_ADVISORIES.md](SECURITY_ADVISORIES.md) | Security advisories |

---

## Performance

| Document | Purpose |
|----------|---------|
| [BENCHMARK_RUNBOOK.md](BENCHMARK_RUNBOOK.md) | Hot path benchmarks and regression thresholds |
| [PERFORMANCE_BASELINE.md](PERFORMANCE_BASELINE.md) | Performance baselines |
| [MEMORY_BENCHMARKS.md](MEMORY_BENCHMARKS.md) | Memory usage benchmarks |
| [OPTIMIZATION_AUDIT.md](OPTIMIZATION_AUDIT.md) | Optimization audit results |
| [FRAMEWORK_STABILITY_IMPROVEMENTS.md](FRAMEWORK_STABILITY_IMPROVEMENTS.md) | Stability improvements |

---

## Release Notes

| Version | Document |
|---------|----------|
| v1.11.0 | [RELEASE_NOTES_v1.11.0.md](RELEASE_NOTES_v1.11.0.md) |
| v1.10.0 | [RELEASE_NOTES_v1.10.0.md](RELEASE_NOTES_v1.10.0.md) |
| v1.9.0 | [RELEASE_NOTES_v1.9.0.md](RELEASE_NOTES_v1.9.0.md) |
| v1.8.0 | [RELEASE_NOTES_v1.8.0.md](RELEASE_NOTES_v1.8.0.md) |
| v1.7.0 | [RELEASE_NOTES_v1.7.0.md](RELEASE_NOTES_v1.7.0.md) |
| v1.6.1 | [RELEASE_NOTES_v1.6.1.md](RELEASE_NOTES_v1.6.1.md) |
| Template | [RELEASE_NOTES_TEMPLATE.md](RELEASE_NOTES_TEMPLATE.md) |

---

## Migration

| Document | Purpose |
|----------|---------|
| [book/src/migration/from-python.md](book/src/migration/from-python.md) | **Migration from Python LangChain** - Complete guide with API mapping, examples, common pitfalls |
| [MIGRATION_v1.0_to_v1.6.md](MIGRATION_v1.0_to_v1.6.md) | Migration guide v1.0 to v1.6 |

---

## Internal / AI Worker References

| Document | Purpose |
|----------|---------|
| [../CLAUDE.md](../CLAUDE.md) | AI worker instructions (in repo root) |
| [../DESIGN_INVARIANTS.md](../DESIGN_INVARIANTS.md) | Architectural invariants |
| [../ROADMAP_CURRENT.md](../ROADMAP_CURRENT.md) | Current roadmap |
| [LANGSTREAM_vs_LANGSMITH.md](LANGSTREAM_vs_LANGSMITH.md) | Framework comparison |
| [PYTHON_PARITY_REPORT.md](PYTHON_PARITY_REPORT.md) | Python parity status |
| [PHASE3_COMPLETION_SUMMARY.md](PHASE3_COMPLETION_SUMMARY.md) | Phase 3 summary |
| [PHASE3_MULTI_MODEL_DESIGN.md](PHASE3_MULTI_MODEL_DESIGN.md) | Multi-model design |
| [COMPLETED_INITIATIVES.md](COMPLETED_INITIATIVES.md) | Completed initiatives |
| [MEMORY_RETRIEVER_INTEGRATION_TESTS.md](MEMORY_RETRIEVER_INTEGRATION_TESTS.md) | Memory retriever tests |

---

## Additional Resources

- **Example Apps:** `examples/apps/` - 2 working example applications (librarian, codex-dashflow) + shared utilities library
- **Crate READMEs:** Each `crates/*/README.md` has crate-specific documentation
- **API Docs:** Run `cargo doc --open` for full API documentation
- **Architecture Docs:** `crates/dashflow/src/*/ARCHITECTURE.md` for module-level docs

---

## Documentation Principles

1. **One Place Per Topic:** Each topic has ONE authoritative document
2. **Link, Don't Duplicate:** Other documents link here instead of repeating
3. **Keep Current:** Update documents when code changes
4. **Delete Stale Docs:** Archive obsolete documentation to `docs/archive/`
