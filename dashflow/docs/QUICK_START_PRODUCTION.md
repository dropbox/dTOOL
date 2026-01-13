# Production Quick Start Guide

**DashFlow v1.11.3**

Get DashFlow running in production in 5 minutes.

---

## Prerequisites

- Docker 24.0+ OR Kubernetes 1.28+
- API keys for LLM providers (OpenAI, Anthropic, etc.)
- 5 minutes

---

## Option 1: Docker (Fastest)

### Step 1: Build the Image

```bash
# Clone repository
git clone <repository-url>
cd dashflow

# Build production image
docker build -t dashflow:latest .
```

**Build time:** ~2-3 minutes
**Image size:** ~20MB (vs 2GB for Python)

### Step 2: Run the Container

```bash
# Run with your API keys
docker run -d \
  --name dashflow \
  -p 8080:8080 \
  -e OPENAI_API_KEY=$OPENAI_API_KEY \
  -e ANTHROPIC_API_KEY=$ANTHROPIC_API_KEY \
  -e RUST_LOG=info \
  dashflow:latest
```

### Step 3: Verify

```bash
# Check health
curl http://localhost:8080/health

# Check metrics
curl http://localhost:8080/metrics

# View logs
docker logs -f dashflow
```

**Done!** Your production-ready DashFlow server is running.

---

## Option 2: Kubernetes (Production Scale)

### Step 1: Create Namespace and Secrets

```bash
# Create namespace
kubectl create namespace dashflow

# Create secrets (replace with your actual API keys)
kubectl create secret generic dashflow-secrets \
  --from-literal=openai-api-key="sk-..." \
  --from-literal=anthropic-api-key="sk-ant-..." \
  -n dashflow
```

### Step 2: Deploy Application

```bash
# Apply all manifests
kubectl apply -f k8s/configmap.yaml -n dashflow
kubectl apply -f k8s/deployment.yaml -n dashflow
kubectl apply -f k8s/ingress.yaml -n dashflow
kubectl apply -f k8s/monitoring.yaml -n dashflow
```

**What this deploys:**
- 3 replicas with auto-scaling (3-20 pods)
- Health checks and readiness probes
- Prometheus metrics collection
- Service mesh ready
- Security hardened (non-root, read-only fs)

### Step 3: Verify Deployment

```bash
# Check pods
kubectl get pods -n dashflow

# Check service
kubectl get svc -n dashflow

# View logs
kubectl logs -f deployment/dashflow -n dashflow

# Check metrics
kubectl port-forward svc/dashflow 9090:9090 -n dashflow
curl http://localhost:9090/metrics
```

### Step 4: Access the Service

```bash
# Via port-forward (development)
kubectl port-forward svc/dashflow 8080:80 -n dashflow

# Via ingress (production)
# Configure your DNS to point to the ingress controller
# Access at: https://dashflow.yourdomain.com
```

**Done!** Your production Kubernetes deployment is live.

---

## Option 3: Local Binary (Development)

### Step 1: Build

```bash
# Build release binary
cargo build --release --bin dashflow-server

# Binary location
ls -lh target/release/dashflow-server
# ~15-20MB
```

### Step 2: Run

```bash
# Set API keys
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
export RUST_LOG=info

# Run server
./target/release/dashflow-server serve
```

### Step 3: Verify

```bash
# Health check
curl http://localhost:8080/health

# Metrics
curl http://localhost:8080/metrics
```

**Done!** Local development server running.

---

## Production Features

### Observability

**Metrics (Prometheus):**
```bash
# Scrape endpoint
curl http://localhost:9090/metrics
```

**Available metrics:**
- `dashflow_requests_total` - Total requests by provider
- `dashflow_request_duration_seconds` - Request latency histogram
- `dashflow_tokens_total` - Token usage by provider
- `dashflow_errors_total` - Error count by type
- `dashflow_cache_hits_total` - Cache performance

**Distributed Tracing:**
- LangSmith integration built-in
- OpenTelemetry compatible
- See [DISTRIBUTED_TRACING.md](DISTRIBUTED_TRACING.md)

