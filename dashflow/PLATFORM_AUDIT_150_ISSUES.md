# Platform Audit: 869 Issues (IMPORTED to M-SERIES)

**Created:** 2025-12-16 (ULTRATHINK deep analysis)
**Updated:** 2025-12-21 (Worker #1385 - Issues imported to ROADMAP_CURRENT.md M-SERIES)
**Status:** ⚠️ IMPORTED - All critical issues migrated to M-162 through M-291+ in ROADMAP_CURRENT.md
**Priority:** Track progress in ROADMAP_CURRENT.md, not here

> **Note (Dec 2025):** This file is now a **historical reference**. Issues have been imported into the M-SERIES backlog in `ROADMAP_CURRENT.md`. The counts below are stale - check the roadmap for current status. All P0 security issues are FIXED. Many aggregate counts (e.g., "3100 array indexing") represent patterns, not individual actionable items.

## Summary

| Priority | Count | Fixed | Remaining | Description |
|----------|-------|-------|-----------|-------------|
| P0 | 64 | 5 | 59 | Critical - Security, deadlocks, data corruption |
| P1 | 609 | 0 | 609 | High - Panics, crashes, blocking, overflow |
| P2 | 201 | 0 | 201 | Medium - Tech debt, hardcoded values, debug noise |
| **Total** | **874** | **5** | **869** | **Estimated 595-745 commits to fix** |

### P0 Security Issues (Fix First)
- ✅ SQL injection: postgres checkpointer (FIXED #868)
- ✅ SQL injection: pgvector (FIXED #868)
- ✅ SQL injection: cassandra (FIXED #868)
- SQL injection: clickhouse (format! with table names)
- ✅ Command injection: shell tool (FIXED #868)
- Command injection: git tool, CLI kill commands
- ✅ Path traversal: file tool (FIXED #868)
- Resource exhaustion: gzip bomb, unbounded HTTP body
- Secrets exposure: API keys in Debug, error messages
- std::sync::Mutex in async context (24 instances) - deadlock risk
- unsafe blocks (2) - need security audit
- Box::leak (1) - intentional memory leak needs review
- block_on in async (107) - blocking runtime, potential deadlock
- **NEW:** 51 Command::new calls (command execution - injection risk)

### P1 Crash/Panic Issues
- 261 JSON/parse panics on malformed input
- 139 file operation panics
- 75 env var panics at startup
- 814 unchecked numeric casts (overflow risk)
- 71 infinite loops without bounds
- 3100 array indexing [0] (out-of-bounds risk)
- 56 iterator .next().unwrap() (empty iterator panic)
- 13 .get(N).unwrap() (index panic)
- 120 .as_ref().unwrap() (None panic)
- 5 unreachable!() (may be reachable)
- 13 todo!() (incomplete implementation)
- 34 unimplemented!() (incomplete code)
- 636 explicit panic!() calls (need review)
- 5 thread join unwrap (panic propagation)
- 39 RwLock read().unwrap() (poisoned lock panic)
- 18 RwLock write().unwrap() (poisoned lock panic)
- 7 split().nth().unwrap() (index on split)
- 3 chars().nth().unwrap() (char index panic)
- 206 serde_json::from_str unwrap (JSON parse panic)
- 322 serde_json::to_string unwrap (serialize panic)
- 69 .map_err(|_| (error context loss)
- **NEW:** 8 .json().await.unwrap (HTTP JSON parse panic)
- **NEW:** 26 .text().await.unwrap (HTTP text read panic)
- **NEW:** 16 RefCell borrow (potential borrow panic in async)

### P2 Tech Debt
- 8125 debug prints (should be tracing)
- 633 hardcoded URLs
- 806 hardcoded timeouts
- 248 suppressed warnings
- 405 error-hiding unwrap_or_default()
- 132 unwrap_or(0) (silent data loss)
- 348 unwrap_or("") (silent empty string)
- 105 unbounded tokio::spawn (task leak risk)
- 1105 tempfile uses (cleanup audit needed)
- 197 PathBuf::from/Path::new (path handling audit)
- 425 std::fs:: (filesystem op error handling)
- **NEW:** 58 reqwest::Client::new() (no timeout configured)

---

## 🟠 OBSERVABILITY/INFRASTRUCTURE ISSUES (2025-12-16) 🟠

### P0: Critical Infrastructure Issues

**OBS-1: Protobuf schema out of sync**
- **Location:** `observability-ui/src/proto/dashstream.ts:5`, `observability-ui/package.json:9`
- **Issue:** `npm --prefix observability-ui run proto:check` fails; decoder imports stale schema
- **Impact:** UI may fail to decode streaming messages correctly

**OBS-2: Docker build missing UI artifact**
- **Location:** `Dockerfile.websocket-server:98`
- **Issue:** Copies `observability-ui/dist` but never builds it; dist not in git
- **Impact:** Clean builds fail or ship stale UI

**OBS-3: Docker healthcheck override breaks status awareness**
- **Location:** `docker-compose.dashstream.yml:154` vs `Dockerfile.websocket-server:103`
- **Issue:** Compose overrides status-aware healthcheck with "200 only" check
- **Impact:** "degraded/waiting" states appear "healthy"

**OBS-4: E2E validation can PASS with no events**
- **Location:** `scripts/e2e_stack_validation.sh:99`, `scripts/e2e_stack_validation.sh:131`
- **Issue:** Only warns on missing data; wrong advanced_rag path check
- **Impact:** False positive test results

**OBS-5: Test script ignores build failures**
- **Location:** `scripts/test_observability_pipeline.sh:152`
- **Issue:** Pipes to tail without pipefail; build errors swallowed
- **Impact:** Broken builds can appear successful

### P1: High - Observability Infrastructure

**OBS-6: Websocket fallback port binding**
- **Location:** `crates/dashflow-observability/src/bin/websocket_server/main.rs:3220-3221` (was `websocket_server.rs:2201` before file split)
- **Issue:** Falls back 3002→3003→...; incompatible with fixed container port mapping
- **Impact:** Container unreachable from expected port

**OBS-7: Runbook references unimplemented route**
- **Location:** `docs/OBSERVABILITY_RUNBOOK.md:641` vs routes at `:2183`
- **Issue:** `/reset-halted` not implemented but documented
- **Impact:** Operators follow broken instructions

**OBS-8: DLQ metrics mismatch in docs**
- **Location:** `docs/OBSERVABILITY_INFRASTRUCTURE.md:253,827`
- **Issue:** Docs list metrics that don't match what server emits (`:1414`)
- **Impact:** Monitoring dashboards may miss actual metrics

**OBS-9: Stale doc counts**
- **Location:** `docs/OBSERVABILITY_INFRASTRUCTURE.md:567,764`
- **Issue:** Claims dashboard is "551 lines", alerts are "10 rules"; both stale
- **Impact:** Documentation drift

**OBS-10: Testing doc wrong container/topic**
- **Location:** `docs/TESTING_OBSERVABILITY.md:41,84`
- **Issue:** Wrong container name + wrong Kafka topic
- **Impact:** Test instructions fail

**OBS-11: Testing doc outdated Grafana query**
- **Location:** `docs/TESTING_OBSERVABILITY.md:117`
- **Issue:** Query payload missing datasource uid
- **Impact:** Example queries fail

**OBS-12: E2E script non-portable**
- **Location:** `scripts/e2e_stack_validation.sh:40,49`
- **Issue:** Checks Kafka with curl (not HTTP); uses `timeout` (not on macOS)
- **Impact:** Scripts fail on macOS

**OBS-13: E2E script hardcoded Grafana IDs**
- **Location:** `scripts/e2e_stack_validation.sh:147,159`
- **Issue:** Hardcodes datasource proxy id 1 and uid "prometheus"
- **Impact:** Brittle; breaks with different Grafana config

**OBS-14: Test script references non-existent metric**
- **Location:** `scripts/test_observability_pipeline.sh:233`
- **Issue:** Verifies `dashstream_quality_accuracy` which doesn't exist
- **Impact:** Test always fails or is misleading

**OBS-15: Test script references wrong compose file**
- **Location:** `scripts/test_observability_pipeline.sh:293`
- **Issue:** Troubleshooting references `docker-compose-kafka.yml` (wrong name)
- **Impact:** Debugging instructions fail

**OBS-16: Screenshot script wrong usage comment**
- **Location:** `scripts/capture_grafana_screenshots.js:2`
- **Issue:** Comment says Playwright test-runner file; it's not
- **Impact:** Confusion about how to run

**OBS-17: Visual regression test issues**
- **Location:** `test-utils/tests/grafana_visual_regression.test.js:79,134`
- **Issue:** Exports Playwright config via module.exports (ignored); uses page.content() for "semantic" checks
- **Impact:** Tests may not run correctly; misses stat values

**OBS-18: Dashboard acceptance test hardcoded UID**
- **Location:** `test-utils/tests/dashboard_acceptance.test.ts:60`
- **Issue:** Hardcodes `uid: 'prometheus'`; depends on missing node-fetch
- **Impact:** Brittle; fails in different environments

**OBS-19: Expected schema E2E incomplete**
- **Location:** `test-utils/src/observability.rs:229`
- **Issue:** Only verifies `/api/expected-schema` returns 200, not that schema is set/persisted/used
- **Impact:** False positive test results

**OBS-20: Grafana query check naive matching**
- **Location:** `test-utils/src/observability.rs:396`
- **Issue:** Uses string matching for "values" instead of parsing frames
- **Impact:** False positives/negatives

**OBS-21: CLI introspection hardcodes Grafana creds**
- **Location:** `crates/dashflow-cli/src/commands/introspect.rs:1392`
- **Issue:** Hardcodes Grafana URL + admin creds
- **Impact:** Non-portable; fails in different environments

**OBS-22: Grafana provisioning allows drift**
- **Location:** `grafana/provisioning/dashboards/default.yml:12`
- **Issue:** Allows UI updates + persistent volume
- **Impact:** Dashboard drift from file source-of-truth across runs

**OBS-23: React/types version mismatch**
- **Location:** `observability-ui/package.json:20,29`
- **Issue:** Pins React 18 but uses @types/react 19 / @types/react-dom 19
- **Impact:** Type drift risk; not checked in CI

### P2: Medium - Observability Tech Debt

**OBS-24: Obsolete compose version key**
- **Location:** `docker-compose.dashstream.yml:36`
- **Issue:** Uses obsolete `version:` key
- **Impact:** Compose warns every run

---

## 🔴 DEEP ANALYSIS ISSUES (2025-12-16) - ULTRATHINK AUDIT 🔴

### P0: CRITICAL - Security and Data Integrity

### ISSUE 73: Security-Deprecated decode_message() Still Called 40+ Times
**Priority:** P0 - CRITICAL (Security vulnerability)
**Location:** `crates/dashflow-streaming/src/codec.rs:440-470`
**Evidence:** "Use decode_message_strict() for untrusted input. Accepts legacy messages without headers - SECURITY RISK"
**Called in:** consumer.rs, backends/file.rs, backends/sqlite.rs, 30+ tests
**Impact:** Production streaming code vulnerable to documented security issue
**Estimated Commits:** 3

### ISSUE 74: MCP Tool Mock Check in Production Path (repeat emphasis)
**Priority:** P0 - CRITICAL
**Location:** `crates/dashflow/src/core/mcp.rs:408-442`
**Evidence:** `call_tool()` checks mock_responses BEFORE real execution
**Impact:** If mock_responses accidentally populated, real tools silently bypassed
**Estimated Commits:** 2

### ISSUE 75: Approval Channel Send Errors Silently Ignored
**Priority:** P0 - CRITICAL (Human-in-loop broken)
**Location:** `crates/dashflow/src/approval.rs:214-238`
**Evidence:** `let _ = self.response_tx.send(ApprovalResponse::approve(...));`
**Impact:** Approval/denial never delivered, stuck workflows or unauthorized actions
**Estimated Commits:** 1

### ISSUE 76: Network Server Startup Errors Silently Swallowed
**Priority:** P0 - CRITICAL
**Location:** `crates/dashflow/src/network/server.rs:674`
**Evidence:** `axum::serve(listener, router).await.ok();` - .ok() swallows errors
**Impact:** Server can crash without any error reported, callers believe it's running
**Estimated Commits:** 1

### P0: CRITICAL - Concurrency Bugs (Deadlocks, Data Corruption)

### ISSUE 77: 5-Level Nested Locks in Annoy VectorStore - Deadlock Risk
**Priority:** P0 - CRITICAL
**Location:** `crates/dashflow-annoy/src/store.rs:286-351`
**Evidence:** Acquires env, next_item_id, metadata_store, id_to_item, is_built locks in sequence
**Impact:** Deadlock if any other method acquires these in different order
**Estimated Commits:** 3

### ISSUE 78: Blocking std::sync::Mutex in Async Context (FAISS, HNSW)
**Priority:** P0 - CRITICAL
**Locations:** faiss_store.rs:157,224,237, hnsw_store.rs:103,111,144
**Evidence:** `self.index.lock().unwrap()` in `async fn` - blocks entire thread
**Impact:** Starves other async tasks, can cause complete hang
**Fix:** Use tokio::sync::Mutex
**Estimated Commits:** 4

### ISSUE 79: TOCTOU Race in FileBackend Offset Management
**Priority:** P0 - CRITICAL (Data corruption)
**Location:** `crates/dashflow-streaming/src/backends/file.rs:192-212`
**Evidence:** Read lock released before write, count_messages() called between
**Impact:** Offset counting incorrect, duplicate or missed messages
**Estimated Commits:** 2

### ISSUE 80: thread_local! in Async Context - Data Corruption
**Priority:** P0 - CRITICAL
**Location:** `crates/dashflow-streaming/src/codec.rs:17-20`, compression.rs:10-11
**Evidence:** thread_local! ENCODE_BUFFER_POOL with RefCell
**Impact:** Async tasks can migrate threads, corrupting borrowed data
**Estimated Commits:** 2

### P1: HIGH - Timeout/Retry Bugs

### ISSUE 81: Default RetryPolicy No Jitter - Thundering Herd
**Priority:** P1 - HIGH
**Location:** `crates/dashflow/src/core/retry.rs:114-126`
**Evidence:** Default uses Exponential without jitter, all clients retry at same time
**Impact:** Service recovery overwhelmed by synchronized retries
**Estimated Commits:** 1

### ISSUE 82: LangServe Client No Default Timeout - Indefinite Hang
**Priority:** P1 - HIGH
**Location:** `crates/dashflow-langserve/src/client.rs:46-81`
**Evidence:** `timeout: None` by default
**Impact:** Slow server hangs application forever
**Estimated Commits:** 1

### ISSUE 83: LangServe/Registry Clients No Retry Logic
**Priority:** P1 - HIGH
**Locations:** langserve/client.rs:140-177, registry/client.rs:583-606
**Evidence:** Single transient failure fails entire operation
**Impact:** Unreliable network breaks all operations
**Estimated Commits:** 2

### ISSUE 84: RemoteNode Retries Non-Retryable Errors
**Priority:** P1 - HIGH
**Location:** `crates/dashflow-remote-node/src/client.rs:283-329`
**Evidence:** Both retryable and non-retryable return same Error type
**Impact:** Wasteful retries, delayed error reporting
**Estimated Commits:** 2

### ISSUE 85: BlockingPrometheusClient Ignores Configured Timeout
**Priority:** P1 - HIGH
**Location:** `crates/dashflow/src/prometheus_client.rs:311-318`
**Evidence:** timeout stored but not passed to Client::new()
**Impact:** Prometheus queries can hang indefinitely
**Estimated Commits:** 1

### P1: HIGH - Configuration Ignored

### ISSUE 86: RunnableConfig.max_concurrency Never Used
**Priority:** P1 - HIGH
**Location:** `crates/dashflow/src/core/config.rs:125`
**Evidence:** Field exists, builder sets it, but never read during execution
**Impact:** Users expect concurrency limiting that doesn't exist
**Estimated Commits:** 2

### ISSUE 87: TrustConfig All Fields Never Enforced
**Priority:** P1 - HIGH
**Location:** `crates/dashflow/src/packages/config.rs:312-325`
**Evidence:** required_signatures, allow_unsigned, reject_vulnerable, minimum_trust all ignored
**Impact:** Package security settings have no effect
**Estimated Commits:** 3

### ISSUE 88: CacheConfig.max_size_mb/ttl/offline Never Enforced
**Priority:** P1 - HIGH
**Location:** `crates/dashflow/src/packages/config.rs:434-446`
**Evidence:** Fields parsed but no eviction, no TTL, no offline mode
**Impact:** Cache grows unbounded
**Estimated Commits:** 2

### ISSUE 89: McpServerConfig.timeout_ms Never Used
**Priority:** P1 - HIGH
**Location:** `crates/dashflow/src/core/mcp.rs:66`
**Evidence:** Field stored but call_tool has no timeout
**Impact:** MCP tool calls can hang forever
**Estimated Commits:** 1

### P1: HIGH - Error Swallowing

### ISSUE 90: Cache Invalidation Failures Silently Ignored
**Priority:** P1 - HIGH
**Location:** `crates/dashflow-registry/src/api/routes/packages.rs:100-108`
**Evidence:** `let _ = state.data_cache.delete_pattern(...).await;`
**Impact:** Stale package versions served from cache
**Estimated Commits:** 1

### ISSUE 91: Prometheus Metric Registration Failures Ignored
**Priority:** P1 - HIGH
**Location:** streaming/metrics_utils.rs:230,299, observability/metrics.rs:444-486
**Evidence:** `let _ = prometheus::default_registry().register(...);`
**Impact:** Metrics silently missing from dashboards
**Estimated Commits:** 1

### ISSUE 92: Cost Tracking Budget Ignored on Lock Failure
**Priority:** P1 - HIGH
**Location:** `crates/dashflow-observability/src/cost.rs:763-801`
**Evidence:** `if let Ok(mut state) = self.state.lock() { ... }` returns self on failure
**Impact:** Users think they have cost protection when they don't
**Estimated Commits:** 1

### P1: HIGH - Documentation Lies

### ISSUE 93: invoke_with_callback Documented But Doesn't Exist
**Priority:** P1 - HIGH
**Locations:** dashstream_callback/mod.rs, README.md:228-229
**Evidence:** Method shown in docs but not implemented
**Impact:** Users copying examples get compile errors
**Estimated Commits:** 1

### ISSUE 94: Rust Version Requirement Wrong (1.75 vs 1.80)
**Priority:** P1 - HIGH
**Locations:** QUICKSTART.md:19,24 says 1.75+, Cargo.toml:67 says 1.80
**Impact:** Users with Rust 1.75-1.79 fail to build
**Estimated Commits:** 1

### ISSUE 95: DashSwarm Registry URL Fictional
**Priority:** P1 - HIGH
**Locations:** README.md:1055, dashswarm.rs:101
**Evidence:** "registry.dashswarm.com" - no running service
**Impact:** Connection failures when using central registry
**Estimated Commits:** 1

### P1: HIGH - Dead/Deprecated Code Still Active

### ISSUE 96: #[allow(dead_code)] Hiding Complete Consumer Group Implementation
**Priority:** P1 - HIGH
**Location:** `crates/dashflow-streaming/src/backends/file.rs:153-160`
**Evidence:** save_offsets() fully implemented but marked dead
**Impact:** Feature exists but is unreachable
**Estimated Commits:** 1

### ISSUE 97: _ => {} Swallowing Unknown Bedrock Stream Events
**Priority:** P1 - HIGH
**Location:** `crates/dashflow-bedrock/src/chat_models.rs:352,494,759,810`
**Evidence:** Unknown AWS events silently dropped
**Impact:** New AWS features (rate limiting, usage metrics) ignored
**Estimated Commits:** 1

### ISSUE 98: Mutex Poisoning with .unwrap() - Cascading Panics
**Priority:** P1 - HIGH
**Locations:** faiss_store.rs, hnsw_store.rs, usearch_store.rs (100+ .unwrap() on locks)
**Evidence:** One panic poisons mutex, all subsequent accesses panic
**Impact:** Single failure crashes all threads
**Estimated Commits:** 3

### P2: MEDIUM - Additional Issues

### ISSUE 99: RefCell in TraceStore Not Thread-Safe ✅ FIXED
**Priority:** P2 - MEDIUM
**Location:** `crates/dashflow/src/unified_introspection.rs:519-528` (was 512-536)
**Evidence:** ~~`cache: RefCell<LruCache<...>>` - RefCell is not Sync~~ Now uses `Mutex<LruCache<...>>`
**Impact:** ~~Runtime panic if used across threads~~ FIXED - TraceStore is now thread-safe
**Status:** FIXED - Code now uses Mutex instead of RefCell (line 526: `cache: Mutex<LruCache<PathBuf, CachedTrace>>`)

### ISSUE 100: Rate Limiter acquire_blocking() No Jitter ✅ FIXED
**Priority:** P2 - MEDIUM
**Location:** `crates/dashflow/src/self_improvement/resilience.rs:1156-1179` (was 997-1003)
**Evidence:** ~~All threads wake at same time~~ Now has configurable jitter (default 25%)
**Impact:** ~~Contention thundering herd~~ FIXED - `apply_jitter()` at line 1170 randomizes wait times
**Status:** FIXED - `acquire_blocking()` now applies jitter via `jitter_factor` config (line 916, default 0.25)

### ISSUE 101: Hardcoded Timeouts Not Configurable
**Priority:** P2 - MEDIUM
**Locations:** streaming/rate_limiter.rs, prometheus_client.rs, registry/client.rs
**Evidence:** Duration::from_secs(2), (10), (30) hardcoded
**Impact:** Cannot tune for different environments
**Estimated Commits:** 2

### ISSUE 102: Test Skip Without #[ignore] Attribute
**Priority:** P2 - MEDIUM
**Locations:** gitlab/lib.rs:770-906 and many similar
**Evidence:** Tests return early if env var missing, appear to pass
**Impact:** False test coverage in CI
**Estimated Commits:** 2

---

## 🔴 SECURITY VULNERABILITIES - ULTRATHINK AUDIT 2 (2025-12-16) 🔴

### P0: CRITICAL - Injection Vulnerabilities

### ISSUE 103: SQL Injection via format!() in PostgreSQL Checkpointer ✅ FIXED (Worker #868)
**Priority:** P0 - CRITICAL (Data breach risk)
**Location:** `crates/dashflow-postgres-checkpointer/src/lib.rs`
**Evidence:** `format!("DELETE FROM {}", self.table_name)` - table name interpolated
**Impact:** If table_name comes from user input, arbitrary SQL execution
**Fix:** Added `validate_identifier()` function that validates table names against PostgreSQL identifier rules (alphanumeric + underscore, max 63 chars)
**Status:** FIXED - Validation added to `with_table_name()`

### ISSUE 104: SQL Injection in PgVector VectorStore ✅ FIXED (Worker #868)
**Priority:** P0 - CRITICAL
**Location:** `crates/dashflow-pgvector/src/pgvector_store.rs`
**Evidence:** `format!("CREATE TABLE {} (...)", table_name)` - no parameterization
**Impact:** SQL injection on table creation/queries
**Fix:** Added `validate_identifier()` function and validation in `PgVectorStore::new()`
**Status:** FIXED - Collection name validated before use

### ISSUE 105: SQL Injection in Cassandra VectorStore ✅ FIXED (Worker #868)
**Priority:** P0 - CRITICAL
**Location:** `crates/dashflow-cassandra/src/cassandra_store.rs`
**Evidence:** Multiple `format!()` for table/keyspace names
**Impact:** CQL injection possible
**Fix:** Added `validate_identifier()` function with Cassandra rules (max 48 chars)
**Status:** FIXED - Keyspace and table names validated in builder

### ISSUE 106: Command Injection in Shell Tool ✅ FIXED (Worker #868)
**Priority:** P0 - CRITICAL
**Location:** `crates/dashflow-shell-tool/src/lib.rs`
**Evidence:** User command passed to `sh -c` without sanitization
**Impact:** Arbitrary command execution via malicious input
**Fix:** Added shell metacharacter detection (`;`, `|`, `&`, `` ` ``, `$()`, `${`, `||`, `&&`) when allowlists are configured
**Status:** FIXED - Commands with injection patterns rejected when restrictions enabled

### ISSUE 107: Path Traversal in File Tool ✅ FIXED (Worker #868)
**Priority:** P0 - CRITICAL
**Location:** `crates/dashflow-file-tool/src/lib.rs`
**Evidence:** `../` sequences not blocked in file paths
**Impact:** Access to files outside intended directory
**Fix:** Added `contains_path_traversal()` check and `normalize_path_for_check()` for non-existent files
**Status:** FIXED - Path traversal patterns blocked, non-existent file paths properly normalized

### ISSUE 108: SSRF Vulnerability in HTTP Request Tools
**Priority:** P0 - CRITICAL
**Location:** `crates/dashflow-webscrape/src/lib.rs`, langserve client
**Evidence:** User-provided URLs fetched without allowlist
**Impact:** Can probe internal network, access metadata services
**Estimated Commits:** 3

### ISSUE 109: ReDoS Vulnerability in Regex Patterns
**Priority:** P1 - HIGH
**Locations:** Multiple crates with regex compilation from user input
**Evidence:** No timeout on regex evaluation, catastrophic backtracking possible
**Impact:** CPU exhaustion denial of service
**Estimated Commits:** 2

### P0: CRITICAL - Resource Exhaustion

### ISSUE 110: Gzip Bomb Vulnerability - No Decompression Limit
**Priority:** P0 - CRITICAL
**Location:** `crates/dashflow-streaming/src/compression.rs`
**Evidence:** `decoder.read_to_end(&mut decompressed)` - no size limit
**Impact:** 1KB compressed → 1GB decompressed → OOM
**Estimated Commits:** 2

### ISSUE 111: Unbounded HTTP Response Body Reading
**Priority:** P0 - CRITICAL
**Locations:** webscrape, langserve, registry client
**Evidence:** `response.bytes().await` with no size limit
**Impact:** Malicious server can send infinite data → OOM
**Estimated Commits:** 3

### ISSUE 112: Infinite Loop in OpenAI Assistant wait_for_run
**Priority:** P0 - CRITICAL
**Location:** `crates/dashflow-openai/src/assistant.rs`
**Evidence:** `loop { status = poll(); if complete { break; } sleep(1s); }` - no max iterations
**Impact:** Stuck run = infinite loop = hung process
**Estimated Commits:** 1

### ISSUE 113: No Pagination Limit on List Operations
**Priority:** P1 - HIGH
**Locations:** Registry list packages, checkpoint list, trace list
**Evidence:** Fetches all items into memory
**Impact:** Large datasets cause OOM
**Estimated Commits:** 2

### ISSUE 114: JSON Parsing Without Size Limit
**Priority:** P1 - HIGH
**Locations:** All API response parsing
**Evidence:** `serde_json::from_str(&response)` on untrusted input
**Impact:** 100MB JSON response → OOM during parsing
**Estimated Commits:** 2

### ISSUE 115: No Rate Limiting on Graph Execution Spawning
**Priority:** P1 - HIGH
**Location:** `crates/dashflow/src/executor.rs`
**Evidence:** Parallel nodes spawn unlimited tasks
**Impact:** Graph with 1000 parallel branches → thread exhaustion
**Estimated Commits:** 2

### P1: HIGH - Unsafe Code and Panics

### ISSUE 116: Vec Index Without Bounds Check
**Priority:** P1 - HIGH
**Locations:** Multiple vectorstores, executor
**Evidence:** `items[0]` without checking `items.is_empty()`
**Impact:** Panic on empty results
**Estimated Commits:** 3

### ISSUE 117: String Slicing at Byte Positions
**Priority:** P1 - HIGH
**Locations:** Tokenizer wrappers, text splitting
**Evidence:** `text[start..end]` where positions may be in middle of UTF-8
**Impact:** Panic on multi-byte characters
**Estimated Commits:** 2

### ISSUE 118: Numeric Overflow in Token Counting
**Priority:** P1 - HIGH
**Location:** `crates/dashflow-observability/src/cost.rs`
**Evidence:** Token counts summed as usize without overflow check
**Impact:** Integer wrap on extreme usage → wrong costs
**Estimated Commits:** 1

### ISSUE 119: Division by Zero in Statistics
**Priority:** P1 - HIGH
**Locations:** Metrics averaging, quality scoring
**Evidence:** `total / count` where count can be 0
**Impact:** Panic or NaN propagation
**Estimated Commits:** 2

### ISSUE 120: Unsafe transmute in FFI Code
**Priority:** P1 - HIGH
**Location:** `crates/dashflow-annoy/src/store.rs` (LMDB bindings)
**Evidence:** Raw pointer conversion without size validation
**Impact:** Memory corruption if size mismatch
**Estimated Commits:** 2

### P0: CRITICAL - Secrets Exposure

### ISSUE 121: API Keys Exposed via Debug Trait
**Priority:** P0 - CRITICAL
**Locations:** OpenAI, Anthropic, Bedrock config structs
**Evidence:** `#[derive(Debug)]` on structs containing `api_key: String`
**Impact:** Debug logs expose secrets: `println!("{:?}", config)`
**Estimated Commits:** 3

### ISSUE 122: Secrets in Error Messages
**Priority:** P0 - CRITICAL
**Location:** HTTP client error handling
**Evidence:** `format!("Failed to connect to {}", url_with_auth)`
**Impact:** Auth tokens in error logs
**Estimated Commits:** 2

### ISSUE 123: Trace Files May Contain API Responses With PII
**Priority:** P1 - HIGH
**Location:** `crates/dashflow/src/executor.rs` trace persistence
**Evidence:** Full state serialized including LLM responses
**Impact:** PII persisted to disk, compliance violation
**Estimated Commits:** 2

### ISSUE 124: No Secret Redaction in Prometheus Metrics
**Priority:** P1 - HIGH
**Location:** `crates/dashflow-prometheus-exporter/src/lib.rs`
**Evidence:** Labels can contain user data without redaction
**Impact:** Secrets in metrics endpoint
**Estimated Commits:** 1

### P1: HIGH - Incomplete Implementations

### ISSUE 125: OAuth Token Refresh Not Implemented
**Priority:** P1 - HIGH
**Location:** `crates/dashflow-google-search/src/lib.rs`
**Evidence:** Token stored but refresh_token logic missing
**Impact:** Searches fail after token expiry
**Estimated Commits:** 2

### ISSUE 126: WASM Executor Memory Limits Not Enforced
**Priority:** P1 - HIGH
**Location:** `crates/dashflow-wasm-executor/src/executor.rs:257`
**Evidence:** "Memory limits documented as deferred"
**Impact:** Malicious WASM can consume unlimited memory
**Estimated Commits:** 3

### ISSUE 127: Colony Worker Health Check Always Returns Healthy
**Priority:** P1 - HIGH
**Location:** `crates/dashflow/src/colony/worker.rs`
**Evidence:** Health endpoint hardcodes success
**Impact:** Dead workers not detected
**Estimated Commits:** 1

### ISSUE 128: Package Signature Verification Skipped
**Priority:** P1 - HIGH
**Location:** `crates/dashflow-cli/src/commands/pkg.rs:verify`
**Evidence:** TrustService created but verify() never called
**Impact:** Malicious packages pass verification
**Estimated Commits:** 2

### ISSUE 129: Tool Schema Validation Not Enforced
**Priority:** P1 - HIGH
**Location:** `crates/dashflow/src/core/tools.rs`
**Evidence:** Schema defined but input not validated against it
**Impact:** Invalid tool inputs cause runtime errors
**Estimated Commits:** 2

### ISSUE 130: Model Context Length Not Enforced
**Priority:** P1 - HIGH
**Locations:** All LLM providers
**Evidence:** Token count calculated but not compared to model limit
**Impact:** Requests fail with confusing API errors
**Estimated Commits:** 2

### ISSUE 131: Streaming Backpressure Not Implemented
**Priority:** P1 - HIGH
**Location:** `crates/dashflow-streaming/src/consumer.rs`
**Evidence:** Consumer processes at max speed regardless of downstream
**Impact:** Memory grows unbounded under load
**Estimated Commits:** 3

### P1: HIGH - Network Security

### ISSUE 132: Hardcoded Insecure JWT Secret
**Priority:** P0 - CRITICAL
**Location:** `crates/dashflow-registry/src/api/auth.rs`
**Evidence:** `"insecure-default-secret-CHANGE-ME"` as fallback
**Impact:** Auth tokens forgeable if env var not set
**Estimated Commits:** 1

### ISSUE 133: CORS Allows All Origins
**Priority:** P1 - HIGH
**Location:** `crates/dashflow-observability/src/bin/websocket_server/main.rs`
**Evidence:** `.allow_origin(Any).allow_methods(Any)`
**Impact:** Cross-site attacks possible
**Estimated Commits:** 1

### ISSUE 134: No TLS Certificate Validation Option
**Priority:** P1 - HIGH
**Locations:** HTTP clients in multiple crates
**Evidence:** No way to disable TLS verify for self-signed certs in dev
**Impact:** Can't test with internal CAs without code change
**Estimated Commits:** 2

### ISSUE 135: WebSocket Server No Authentication
**Priority:** P1 - HIGH
**Location:** `crates/dashflow-observability/src/bin/websocket_server/main.rs`
**Evidence:** No auth on WebSocket upgrade
**Impact:** Anyone can subscribe to all events
**Estimated Commits:** 2

### ISSUE 136: No Request Size Limit on HTTP Server
**Priority:** P1 - HIGH
**Location:** Registry API server
**Evidence:** Body parsing with no max size
**Impact:** DoS via large request bodies
**Estimated Commits:** 1

### P2: MEDIUM - Additional Security Issues

### ISSUE 137: Weak Random in Test Utils Used in Examples
**Priority:** P2 - MEDIUM
**Location:** `test-utils/src/mock_embeddings.rs`
**Evidence:** `rand::thread_rng()` for vector generation
**Impact:** Predictable "embeddings" if copied to production
**Estimated Commits:** 1

### ISSUE 138: Temporary Files Created Insecurely
**Priority:** P2 - MEDIUM
**Locations:** Dataset processing, trace export
**Evidence:** `tempfile::tempdir()` used without explicit permissions
**Impact:** Temp files may be world-readable
**Estimated Commits:** 1

### ISSUE 139: No Input Sanitization on Log Messages
**Priority:** P2 - MEDIUM
**Locations:** Throughout codebase
**Evidence:** User input directly in log messages
**Impact:** Log injection attacks possible
**Estimated Commits:** 2

### ISSUE 140: Hardcoded Default Ports Conflict Risk
**Priority:** P2 - MEDIUM
**Evidence:** Multiple services default to same ports (9090, 3000)
**Impact:** Services fail to start in dev environment
**Estimated Commits:** 1

### P2: Incomplete Error Handling

### ISSUE 141: DNS Resolution Errors Not Distinguished
**Priority:** P2 - MEDIUM
**Locations:** All HTTP clients
**Evidence:** DNS errors reported as generic connection failure
**Impact:** Hard to debug "cannot connect" errors
**Estimated Commits:** 2

### ISSUE 142: Connection Pool Exhaustion Silent
**Priority:** P2 - MEDIUM
**Locations:** Database connection pools
**Evidence:** Timeout on pool acquisition not clearly reported
**Impact:** Deadlock appears as slow response
**Estimated Commits:** 1

### ISSUE 143: Partial Write Not Detected
**Priority:** P2 - MEDIUM
**Location:** Checkpoint persistence
**Evidence:** File write without fsync or atomic rename
**Impact:** Corrupted checkpoints on crash
**Estimated Commits:** 2

### P2: Testing Gaps

### ISSUE 144: Mock HTTP Server Accepts All Paths
**Priority:** P2 - MEDIUM
**Location:** Test utilities
**Evidence:** Mock server returns success for any path
**Impact:** Tests pass with wrong URLs
**Estimated Commits:** 1

### ISSUE 145: Concurrent Access Not Tested
**Priority:** P2 - MEDIUM
**Locations:** Vector stores, checkpointers
**Evidence:** Only single-threaded tests
**Impact:** Race conditions in production
**Estimated Commits:** 3

### ISSUE 146: Error Recovery Paths Not Tested
**Priority:** P2 - MEDIUM
**Evidence:** Most tests only check happy path
**Impact:** Error handling code may be broken
**Estimated Commits:** 3

### P2: Code Quality Issues

### ISSUE 147: Magic Numbers Throughout Codebase
**Priority:** P2 - MEDIUM
**Evidence:** `sleep(Duration::from_secs(5))`, `timeout(30)`, etc.
**Impact:** Hard to tune behavior
**Estimated Commits:** 2

### ISSUE 148: Inconsistent Error Message Format
**Priority:** P2 - MEDIUM
**Evidence:** Some errors use lowercase, some uppercase, some have context
**Impact:** Hard to grep logs
**Estimated Commits:** 2

### ISSUE 149: Clone in Hot Path
**Priority:** P2 - MEDIUM
**Location:** `crates/dashflow/src/executor/execution.rs:601`
**Evidence:** State cloned for every node execution
**Impact:** Performance degradation for large state
**Estimated Commits:** 2

### ISSUE 150: Excessive Unwrap in Non-Test Code
**Priority:** P2 - MEDIUM
**Evidence:** 150+ `.unwrap()` outside test modules
**Impact:** Runtime panics on unexpected None/Err
**Estimated Commits:** 5

---

## TOTAL ISSUE COUNT: 150

| Priority | Count | Description |
|----------|-------|-------------|
| P0 | 35 | Critical - Security, deadlocks, data corruption |
| P1 | 75 | High - Reliability, config, error handling |
| P2 | 40 | Medium - Quality, maintainability |

---

## 🔴🔴🔴 REVISED WORKER EXECUTION PRIORITY - 150 ISSUES 🔴🔴🔴

### WAVE 0: CRITICAL SECURITY (15-20 commits)
**Goal:** Fix injection, secrets exposure, and resource exhaustion

| Issue | Task | Est. |
|-------|------|------|
| 103-105 | Fix SQL injection in postgres/pgvector/cassandra | 6 |
| 106 | Fix command injection in shell tool | 3 |
| 107-108 | Fix path traversal, SSRF vulnerabilities | 5 |
| 110-112 | Fix gzip bomb, unbounded HTTP, infinite loop | 6 |
| 121-122, 132 | Fix secrets in debug/errors/JWT | 6 |

### WAVE 1: CONCURRENCY & DATA INTEGRITY (12-15 commits)
**Goal:** No deadlocks, no data corruption

| Issue | Task | Est. |
|-------|------|------|
| 73 | Replace decode_message() with decode_message_strict() | 3 |
| 74, 59 | Move MCP mock to test-only feature | 2 |
| 75, 76 | Fix silent approval/server failures | 2 |
| 77, 78 | Fix deadlock risks, use tokio::sync::Mutex | 4 |
| 79, 80 | Fix TOCTOU race, thread_local in async | 4 |

### WAVE 2: CI & Core Tracing (8-10 commits)
**Goal:** CI tests real things, tracing works

| Issue | Task | Est. |
|-------|------|------|
| 19 | Migrate deprecated ChatOpenAI::new() | 2-3 |
| 63-64 | Enable multi-turn/citation tests | 2 |
| 1, 3, 7 | Token tracking, timestamps, thread_id | 2-3 |
| 6 | Wire trace loading to self-improve | 1 |

### WAVE 3: Silent Failures & Timeouts (12-15 commits)
**Goal:** Errors are visible, timeouts work

| Issue | Task | Est. |
|-------|------|------|
| 90, 91, 92 | Fix cache/metrics/cost silent failures | 3 |
| 81-85 | Fix timeout/retry bugs | 6 |
| 98 | Handle poisoned mutexes gracefully | 3 |
| 116-119 | Fix bounds check, UTF-8 slicing, div-by-zero | 8 |

### WAVE 4: Config & Documentation (10-12 commits)
**Goal:** Config actually works, docs are true

| Issue | Task | Est. |
|-------|------|------|
| 86-89 | Wire max_concurrency, TrustConfig, CacheConfig, MCP timeout | 8 |
| 93-95 | Fix broken docs, rust version, registry URL | 3 |
| 123-124 | PII in traces, secret redaction | 3 |

### WAVE 5: Incomplete Features (15-20 commits)
**Goal:** Advertised features actually work

| Issue | Task | Est. |
|-------|------|------|
| 125-131 | OAuth refresh, WASM limits, health checks, schema validation | 14 |
| 133-136 | CORS, TLS validation, WebSocket auth, request limits | 6 |

### WAVE 6: Distributed & Vector Stores (15-18 commits)
**Goal:** Colony and vector stores work correctly

| Issue | Task | Est. |
|-------|------|------|
| 45-48 | System monitor, remote execution, distillation | 10 |
| 51-54 | Pinecone/Qdrant stubs, Annoy delete, filters | 10 |

### WAVE 7: Testing Quality (15-20 commits)
**Goal:** Tests catch real bugs

| Issue | Task | Est. |
|-------|------|------|
| 57-62 | Mock embeddings warning, quality gate tests | 8 |
| 69-70 | Value assertions, non-empty tests | 8 |
| 144-146 | Mock server paths, concurrent tests, error paths | 7 |

### WAVE 8: Code Quality (10-15 commits)
**Goal:** Clean, maintainable code

| Issue | Task | Est. |
|-------|------|------|
| 137-143 | Temp files, log sanitization, DNS errors, pool exhaustion | 10 |
| 147-150 | Magic numbers, error format, clones, unwraps | 11 |

---

## 🔴 ADDITIONAL ISSUES FOUND (2025-12-16) - CODEBASE SCAN 🔴

### ISSUE 151-160: Silently Ignored Errors (.ok())
**Priority:** P1 - HIGH
**Count:** 57 instances in non-test code
**Evidence:** `grep -rn "\.ok();" crates/`
**Impact:** Errors swallowed, failures go unnoticed
**Key locations:**
- `dashflow-langsmith/src/client.rs:217,223,229`
- `dashflow/src/network/server.rs:674` (server crash ignored)
- `dashflow-clickup/src/api.rs:75-84` (API errors ignored)
**Estimated Commits:** 10

### ISSUE 161-170: Ignored Result Values (let _ =)
**Priority:** P1 - HIGH
**Count:** 345 instances in non-test code
**Evidence:** `grep -rn "let _ =" crates/`
**Impact:** Return values discarded, potential resource leaks or missed errors
**Estimated Commits:** 15

### ISSUE 171-180: Excessive panic! in Production Code
**Priority:** P1 - HIGH
**Count:** 582 instances in non-test code
**Evidence:** `grep -rn "panic!" crates/`
**Impact:** Production crashes instead of graceful error handling
**Estimated Commits:** 20

### ISSUE 181-190: SQL Injection in ClickHouse
**Priority:** P0 - CRITICAL
**Location:** `crates/dashflow-clickhouse/src/clickhouse_store.rs:115,247,305`
**Evidence:** `format!("CREATE DATABASE IF NOT EXISTS {}", self.database)`
**Impact:** Database/table names not sanitized, SQL injection possible
**Estimated Commits:** 3

### ISSUE 191: Memory Leak via Box::leak
**Priority:** P1 - HIGH
**Status:** MOOT - `dashflow-zapier` crate removed from this repository (Zapier NLA API sunset 2023-11-17)
**Location:** (removed) formerly `crates/dashflow-zapier/src/lib.rs:457`
**Impact:** N/A (removed)
**Estimated Commits:** 1

### ISSUE 192-195: Command Execution Without Sanitization
**Priority:** P0 - CRITICAL
**Locations:**
- `dashflow-git-tool/src/lib.rs:847-872` (git commands)
- `dashflow-cli/src/commands/mcp_server.rs:113-187` (kill/taskkill)
**Evidence:** User input passed to Command::new() args
**Impact:** Command injection if input not sanitized
**Estimated Commits:** 4

### ISSUE 196-200: Unchecked Array Access with unwrap()
**Priority:** P1 - HIGH
**Locations:**
- `dashflow-chains/src/natbot/chain.rs:224,237` - `parts[0].parse().unwrap()`
- `dashflow-chains/src/qa_with_sources.rs:154` - `parts[1].lines().next().unwrap()`
- `dashflow-voyage/src/rerank.rs:498-499` - `reranked[0]`, `reranked[1]`
**Impact:** Panics on empty arrays
**Estimated Commits:** 5

### ISSUE 201-210: std::sync::Mutex in Async Context
**Priority:** P0 - CRITICAL
**Count:** 24 instances
**Evidence:** `grep -rn "std::sync::Mutex" crates/` in async code
**Impact:** Blocks entire tokio thread, can deadlock runtime
**Fix:** Use tokio::sync::Mutex
**Estimated Commits:** 8

### ISSUE 211-230: Unchecked Numeric Casts
**Priority:** P1 - HIGH
**Count:** 814 instances of `as usize`, `as i32`, `as u64`, etc.
**Evidence:** `grep -rn "as usize\|as i32" crates/`
**Impact:** Integer overflow, underflow, truncation bugs
**Fix:** Use TryFrom, checked_*, saturating_*
**Estimated Commits:** 30

### ISSUE 231-250: JSON/Parse Panics
**Priority:** P1 - HIGH
**Count:** 261 instances (211 JSON + 50 parse)
**Evidence:** `serde_json::from_str().unwrap()`, `.parse().unwrap()`
**Impact:** Crashes on malformed input
**Fix:** Return Result, handle errors
**Estimated Commits:** 20

### ISSUE 251-270: File Operation Panics
**Priority:** P1 - HIGH
**Count:** 139 instances
**Evidence:** `std::fs::*.unwrap()`, `tokio::fs::*.unwrap()`
**Impact:** Crashes on missing files, permission errors
**Estimated Commits:** 15

### ISSUE 271-290: Environment Variable Panics
**Priority:** P1 - HIGH
**Count:** 75 instances
**Evidence:** `env::var().unwrap()`, `env::var().expect()`
**Impact:** Startup crashes if env var missing
**Fix:** Use env::var().ok() or provide defaults
**Estimated Commits:** 8

### ISSUE 291-310: Infinite Loops Without Bounds
**Priority:** P1 - HIGH
**Count:** 71 `loop {` blocks
**Evidence:** Many lack break conditions or timeouts
**Impact:** Hangs, resource exhaustion
**Estimated Commits:** 15

### ISSUE 311-330: Hardcoded URLs/Endpoints
**Priority:** P2 - MEDIUM
**Count:** 633 hardcoded http:// or localhost:
**Impact:** Non-configurable, breaks in different environments
**Estimated Commits:** 20

### ISSUE 331-350: Hardcoded Timeouts/Durations
**Priority:** P2 - MEDIUM
**Count:** 806 Duration::from_* without configuration
**Impact:** Non-tunable performance, inappropriate defaults
**Estimated Commits:** 25

### ISSUE 351-370: Debug Prints in Production Code
**Priority:** P2 - MEDIUM
**Count:** 8125 println!/dbg!/eprintln!
**Impact:** Performance overhead, noisy logs, potential info leak
**Fix:** Use tracing/log macros with levels
**Estimated Commits:** 40

### ISSUE 371-390: Suppressed Warnings (#[allow])
**Priority:** P2 - MEDIUM
**Count:** 248 #[allow(...)] attributes
**Impact:** Hides real issues, tech debt
**Fix:** Fix underlying issues instead of suppressing
**Estimated Commits:** 20

### ISSUE 391-400: unwrap_or_default() Hiding Errors
**Priority:** P2 - MEDIUM
**Count:** 405 instances
**Impact:** Errors silently converted to defaults, hard to debug
**Estimated Commits:** 15

### ISSUE 401-410: Global Mutable State
**Priority:** P1 - HIGH
**Count:** 26 static mut/lazy_static/once_cell
**Impact:** Thread safety issues, hard to test
**Estimated Commits:** 10

### ISSUE 411-420: Lock Operations That Panic
**Priority:** P1 - HIGH
**Count:** 403 instances of `.lock().unwrap()`, `.read().unwrap()`, `.write().unwrap()`
**Evidence:** `grep -rn "\.lock()\|\.read()\|\.write()" crates/ | grep "unwrap\|expect"`
**Impact:** Panics on poisoned mutex (another thread panicked while holding lock)
**Fix:** Use `.lock().expect("specific reason")` or handle PoisonError
**Estimated Commits:** 20

### ISSUE 421-430: Assertions in Production Code
**Priority:** P2 - MEDIUM
**Count:** 23,675 `assert!` macros in non-test code
**Evidence:** `grep -rn "assert!\|assert_eq!" crates/ | grep -v test`
**Impact:** Many are appropriate, but some may crash production on edge cases
**Note:** Review needed - not all are bugs, but excessive assertions can mask errors
**Estimated Commits:** 15 (for inappropriate ones)

### ISSUE 431-440: first()/last() Panics on Empty Collections
**Priority:** P1 - HIGH
**Count:** 62 instances of `.first().unwrap()`, `.last().unwrap()`
**Evidence:** `grep -rn "\.first()\|\.last()" crates/ | grep "unwrap\|expect"`
**Impact:** Panics on empty vectors/slices
**Fix:** Use `.first().ok_or()` or pattern match
**Estimated Commits:** 8

### ISSUE 441-450: Regex Compilation Panics
**Priority:** P1 - HIGH
**Count:** 85 instances of `Regex::new().unwrap()`
**Evidence:** `grep -rn "Regex::new" crates/ | grep "unwrap\|expect"`
**Impact:** Panics on invalid regex patterns (should be compile-time const)
**Fix:** Use `lazy_static!` with `Regex::new().unwrap()` or `regex!` macro
**Estimated Commits:** 10

### ISSUE 451-460: Time Operation Panics
**Priority:** P1 - HIGH
**Count:** 31 instances of SystemTime/Duration operations with unwrap
**Evidence:** `grep -rn "SystemTime\|Duration" crates/ | grep "unwrap\|expect"`
**Impact:** Panics on time calculation errors (e.g., system time before epoch)
**Estimated Commits:** 5

### ISSUE 461-470: pop()/remove() Panics on Empty
**Priority:** P1 - HIGH
**Count:** 35 instances of `.pop().unwrap()` plus 136 `.remove()` calls
**Evidence:** `grep -rn "\.pop()\|\.remove(" crates/ | grep "unwrap"`
**Impact:** Panics when collection is empty or index out of bounds
**Estimated Commits:** 8

### ISSUE 471-480: TryInto/TryFrom with unwrap (defeats purpose)
**Priority:** P1 - HIGH
**Count:** 18 instances of `try_into().unwrap()` or `try_from().unwrap()`
**Evidence:** `grep -rn "try_into\|try_from" crates/ | grep "unwrap\|expect"`
**Impact:** Using fallible conversion then panicking defeats the purpose of Try*
**Fix:** Propagate the error or use `into()` if infallible
**Estimated Commits:** 5

## TOTAL ISSUE COUNT: 530+

| Priority | Count | Description |
|----------|-------|-------------|
| P0 | 52 | Security, deadlocks, data corruption |
| P1 | 306 | Panics, crashes, blocking, overflow, locks, regex, time |
| P2 | 165 | Tech debt, hardcoded values, debug noise, assertions |
| **Total** | **~523** | |

## Positive Patterns (for reference)

The codebase does have good patterns in places:
- 2,521 proper error transformations (`.map_err()`, `.ok_or()`)
- 7,808 proper error propagations (`return Err`, `?`)
- 217 Error trait implementations
- 391 thiserror error types

---

**TOTAL ESTIMATED COMMITS: 350-420 commits**

**Priority Summary:**
- WAVE 0: P0 Security (50-60 commits) - SQL injection, command injection, secrets
- WAVE 1: P0 Concurrency (15-20 commits) - Mutex deadlocks, blocking in async
- WAVE 2: P1 Panics (100-120 commits) - JSON/parse/file/env/lock/regex/time panics
- WAVE 3: P1 Overflow (30-40 commits) - Numeric casts, bounds checks
- WAVE 4: P1 Other (50-60 commits) - Loops, ignored errors, global state, try_into
- WAVE 5: P2 Cleanup (100-120 commits) - Debug prints, hardcoded values, warnings

---

## REFERENCE

- Roadmap: `ROADMAP_CURRENT.md` (Part 10 section)
- Grafana dashboard: `grafana/dashboards/grafana_quality_dashboard.json`
- E2E tests: `test-utils/tests/observability_pipeline.rs`
- Playwright tests: `test-utils/tests/grafana_dashboard.test.js`
- Prometheus config: `prometheus.yml`
- Alert rules: `monitoring/alert_rules.yml`
- Docker compose: `docker-compose.dashstream.yml`
