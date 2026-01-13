# Manager: Evals & Quality Assurance Options (No Conflict with Observability Worker)

**Date:** November 16, 2025
**User Interest:** Evals and Quality Assurance
**Constraint:** Another worker building observability dashboards/visualization

---

## Division of Labor

### Other Worker (observability/real-time-dashboards branch)
**Focus:** VISUALIZATION & INFRASTRUCTURE
- ✅ React frontend dashboard
- ✅ Kafka stream processor
- ✅ Grafana dashboards
- ✅ Docker infrastructure (Kafka, TimescaleDB, Prometheus)
- ✅ Web UI for live quality monitoring
- ✅ Chat viewer (real-time conversations)

**Status:** 25 commits (N=19-25), infrastructure complete

### Your Focus (Evals & Quality)
**Focus:** DATA GENERATION & EVALUATION LOGIC
- Quality evaluation algorithms
- Golden datasets and test data
- Regression detection logic
- Eval metrics and scoring
- Test automation framework
- Quality gates and thresholds

**Status:** Ready to start (no conflicts with visualization work)

---

## RECOMMENDED OPTIONS (Conflict-Free)

### **Option 1: Golden Dataset & Regression Testing Framework** 🎯 HIGH PRIORITY

**What it is:** Backend data layer for quality testing

**Components:**
1. **Golden Dataset Repository**
   - Storage format for test scenarios (queries + expected outputs)
   - Dataset versioning (v1, v2, etc.)
   - Management CLI (add, update, list, validate)

2. **Regression Detection Engine**
   - Run agents against golden data
   - Compare outputs to expected
   - LLM-as-judge scoring for semantic similarity
   - Pass/fail thresholds

3. **CI/CD Integration**
   - Automated regression tests on PR
   - Block merges on quality drops
   - Performance regression detection

4. **Eval Report Generation**
   - Which scenarios passed/failed
   - Quality score trends
   - Detailed diff reports

**No Conflict:** Generates DATA that feeds into other worker's dashboards
**Location:** `crates/dashflow-evals/` (new crate)
**Estimated:** 15-20 commits
**Impact:** HIGH - Prevents quality regressions

**Deliverables:**
- `dashflow-evals` crate with golden dataset framework
- CLI tool: `cargo run --bin eval-runner -- --dataset golden_v1.json`
- CI integration: GitHub Actions workflow
- Report format that can be consumed by dashboards

---

### **Option 2: Advanced Quality Metrics & Scoring** 🔬 MEDIUM PRIORITY

**What it is:** Sophisticated evaluation logic

**Components:**
1. **Multi-Dimensional Quality Scoring**
   - Correctness (factual accuracy)
   - Relevance (answers the question)
   - Completeness (covers all aspects)
   - Safety (no harmful content)
   - Coherence (logical flow)
   - Conciseness (not verbose)

2. **Explainable Quality Scores**
   - Why did response get 0.85?
   - Which dimensions were weak?
   - Specific improvement suggestions

3. **Calibrated Confidence Scores**
   - Align LLM confidence with actual accuracy
   - Confidence calibration curves
   - Uncertainty quantification

4. **Comparative Evaluation**
   - A/B testing framework
   - Statistical significance tests
   - Which agent/prompt/model is better?

**No Conflict:** Evaluation LOGIC, not visualization
**Location:** `crates/dashflow/src/quality/` (extend existing)
**Estimated:** 15-18 commits
**Impact:** MEDIUM-HIGH - Better quality insights

**Deliverables:**
- Multi-dimensional judge implementation
- Calibration framework
- A/B testing statistical tools
- Outputs JSON that dashboards can consume

---

### **Option 3: Automated Eval Pipeline (DashFlow Streaming Integration)** 🚀 MEDIUM PRIORITY

**What it is:** Backend evaluation infrastructure

**Components:**
1. **Eval Runner Service**
   - Kafka consumer: reads from dashstream-events
   - Triggers quality evaluation on graph completions
   - Publishes results to dashstream-quality topic
   - Runs continuously in background

2. **Eval Scheduler**
   - Periodic regression testing (hourly, daily)
   - Ad-hoc eval triggers (on-demand testing)
   - Batch evaluation mode (test many scenarios)

3. **Eval Results Store**
   - TimescaleDB storage for eval history
   - Query API for historical comparisons
   - Retention policies (keep 90 days)

4. **Alert Logic**
   - Quality drop detection (threshold-based)
   - Anomaly detection (statistical outliers)
   - Slack/Email notification triggers