**Logging:**
```bash
# Set log level
export RUST_LOG=debug  # trace, debug, info, warn, error
```

### Performance

**Throughput:**
- 3.38M requests/second (338× Python)
- 25.6× median speedup
- <100ms startup time

**Resource Usage:**
- 128Mi memory (base)
- 100m CPU (base)
- Scales to 512Mi memory, 1 CPU under load

**Load Testing:**
```bash
# Install k6
# See load-tests/README.md

cd load-tests
k6 run scenarios/smoke-test.js
k6 run scenarios/load-test.js
```

### Security

**Built-in Security:**
- Non-root user (UID 1000)
- Read-only root filesystem
- No privilege escalation
- Dropped capabilities
- TLS/HTTPS ready

**Security Audit:**
- See [SECURITY_AUDIT.md](SECURITY_AUDIT.md)
- 2 known low-risk vulnerabilities (documented)
- No critical or high-severity issues
- Monthly review cycle

**Secrets Management:**
```bash
# Kubernetes secrets
kubectl create secret generic dashflow-secrets \
  --from-literal=openai-api-key="$OPENAI_API_KEY" \
  -n dashflow

# Environment variables
docker run -e OPENAI_API_KEY="$OPENAI_API_KEY" ...

# Never commit secrets to git!
```

### Scaling

**Horizontal Pod Autoscaling (HPA):**
- Min replicas: 3
- Max replicas: 20
- Target CPU: 70%
- Target memory: 80%
- Scale up: 100% every 30s (max)
- Scale down: 50% every 60s (gradual)

**Validated Scale:**
- 10M+ requests/day
- See [PRODUCTION_DEPLOYMENT.md](PRODUCTION_DEPLOYMENT.md) Load Testing section

### High Availability

**Health Checks:**
- Liveness probe: `/health` (30s interval)
- Readiness probe: `/ready` (10s interval)
- Startup probe: 5s initial delay

**Pod Disruption Budget:**
- Min available: 1
- Ensures zero-downtime deployments

**Graceful Shutdown:**
- 30s termination grace period
- Drains connections before shutdown

---

## Configuration

### Environment Variables

**LLM Providers:**
```bash
OPENAI_API_KEY          # OpenAI and Azure OpenAI
ANTHROPIC_API_KEY       # Anthropic Claude
COHERE_API_KEY          # Cohere
GROQ_API_KEY            # Groq
MISTRAL_API_KEY         # Mistral AI
FIREWORKS_API_KEY       # Fireworks AI
HUGGINGFACE_API_KEY     # HuggingFace Hub
```

**Application Settings:**
```bash
RUST_LOG                # Logging level (debug, info, warn, error)
LANGCHAIN_ENV           # Environment (development, production)
LANGCHAIN_PORT          # Server port (default: 8080)
LANGCHAIN_METRICS_PORT  # Metrics port (default: 9090)
```

**Observability:**
```bash
LANGSMITH_API_KEY       # LangSmith tracing
LANGSMITH_PROJECT       # LangSmith project name
OTEL_EXPORTER_OTLP_ENDPOINT  # OpenTelemetry endpoint
```

### ConfigMap (Kubernetes)

```bash
# Edit configuration
kubectl edit configmap dashflow-config -n dashflow

# Apply changes (pods restart automatically)
kubectl rollout restart deployment/dashflow -n dashflow
```

---

## Troubleshooting

### Container won't start

```bash
# Check logs
docker logs dashflow

# Common issues:
# - Missing API key: Set OPENAI_API_KEY or other required keys
# - Port already in use: Use -p 8081:8080 instead
# - Build failed: Ensure you have Docker 24.0+
```

### Kubernetes pods not ready

```bash
# Check pod status
kubectl describe pod <pod-name> -n dashflow

# Check events
kubectl get events -n dashflow --sort-by='.lastTimestamp'

# Common issues:
# - ImagePullBackOff: Build and push image to registry
# - CrashLoopBackOff: Check logs with kubectl logs
# - Secret not found: Create dashflow-secrets first
```

### High memory usage

