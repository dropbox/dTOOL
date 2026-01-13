# Production Deployment Guide

**Last Updated:** 2026-01-04 (Worker #2450 - Metadata sync)

**DashFlow v1.11.3**

This guide covers deploying DashFlow to production environments with Kubernetes.

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Quick Start](#quick-start)
3. [Docker Deployment](#docker-deployment)
4. [Kubernetes Deployment](#kubernetes-deployment)
5. [Configuration](#configuration)
6. [Monitoring & Observability](#monitoring--observability)
7. [Performance Tuning](#performance-tuning)
8. [Security](#security)
9. [Load Testing](#load-testing)
10. [CI/CD Integration](#cicd-integration)
11. [Troubleshooting](#troubleshooting)

---

## Prerequisites

### Required
- Docker 24.0+
- Kubernetes 1.28+
- kubectl configured with cluster access
- Helm 3.0+ (for monitoring stack)

### Optional but Recommended
- Prometheus Operator (for metrics)
- Grafana (for dashboards)
- Cert-manager (for TLS certificates)
- Nginx Ingress Controller

---

## Quick Start

### 1. Build the Docker Image

```bash
# From repository root
docker build -t dashflow:v1.11.3 .
```

**Image size:** ~20MB (vs 2GB for Python container)

### 2. Test Locally

```bash
# Run with Docker
docker run -p 8080:8080 \
  -e OPENAI_API_KEY=$OPENAI_API_KEY \
  dashflow:v1.11.3
```

### 3. Deploy to Kubernetes

```bash
# Create namespace
kubectl create namespace dashflow

# Create secrets (see Configuration section)
kubectl apply -f k8s/secrets.yaml -n dashflow

# Deploy application
kubectl apply -f k8s/configmap.yaml -n dashflow
kubectl apply -f k8s/deployment.yaml -n dashflow
kubectl apply -f k8s/ingress.yaml -n dashflow

# Verify deployment
kubectl get pods -n dashflow
```

---

## Docker Deployment

### Building for Production

The Dockerfile uses multi-stage builds to create minimal production images:

**Build stages:**
1. **Builder stage:** Compiles Rust code with optimizations
2. **Runtime stage:** Minimal Debian slim image with only runtime dependencies

**Optimizations:**
- Static linking where possible
- Stripped binaries
- No development dependencies
- Non-root user (UID 1000)
- Read-only root filesystem support

### Image Tags

```bash
# Production release
docker build -t dashflow:v1.11.3 .
docker tag dashflow:v1.11.3 your-registry/dashflow:v1.11.3

# Latest tag (for development)
docker tag dashflow:v1.11.3 your-registry/dashflow:latest

# Push to registry
docker push your-registry/dashflow:v1.11.3
docker push your-registry/dashflow:latest
```

### Docker Compose

For local testing with dependencies:

```yaml
version: '3.8'
services:
  dashflow:
    image: dashflow:v1.11.3
    ports:
      - "8080:8080"
      - "9090:9090"
    environment:
      - RUST_LOG=info
      - OPENAI_API_KEY=${OPENAI_API_KEY}
    restart: unless-stopped
```

---

## Kubernetes Deployment

### Architecture

**Components:**
- **Deployment:** 3 replicas (scales 3-20 with HPA)
- **Service:** ClusterIP with HTTP (80) and metrics (9090) ports
- **Ingress:** HTTPS with TLS termination
- **HPA:** Auto-scaling based on CPU/memory
- **PDB:** Ensures at least 1 pod during disruptions

### Deployment Steps

#### 1. Configure Secrets

```bash
# Copy example secrets
cp k8s/secrets.yaml.example k8s/secrets.yaml

# Edit with your API keys
vim k8s/secrets.yaml

# Apply secrets
kubectl apply -f k8s/secrets.yaml -n dashflow
```

**Security note:** Never commit `k8s/secrets.yaml` to git. Use a secrets management solution for production.

#### 2. Apply Configuration

```bash
# ConfigMap (environment variables)
kubectl apply -f k8s/configmap.yaml -n dashflow

# Deployment, Service, HPA, PDB
kubectl apply -f k8s/deployment.yaml -n dashflow

# Ingress (update hostname first)
kubectl apply -f k8s/ingress.yaml -n dashflow
```

#### 3. Configure Monitoring (Optional)

```bash
# Requires Prometheus Operator
kubectl apply -f k8s/monitoring.yaml -n dashflow
```

#### 4. Verify Deployment

```bash
# Check pods
kubectl get pods -n dashflow

# Check service
kubectl get svc -n dashflow

# Check ingress
kubectl get ingress -n dashflow

# View logs
kubectl logs -f deployment/dashflow -n dashflow

# Check metrics
kubectl port-forward svc/dashflow 9090:9090 -n dashflow
curl http://localhost:9090/metrics
```

### Resource Requirements

**Per pod:**
- **Requests:** 100m CPU, 128Mi memory
- **Limits:** 1000m CPU, 512Mi memory

**Expected usage:**
- Idle: 10-20Mi memory, 5-10m CPU
- Load (1000 req/s): 100-200Mi memory, 200-400m CPU

**Scaling capacity:**
- 3 pods: ~3,000 req/s
- 10 pods: ~10,000 req/s
- 20 pods (max): ~20,000 req/s

---

## Configuration

### Environment Variables

**Core settings:**
```bash
RUST_LOG=info                    # Log level (error, warn, info, debug, trace)
LANGCHAIN_ENV=production         # Environment name
TOKIO_WORKER_THREADS=4           # Async runtime threads
```

**API Keys:**
```bash
OPENAI_API_KEY=sk-...           # OpenAI API key
ANTHROPIC_API_KEY=sk-ant-...    # Anthropic API key
COHERE_API_KEY=...              # Cohere API key (optional)
MISTRAL_API_KEY=...             # Mistral API key (optional)
```

**Performance:**
```bash
MAX_CONCURRENT_REQUESTS=1000    # Max concurrent requests
REQUEST_TIMEOUT=60              # Request timeout (seconds)
LLM_TIMEOUT=30                  # LLM call timeout (seconds)
MAX_RETRIES=3                   # Retry attempts
RETRY_BACKOFF_MS=1000           # Retry backoff (milliseconds)
```

**Observability:**
```bash
ENABLE_METRICS=true             # Prometheus metrics
ENABLE_HEALTH_CHECKS=true       # Health/readiness endpoints
LANGSMITH_TRACING=false         # LangSmith tracing
LANGSMITH_PROJECT=my-project    # LangSmith project name
LANGSMITH_API_KEY=...           # LangSmith API key
```

### ConfigMap vs Secrets

**ConfigMap** (`k8s/configmap.yaml`):
- Non-sensitive configuration
- Environment variables
- Feature flags
- Performance tuning

**Secrets** (`k8s/secrets.yaml`):
- API keys
- Credentials
- TLS certificates
- Database passwords

---

## Monitoring & Observability

**📖 Comprehensive Guide:** See [Observability Guide](OBSERVABILITY.md) for detailed information on metrics, tracing, dashboards, and alerts.

### Quick Reference

#### Health Checks

**Liveness probe:** `/health`
- Checks if the application is alive
- Restarts pod if check fails
- Timeout: 3s, Interval: 30s

**Readiness probe:** `/ready`
- Checks if the application can serve traffic
- Removes pod from service if check fails
- Timeout: 3s, Interval: 10s

#### Metrics

**Prometheus endpoint:** `/metrics` (port 9090)

**Key metrics:**
- `dashflow_requests_total` - Total requests by endpoint and status
- `dashflow_request_duration_seconds` - Request latency histogram
- `dashflow_batch_size` - Batch request sizes
- `dashflow_stream_chunks` - Chunks per streaming request
- `dashflow_errors_total` - Errors by type and endpoint

### Alerts

The monitoring configuration includes alerts for:
- High error rate (>5%)
- High latency (P95 >1s)
- High memory usage (>90%)
- High CPU usage (>90%)
- Pod not ready
- Frequent restarts

#### Grafana Dashboard

**Pre-built dashboard:** Import `observability/grafana-dashboard.json` into Grafana

**Included panels:**
1. Request rate by endpoint (req/s)
2. Error rate (%)
3. Request latency percentiles (P50, P95, P99)
4. Batch request size distribution
5. Stream chunks per request
6. Errors by type and endpoint

**Dashboard features:**
- Auto-refresh every 10 seconds
- Time range configurable (default: last 1 hour)
- Color-coded thresholds (green/yellow/red)
- Mean, max, and P95 legends

#### Logs

**Structured logging with tracing:**

DashFlow uses the `tracing` crate for structured logging with span context.

```bash
# Set log level
export RUST_LOG=info                    # Default
export RUST_LOG=debug                   # Verbose
export RUST_LOG=dashflow_langserve=debug,dashflow_core=info  # Module-specific

# View logs in Kubernetes
kubectl logs -f deployment/dashflow -n dashflow

# Filter by level
kubectl logs deployment/dashflow -n dashflow | grep ERROR

# Tail last 100 lines
kubectl logs --tail=100 deployment/dashflow -n dashflow
```

**Example logs:**
```
INFO Processing invoke request base_path=/my_runnable
INFO Invoke request completed successfully in 0.045s
ERROR Invoke request failed: execution error: model timeout
```

**Log aggregation:** Configure Fluentd/Fluent Bit to ship logs to:
- Elasticsearch + Kibana
- Loki + Grafana
- CloudWatch Logs
- Datadog

See [Observability Guide](OBSERVABILITY.md) for detailed logging configuration.

---

## Performance Tuning

### Concurrency

**Tokio worker threads:**
```bash
# Set based on CPU cores
TOKIO_WORKER_THREADS=4  # 4 cores
TOKIO_WORKER_THREADS=8  # 8 cores
```

**Max concurrent requests:**
```bash
MAX_CONCURRENT_REQUESTS=1000  # Conservative
MAX_CONCURRENT_REQUESTS=5000  # Aggressive
```

### Memory

**Heap size:** No explicit limit (Rust uses minimal heap)

**Expected memory per pod:**
- Baseline: 10-20 MB
- Under load: 100-200 MB
- Max (with limits): 512 MB

**Memory efficiency:** 88× less than Python (6 MB vs 530 MB for 2000 operations)

### CPU

**CPU allocation:**
- 1 core: ~1,000 req/s
- 2 cores: ~2,000 req/s
- 4 cores: ~4,000 req/s

**Note:** Most latency is from LLM API calls, not CPU.

### Network

**Connection pooling:** Automatic via `reqwest`
**Keep-alive:** Enabled by default
**HTTP/2:** Enabled for supported APIs

### Database Connections

For vector stores and SQL databases:
```bash
# PostgreSQL example
DATABASE_MAX_CONNECTIONS=10
DATABASE_MIN_CONNECTIONS=2
DATABASE_CONNECT_TIMEOUT=5
```

### Horizontal Scaling

**HPA configuration:**
- Min replicas: 3
- Max replicas: 20
- Target CPU: 70%
- Target memory: 80%

**Scaling behavior:**
- Scale up: +100% or +4 pods (max) every 30s
- Scale down: -50% every 60s after 5min stabilization

---

## Security

### Container Security

**Non-root user:**
- User ID: 1000
- Group ID: 1000
- No privilege escalation

**Read-only root filesystem:**
```yaml
securityContext:
  readOnlyRootFilesystem: true
```

**Dropped capabilities:**
```yaml
capabilities:
  drop:
  - ALL
```

### Network Security

**TLS/HTTPS:**
- Ingress terminates TLS
- Cert-manager for automatic certificate renewal
- Redirect HTTP to HTTPS

**Rate limiting:**
```yaml
nginx.ingress.kubernetes.io/limit-rps: "100"
nginx.ingress.kubernetes.io/limit-connections: "10"
```

**Network policies:** (Optional)
```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: dashflow
spec:
  podSelector:
    matchLabels:
      app: dashflow
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: ingress-nginx
  egress:
  - to:
    - namespaceSelector: {}
```

### Secrets Management

**Best practices:**
1. Never commit secrets to git
2. Use Kubernetes secrets or external secrets operator
3. Rotate API keys regularly
4. Use RBAC to restrict secret access
5. Consider HashiCorp Vault or AWS Secrets Manager

### API Key Security

**Environment variables:**
- API keys loaded from Kubernetes secrets
- Not logged or exposed in metrics
- Automatically redacted in error messages

---

## Load Testing

Before deploying to production, validate performance with load tests.

### Quick Start

```bash
# Install k6
brew install k6  # macOS

# Run automated load test (starts server, runs test, cleanup)
./scripts/run-load-test.sh

# Run 10M/day scale test (3.5 hours)
./scripts/run-load-test.sh 10m-requests-day
```

### Test Against Kubernetes

```bash
# Deploy to K8s
kubectl apply -f k8s/

# Run load test against K8s deployment
./scripts/run-load-test.sh -k basic-invoke
```

### Performance Baseline

Establish performance baseline before production:

```bash
# Start server
cargo run --release --example basic_skeleton -p dashflow-langserve &

# Run baseline test suite (25 minutes)
./scripts/performance-baseline.sh

# View results
cat load-tests/results/baseline_*_summary.txt
```

### Available Test Scenarios

| Scenario | Duration | Purpose |
|----------|----------|---------|
| `basic-invoke` | 5 min | Test `/invoke` endpoint |
| `streaming` | 5 min | Test `/stream` endpoint |
| `batch` | 7 min | Test `/batch` endpoint |
| `mixed-workload` | 10 min | Realistic traffic mix |
| `10m-requests-day` | 3.5 hours | 10M+ req/day validation |

### Success Criteria

**Must pass before production:**
- ✅ Error rate < 0.1%
- ✅ P95 latency < 200ms
- ✅ P99 latency < 500ms
- ✅ No memory leaks during soak test
- ✅ Handles 10M+ requests/day (116 req/s sustained)

### Full Documentation

Load testing is performed using k6 and documented in the monitoring configuration.

---

## Troubleshooting

### Pod Won't Start

```bash
# Check pod status
kubectl describe pod <pod-name> -n dashflow

# Check logs
kubectl logs <pod-name> -n dashflow

# Common issues:
# - Image pull errors (check registry access)
# - Secret not found (apply secrets.yaml)
# - Resource limits (check node capacity)
```

### High Error Rate

```bash
# Check error logs
kubectl logs deployment/dashflow -n dashflow | grep ERROR

# Check metrics
kubectl port-forward svc/dashflow 9090:9090 -n dashflow
curl http://localhost:9090/metrics | grep dashflow_requests_total

# Common causes:
# - Invalid API keys (check secrets)
# - LLM API rate limits (increase retries/backoff)
# - Network issues (check egress)
```

### High Latency

```bash
# Check P95/P99 latency
curl http://localhost:9090/metrics | grep dashflow_request_duration

# Check LLM latency
curl http://localhost:9090/metrics | grep dashflow_llm_duration

# Common causes:
# - LLM API slowness (provider-side)
# - Resource constraints (increase CPU/memory)
# - High concurrent load (scale horizontally)
```

### Memory Issues

```bash
# Check memory usage
kubectl top pods -n dashflow

# Check for OOMKilled
kubectl describe pod <pod-name> -n dashflow | grep OOMKilled

# Solutions:
# - Increase memory limits
# - Reduce concurrent requests
# - Check for memory leaks (report issue)
```

### Scaling Issues

```bash
# Check HPA status
kubectl get hpa -n dashflow

# Check metrics server
kubectl top nodes
kubectl top pods -n dashflow

# Common issues:
# - Metrics server not installed
# - Resource requests not set
# - Node capacity exceeded
```

---

## Production Checklist

Before going to production:

- [ ] Build Docker image with release profile
- [ ] Push image to production registry
- [ ] Create namespace and RBAC policies
- [ ] Configure secrets (API keys, credentials)
- [ ] Apply ConfigMap with production settings
- [ ] Deploy application with 3+ replicas
- [ ] Configure ingress with TLS certificates
- [ ] Set up monitoring (Prometheus, Grafana)
- [ ] Configure alerts (PagerDuty, Slack)
- [ ] Test health checks and auto-scaling
- [ ] Set up log aggregation
- [ ] Document runbooks for common issues
- [ ] Test disaster recovery procedures
- [ ] Performance test at expected load
- [ ] Security audit (secrets, RBAC, network)
- [ ] Set up CI/CD pipeline
- [ ] Plan rollback strategy

---

## Performance Targets

Based on verified benchmarks:

**Throughput:**
- Single pod: ~1,000 req/s
- 10 pods: ~10,000 req/s
- 20 pods: ~20,000 req/s

**Latency:**
- P50: <10ms (internal operations)
- P95: <50ms (internal operations)
- P99: <100ms (internal operations)
- Note: LLM calls add 500-5000ms depending on provider

**Memory:**
- Idle: 10-20 MB per pod
- Load: 100-200 MB per pod
- Max: 512 MB per pod (limit)

**Concurrency:**
- 3.38M req/s theoretical (benchmarked)
- 338× better than Python

---

## CI/CD Integration

### Automated Deployment Pipeline

**Note:** DashFlow uses internal Dropbox CI systems. The patterns below are templates for external deployments and can be adapted to your CI/CD platform (GitHub Actions, GitLab CI, Jenkins, etc.).

**Pipeline stages:**
1. **Build & Test** - Automated testing, linting, formatting on every push
2. **Security Scanning** - Dependency audits, secret scanning, vulnerability detection
3. **Performance Testing** - Benchmark regression detection, load testing
4. **Docker Build** - Multi-architecture image builds (AMD64, ARM64, ARMv7)
5. **Deployment** - Automated push to container registry

**Workflow triggers:**
- Push to `main` - Full pipeline with deployment
- Pull requests - Build, test, and security scans (no deployment)
- Tags (`v*`) - Release builds with multi-arch images
- Daily schedule - Security scans and performance baselines

### Key Features

**Code Quality:**
- ✅ Automated test suite (16,600+ tests)
- ✅ Clippy linting with deny warnings
- ✅ rustfmt formatting checks
- ✅ Documentation validation
- ✅ Example compilation verification

**Security:**
- ✅ Dependency vulnerability scanning (cargo-audit)
- ✅ License compliance checking (cargo-deny)
- ✅ Secret detection (Gitleaks)
- ✅ Static analysis (CodeQL, Semgrep)
- ✅ Docker image scanning (Trivy)
- ✅ Unsafe code detection

**Performance:**
- ✅ Criterion benchmarks with regression detection
- ✅ k6 load tests (smoke, load, stress, spike)
- ✅ Memory profiling (Valgrind)
- ✅ Performance threshold enforcement (P95 < 100ms, error rate < 1%)

**Artifacts:**
- Release binaries (7 day retention)
- Docker images pushed to ghcr.io
- Code coverage reports (Codecov)
- Performance benchmark history
- Security scan results

### Docker Images

**Registry:** GitHub Container Registry (ghcr.io)

**Available tags:**
```bash
# Latest from main branch
ghcr.io/<owner>/<repo>:latest

# Specific version
ghcr.io/<owner>/<repo>:v1.11.3

# Major.minor version
ghcr.io/<owner>/<repo>:1.11

# Branch-specific
ghcr.io/<owner>/<repo>:main
ghcr.io/<owner>/<repo>:all-to-rust2

# Commit-specific
ghcr.io/<owner>/<repo>:main-abc1234
```

**Platforms supported:**
- linux/amd64 (Intel/AMD 64-bit)
- linux/arm64 (ARM 64-bit, Apple Silicon, AWS Graviton)
- linux/arm/v7 (ARM 32-bit, Raspberry Pi)

**Pulling images:**
```bash
# Authenticate with GitHub
echo $GITHUB_TOKEN | docker login ghcr.io -u USERNAME --password-stdin

# Pull latest
docker pull ghcr.io/<owner>/<repo>:latest

# Pull specific version
docker pull ghcr.io/<owner>/<repo>:v1.11.3
```

### Integration with Kubernetes

**Using CI/CD images in deployments:**

```yaml
# k8s/deployment.yaml
spec:
  template:
    spec:
      containers:
      - name: dashflow
        image: ghcr.io/<owner>/<repo>:v1.11.3  # Use specific version
        imagePullPolicy: IfNotPresent
      imagePullSecrets:
      - name: ghcr-secret  # For private repos
```

**Create image pull secret:**
```bash
kubectl create secret docker-registry ghcr-secret \
  --docker-server=ghcr.io \
  --docker-username=<github-username> \
  --docker-password=<github-token> \
  --namespace=dashflow
```

### Continuous Deployment Strategy

**Recommended approach:**

1. **Development** - Push to feature branch triggers CI checks
2. **Pull Request** - Creates review environment (optional)
3. **Merge to Main** - Builds and pushes `:latest` tag
4. **Tag Release** - Creates multi-arch builds, pushes versioned tags
5. **Deploy** - Use versioned tags in production (not `:latest`)

**GitOps workflow (recommended):**

1. CI pipeline builds and pushes image with version tag
2. Update Kubernetes manifests in separate repo with new tag
3. ArgoCD/Flux detects manifest change and deploys automatically
4. Gradual rollout using canary or blue-green deployment

**Example GitOps manifest:**
```yaml
# gitops-repo/production/dashflow.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dashflow
spec:
  template:
    spec:
      containers:
      - name: dashflow
        image: ghcr.io/<owner>/<repo>:v1.11.3  # Updated by CI
```

### Monitoring CI/CD Pipeline

**CI/CD dashboard (adapt to your platform):**
- View workflow/job status in your CI provider's UI
- Download build logs and artifacts
- Track test results and code coverage

**Performance trends:**
- Benchmark results tracked automatically
- Alerts on >30% regression
- Historical data viewable in CI artifacts

**Security alerts:**
- Dependency vulnerability notifications
- Automatic PRs for security updates (if enabled)
- Static analysis results from configured scanners

### Complete CI/CD Documentation

---

## Next Steps

1. **Review configuration** - Adjust replicas, resources, timeouts
2. **Set up monitoring** - Deploy Prometheus, Grafana, alerting
3. **Performance test** - Load test at expected traffic
4. **Gradual rollout** - Start with 10% traffic, increase slowly
5. **Monitor metrics** - Watch error rate, latency, resource usage
6. **Optimize** - Tune based on production metrics

---

## Support

**Issues:** Report issues at your issue tracker
**Documentation:** See `docs/` directory
**Examples:** See `examples/` directory
**Tests:** See `crates/*/tests/` directories

---

**Last updated:** 2025-12-08
**Version:** 1.11.3

© 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