**No Conflict:** Backend services, not frontend
**Location:** `crates/dashflow-observability/src/bin/eval_runner.rs`
**Estimated:** 20-25 commits
**Impact:** HIGH - Continuous quality monitoring

**Deliverables:**
- eval_runner binary (Rust service)
- Integration with existing Kafka topics
- Alert generation (consumed by other worker's alert dashboard)

---

### **Option 4: Test Data Generation & Synthetic Scenarios** 📊 LOW-MEDIUM PRIORITY

**What it is:** Automated test case generation

**Components:**
1. **Synthetic Query Generator**
   - Generate diverse test queries automatically
   - Coverage: simple, complex, edge cases, adversarial
   - Parameterized templates
   - Domain-specific generators (customer support, research, troubleshooting)

2. **Expected Output Generator**
   - Use high-quality model (GPT-4) to generate golden outputs
   - Human-in-loop review
   - Version control for datasets

3. **Adversarial Test Generator**
   - Prompt injection attempts
   - Jailbreak scenarios
   - Out-of-distribution inputs
   - Stress tests (very long queries, malformed input)

4. **Coverage Analysis**
   - Which scenarios are well-tested?
   - Which edge cases are missing?
   - Diversity metrics for test suites

**No Conflict:** Data generation, not visualization
**Location:** `crates/dashflow-evals/src/generators/`
**Estimated:** 12-15 commits
**Impact:** MEDIUM - Better test coverage

---

### **Option 5: Quality Gate Integration (Production Guardrails)** 🛡️ HIGH PRIORITY

**What it is:** Runtime quality enforcement

**Components:**
1. **Pre-Production Quality Gates**
   - Before deploying: run against golden data
   - Must achieve ≥X% quality score
   - Must pass all regression tests
   - Block deployment if quality drops

2. **Runtime Quality Gates**
   - Monitor quality in production
   - Automatically fall back to safer model if quality drops
   - Circuit breaker pattern (stop using failing agent)
   - Gradual rollout based on quality metrics

3. **Quality SLOs (Service Level Objectives)**
   - Define: 95% of responses must have quality ≥0.90
   - Track compliance
   - Alert on SLO violations
   - Monthly/quarterly SLO reports

4. **Canary Deployment Framework**
   - Deploy new version to 5% of traffic
   - Compare quality to production version
   - Auto-rollback if quality degrades
   - Statistical significance testing

**No Conflict:** Deployment logic, feeds data to dashboards
**Location:** `crates/dashflow-deployment/` (new crate)
**Estimated:** 18-22 commits
**Impact:** HIGH - Production safety

---

## RECOMMENDED SEQUENCE

### Phase 1: Foundation (Option 1) - 15-20 commits
**Golden Dataset & Regression Testing**
- Build the data layer for evals
- Create test scenarios and expected outputs
- Regression detection algorithms
- CI integration

### Phase 2: Enhanced Scoring (Option 2) - 15-18 commits
**Advanced Quality Metrics**
- Multi-dimensional scoring
- Explainable scores
- Calibration and A/B testing

### Phase 3: Production Integration (Options 3 & 5) - 25-30 commits
**Automated Eval Pipeline + Quality Gates**
- Continuous evaluation service
- Quality SLOs and alerts
- Canary deployments

**Total: 55-68 commits, 5-7 days**

---

## Clear Boundaries with Observability Worker

**You Build (Backend Logic):**
- Evaluation algorithms
- Scoring logic
- Test data management
- Regression detection
- Alert TRIGGERS (logic that decides when to alert)

**They Build (Frontend/Infrastructure):**
- Web dashboards
- React components
- Grafana visualization
- Kafka consumers for display
- Alert DISPLAY (showing alerts in UI)

**Integration Points:**
- You publish to Kafka topics (`dashstream-quality`, `dashstream-evals`)
- They consume and visualize
- You define JSON schema, they render it
- Clean separation of concerns

---

## Next Steps

**If you choose Option 1 (Recommended):**

1. I create detailed implementation plan
2. Worker creates `crates/dashflow-evals/` crate
3. Design golden dataset format (JSON schema)
4. Implement dataset management CLI
5. Build regression detection engine
6. Add CI integration
7. First eval: Run against document_search app

**Deliverables after Phase 1:**
- Golden dataset with 50-100 test scenarios
- Automated regression testing
- CI that blocks bad PRs
- Eval reports (JSON) that observability worker can visualize

**User: Choose Option 1 to start?**