```bash
# Check metrics
kubectl top pods -n dashflow

# Increase limits
kubectl edit deployment dashflow -n dashflow
# Update resources.limits.memory

# Or reduce load
kubectl scale deployment dashflow --replicas=5 -n dashflow
```

### Slow responses

```bash
# Check Prometheus metrics
curl http://localhost:9090/metrics | grep duration

# Check if rate limited by LLM provider
kubectl logs deployment/dashflow -n dashflow | grep -i "rate"

# Scale up
kubectl scale deployment dashflow --replicas=10 -n dashflow
```

---

## Next Steps

### Production Hardening

1. **Enable TLS/HTTPS:**
   - Install cert-manager
   - Configure ingress with TLS
   - See [PRODUCTION_DEPLOYMENT.md](PRODUCTION_DEPLOYMENT.md#security)

2. **Set up Monitoring:**
   - Deploy Prometheus + Grafana
   - Import dashboard from `k8s/monitoring.yaml`
   - Configure alerts
   - See [PRODUCTION_DEPLOYMENT.md](PRODUCTION_DEPLOYMENT.md#monitoring--observability)

3. **Configure CI/CD:**
   - Set up automated builds
   - Run tests in pipeline
   - Security scanning
   - See [CI/CD Integration docs](PRODUCTION_DEPLOYMENT.md#cicd-integration)

4. **Load Testing:**
   - Run k6 load tests
   - Validate scale targets
   - See `load-tests/README.md`

5. **Backup and Disaster Recovery:**
   - Document runbooks
   - Set up monitoring alerts
   - Practice incident response
   - See [PRODUCTION_RUNBOOK.md](PRODUCTION_RUNBOOK.md)

### Development Workflow

1. **Local Development:**
   - Use `cargo run` for fast iteration
   - Run tests: `cargo test --workspace`
   - See [examples/](../examples/) for code samples

2. **Testing:**
   - Unit tests: `cargo test --lib`
   - Integration tests: `cargo test --test '*'`
   - See [TEST_PHILOSOPHY.md](TEST_PHILOSOPHY.md)

3. **Performance Optimization:**
   - Run benchmarks: `cargo bench`
   - Profile with flamegraph
   - See [BEST_PRACTICES.md](BEST_PRACTICES.md)

### Migration from Python

- **Read first:** [Golden Path Guide](GOLDEN_PATH.md) for recommended API patterns
- **Architecture:** See [Architecture Design](ARCHITECTURE.md) for system design
- **Common patterns:** Error handling, async/await, serialization
- **Examples:** Check `examples/` directory for working code

---

## Resources

**Documentation:**
- [Production Deployment Guide](PRODUCTION_DEPLOYMENT.md) - Comprehensive deployment docs
- [Production Runbook](PRODUCTION_RUNBOOK.md) - Operational procedures
- [Golden Path Guide](GOLDEN_PATH.md) - Recommended API patterns
- [Security Audit](SECURITY_AUDIT.md) - Security assessment
- [Security Advisories](SECURITY_ADVISORIES.md) - Security status

**Examples:**
- [examples/](../examples/) - 57 working code examples
- [Quick examples](../README.md#quick-start) - Basic usage

**Support:**
- Check documentation first
- Search GitHub issues
- Review test files for usage patterns

---

## Summary

**You now have:**
- ✅ Production-ready DashFlow server
- ✅ Docker container (<20MB)
- ✅ Kubernetes manifests (auto-scaling, monitoring)
- ✅ Prometheus metrics and health checks
- ✅ Security hardened configuration
- ✅ 25.6× faster than Python

**Performance validated:**
- 10M+ requests/day
- 3.38M requests/second throughput
- <100ms startup time
- 100× smaller container (20MB vs 2GB)

**Production-ready checklist:**
- ✅ 16,600+ tests passing (100% pass rate)
- ✅ Security audit complete
- ✅ Observability built-in
- ✅ Load testing validated
- ✅ CI/CD integration ready
- ✅ Comprehensive documentation

**Get started in 5 minutes with Docker or Kubernetes.**

---

**Last Updated:** 2026-01-04
**Version:** 1.11.3
**Status:** Production-ready
