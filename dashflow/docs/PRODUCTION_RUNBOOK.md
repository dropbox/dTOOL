# Production Runbook

**Version**: 1.0
**Last Updated:** 2026-01-03 (Worker #2425 - Fix stale Last Updated date)
**Owner**: SRE/DevOps Team
**Review Cycle**: Quarterly

This runbook provides operational procedures for running DashFlow in production. It covers incident response, common failure scenarios, rollback procedures, scaling operations, and on-call escalation.

---

## Table of Contents

1. [Quick Reference](#quick-reference)
2. [Incident Response Framework](#incident-response-framework)
3. [Common Failure Scenarios](#common-failure-scenarios)
4. [Rollback Procedures](#rollback-procedures)
5. [Scaling Operations](#scaling-operations)
6. [Dependency Outage Handling](#dependency-outage-handling)
7. [Database and Vector Store Recovery](#database-and-vector-store-recovery)
8. [Performance Degradation](#performance-degradation)
9. [Security Incidents](#security-incidents)
10. [On-Call Escalation](#on-call-escalation)
11. [Maintenance Windows](#maintenance-windows)
12. [Post-Incident Review](#post-incident-review)

---

## Quick Reference

### Critical Commands

```bash
# Check deployment status
kubectl get deployment dashflow -n dashflow

# View logs (last 100 lines)
kubectl logs -n dashflow deployment/dashflow --tail=100 --follow

# Check pod health
kubectl get pods -n dashflow -l app=dashflow
kubectl describe pod <pod-name> -n dashflow

# View metrics
curl http://localhost:8080/metrics

# Health check
curl http://localhost:8080/health

# Scale deployment
kubectl scale deployment dashflow -n dashflow --replicas=5

# Rollback to previous version
kubectl rollout undo deployment/dashflow -n dashflow

# Check rollout status
kubectl rollout status deployment/dashflow -n dashflow
```

### Key Endpoints

- **Health Check**: `GET /health`
- **Readiness Check**: `GET /ready`
- **Metrics**: `GET /metrics`
- **Invoke**: `POST /invoke`
- **Batch**: `POST /batch`
- **Stream**: `POST /stream`

### Severity Levels

| Level | Description | Response Time | Example |
|-------|-------------|---------------|---------|
| **P0 (Critical)** | Service down, data loss, security breach | 15 minutes | All pods crash looping |
| **P1 (High)** | Severe degradation, affecting multiple users | 1 hour | 50% error rate |
| **P2 (Medium)** | Partial degradation, limited user impact | 4 hours | Single pod unhealthy |
| **P3 (Low)** | Minor issues, no user impact | Next business day | High latency on non-critical endpoint |

---

## Incident Response Framework

### 1. Detection

**Automated Alerts** (Prometheus/Alertmanager):
- High error rate (>5% for 5 minutes)
- High latency (P95 >2s for 5 minutes)
- Pod crash loops (>3 restarts in 10 minutes)
- Low pod availability (<50% of desired replicas)
- Memory/CPU exhaustion (>90% for 5 minutes)

**Manual Detection**:
- User reports
- Monitoring dashboard anomalies
- Log pattern changes
- Third-party service status pages

### 2. Triage

**Immediate Actions** (First 5 minutes):
1. Acknowledge alert in PagerDuty/Opsgenie
2. Join incident channel (#incident-dashflow)
3. Declare incident severity (P0-P3)
4. Check deployment status: `kubectl get pods -n dashflow`
5. Check recent changes: `kubectl rollout history deployment/dashflow -n dashflow`

**Information Gathering**:
```bash
# Check pod status and restarts
kubectl get pods -n dashflow -l app=dashflow -o wide

# View recent logs (last 500 lines)
kubectl logs -n dashflow deployment/dashflow --tail=500 --all-containers=true

# Check resource usage
kubectl top pods -n dashflow -l app=dashflow

# Check events
kubectl get events -n dashflow --sort-by='.lastTimestamp' | head -20

# Check Prometheus metrics
curl http://prometheus:9090/api/v1/query?query=dashflow_errors_total

# Check upstream dependencies
curl https://status.openai.com/api/v2/status.json
```

### 3. Communication

**Incident Declaration**:
```
INCIDENT: [P0/P1/P2/P3] - [Brief Description]
Time: [HH:MM UTC]
Impact: [Number of users/services affected]
Status: Investigating
Incident Commander: [Name]
Communication Channel: #incident-dashflow
Status Page: https://status.example.com
```

**Update Frequency**:
- **P0**: Every 15 minutes
- **P1**: Every 30 minutes
- **P2**: Every 1 hour
- **P3**: Daily or as needed

### 4. Resolution

Follow specific runbook procedures (see [Common Failure Scenarios](#common-failure-scenarios))

### 5. Post-Incident

- Complete post-incident review (see [Post-Incident Review](#post-incident-review))
- Update runbook with new learnings
- Implement preventive measures
- Update monitoring/alerting if needed

---

## Common Failure Scenarios

### Scenario 1: Pod Crash Loop

**Symptoms**:
- Pods continuously restarting
- `CrashLoopBackOff` status in `kubectl get pods`
- Application logs show panic or fatal error

**Diagnosis**:
```bash
# Check pod status
kubectl get pods -n dashflow -l app=dashflow

# View logs from failed container
kubectl logs -n dashflow <pod-name> --previous

# Describe pod for events
kubectl describe pod <pod-name> -n dashflow
```

**Common Causes**:
1. **Missing Environment Variables**
   - Symptom: Logs show "environment variable not found" or panic on startup
   - Fix: Verify ConfigMap/Secret mounted correctly
   ```bash
   kubectl get configmap dashflow-config -n dashflow -o yaml
   kubectl get secret dashflow-secrets -n dashflow -o yaml
   ```

2. **Out of Memory (OOM)**
   - Symptom: Pod killed with exit code 137, events show "OOMKilled"
   - Fix: Increase memory limit in deployment manifest
   ```yaml
   resources:
     limits:
       memory: "256Mi"  # Increase from 128Mi
   ```

3. **Invalid Configuration**
   - Symptom: Logs show parsing error or validation failure
   - Fix: Validate configuration format, check RUST_LOG syntax

4. **Port Conflict**
   - Symptom: "Address already in use" in logs
   - Fix: Verify no port conflicts in deployment, check service configuration

**Resolution Steps**:
1. Identify root cause from logs/events
2. Apply fix (update ConfigMap, increase resources, fix config)
3. Trigger redeployment: `kubectl rollout restart deployment/dashflow -n dashflow`
4. Monitor rollout: `kubectl rollout status deployment/dashflow -n dashflow`
5. Verify health: `curl http://<service-ip>:8080/health`

**Rollback if Fix Fails**:
```bash
kubectl rollout undo deployment/dashflow -n dashflow
```

---

### Scenario 2: High Error Rate

**Symptoms**:
- `dashflow_errors_total` metric increasing rapidly
- HTTP 500 errors in logs
- Prometheus alert: `HighErrorRate`

**Diagnosis**:
```bash
# Check error rate (last 5 minutes)
kubectl logs -n dashflow deployment/dashflow --tail=1000 | grep ERROR | wc -l

# Check specific error types
kubectl logs -n dashflow deployment/dashflow --tail=1000 | grep ERROR | head -20

# Query Prometheus for error breakdown
curl 'http://prometheus:9090/api/v1/query?query=rate(dashflow_errors_total[5m])'
```

**Common Causes**:

1. **Upstream LLM API Failure** (OpenAI, Anthropic, etc.)
   - Symptom: Errors contain "API error", "rate limit", "timeout"
   - Check: Verify upstream status page
   - Fix: See [Dependency Outage Handling](#dependency-outage-handling)

2. **Database Connection Exhaustion**
   - Symptom: Errors contain "too many connections", "connection refused"
   - Check: Database connection pool metrics
   - Fix: Increase connection pool size or scale database

3. **Rate Limiting**
   - Symptom: Errors contain "rate limit exceeded", 429 responses
   - Check: `dashflow_requests_total` rate vs configured limits
   - Fix: Increase rate limits or scale horizontally

4. **Invalid Input**
   - Symptom: Errors contain "validation error", "parse error"
   - Check: Sample failed requests in logs
   - Fix: Improve input validation, add better error messages

**Resolution Steps**:
1. Identify error pattern from logs (group by error type)
2. Check if errors are transient (retry succeeds) or persistent
3. If transient: Monitor recovery, may self-resolve
4. If persistent: Apply specific fix based on root cause
5. If no fix available: Enable circuit breaker to fail fast

**Circuit Breaker Configuration** (if supported):
```rust
// Example: Fail fast after 10 consecutive errors
circuit_breaker_threshold: 10,
circuit_breaker_timeout: Duration::from_secs(30),
```

---

### Scenario 3: High Latency

**Symptoms**:
- `dashflow_request_duration_seconds` P95/P99 elevated
- User reports of slow responses
- Prometheus alert: `HighLatency`

**Diagnosis**:
```bash
# Check P95/P99 latency
curl 'http://prometheus:9090/api/v1/query?query=histogram_quantile(0.95, rate(dashflow_request_duration_seconds_bucket[5m]))'

# Check if latency is endpoint-specific
kubectl logs -n dashflow deployment/dashflow --tail=500 | grep "duration_ms"

# Check resource usage
kubectl top pods -n dashflow -l app=dashflow

# Check HPA status (if autoscaling enabled)
kubectl get hpa dashflow -n dashflow
```

**Common Causes**:

1. **Resource Contention** (CPU/Memory)
   - Symptom: `kubectl top pods` shows high CPU/memory usage
   - Fix: Scale horizontally (add more pods)
   ```bash
   kubectl scale deployment dashflow -n dashflow --replicas=5
   ```

2. **Database Slow Queries**
   - Symptom: Latency spikes correlate with database load
   - Fix: Optimize queries, add indexes, scale database

3. **Upstream LLM Latency**
   - Symptom: Latency matches LLM API response time
   - Fix: Use faster models, implement caching, parallel requests

4. **Large Batch Requests**
   - Symptom: High latency on `/batch` endpoint, large `dashflow_batch_size`
   - Fix: Implement batch size limits, request splitting

5. **Network Issues**
   - Symptom: Latency spikes across all endpoints
   - Fix: Check network connectivity, DNS resolution, firewall rules

**Resolution Steps**:
1. Identify bottleneck (CPU, memory, I/O, network, upstream)
2. Scale horizontally if resource-constrained
3. Optimize code path if application bottleneck
4. Add caching if appropriate
5. Implement timeouts to prevent cascading delays

**Temporary Mitigation**:
```bash
# Reduce traffic by scaling ingress rate limit
kubectl annotate ingress dashflow -n dashflow \
  nginx.ingress.kubernetes.io/rate-limit="50" --overwrite
```

---

### Scenario 4: Pod Eviction

**Symptoms**:
- Pods terminated unexpectedly
- Events show "Evicted" status
- Pod describe shows "Reason: Evicted"

**Diagnosis**:
```bash
# Check evicted pods
kubectl get pods -n dashflow -l app=dashflow --field-selector=status.phase=Failed

# View eviction reason
kubectl describe pod <evicted-pod-name> -n dashflow | grep -A 10 "Evicted"
```

**Common Causes**:

1. **Node Disk Pressure**
   - Symptom: Events show "DiskPressure" or "ephemeral-storage exceeded"
   - Fix: Clean up logs, increase ephemeral storage limit, clean node disk

2. **Node Memory Pressure**
   - Symptom: Events show "MemoryPressure"
   - Fix: Reduce memory requests/limits, add more nodes

3. **QoS Class Preemption**
   - Symptom: "BestEffort" or "Burstable" pods evicted under pressure
   - Fix: Set resource requests = limits for "Guaranteed" QoS

**Resolution Steps**:
1. Identify eviction reason from pod events
2. Address underlying node pressure (clean disk, add capacity)
3. Adjust resource requests/limits if needed
4. New pods will be automatically created by deployment controller
5. Monitor node conditions: `kubectl get nodes -o wide`

---

### Scenario 5: Configuration Error

**Symptoms**:
- Pods start but behave incorrectly
- Logs show warnings or errors about configuration
- Features not working as expected

**Diagnosis**:
```bash
# Check current ConfigMap
kubectl get configmap dashflow-config -n dashflow -o yaml

# Check environment variables in running pod
kubectl exec -n dashflow <pod-name> -- env | grep -E "RUST_|LANGCHAIN_"

# Compare with expected configuration
diff <(kubectl get configmap dashflow-config -n dashflow -o yaml) expected_config.yaml
```

**Common Causes**:

1. **Wrong Environment Variable Format**
   - Symptom: RUST_LOG not taking effect, incorrect log level
   - Fix: Verify RUST_LOG syntax (e.g., `dashflow=debug`)

2. **Missing Secret**
   - Symptom: API calls fail with authentication error
   - Fix: Verify secret mounted and contains correct keys
   ```bash
   kubectl exec -n dashflow <pod-name> -- ls -la /etc/secrets/
   ```

3. **Stale ConfigMap**
   - Symptom: Changes to ConfigMap not reflected in pods
   - Fix: Restart pods to pick up new ConfigMap
   ```bash
   kubectl rollout restart deployment/dashflow -n dashflow
   ```

**Resolution Steps**:
1. Validate configuration syntax and values
2. Update ConfigMap/Secret: `kubectl apply -f config.yaml`
3. Restart deployment to pick up changes
4. Verify configuration took effect in logs
5. Test functionality

---

### Scenario 6: Network Connectivity Issues

**Symptoms**:
- Timeouts connecting to external services
- DNS resolution failures
- Connection refused errors

**Diagnosis**:
```bash
# Test DNS resolution from pod
kubectl exec -n dashflow <pod-name> -- nslookup api.openai.com

# Test network connectivity
kubectl exec -n dashflow <pod-name> -- curl -v https://api.openai.com/v1/models

# Check service endpoints
kubectl get endpoints dashflow -n dashflow

# Check network policies
kubectl get networkpolicies -n dashflow
```

**Common Causes**:

1. **Firewall/Security Group Blocking Traffic**
   - Symptom: Connection timeout or refused
   - Fix: Update firewall rules to allow outbound HTTPS (443)

2. **DNS Resolution Failure**
   - Symptom: "no such host" errors
   - Fix: Check CoreDNS status, verify DNS config

3. **Service Misconfiguration**
   - Symptom: Internal service calls fail
   - Fix: Verify service selector matches pod labels

4. **Network Policy Blocking Traffic**
   - Symptom: Traffic blocked between pods/namespaces
   - Fix: Update NetworkPolicy to allow required traffic

**Resolution Steps**:
1. Isolate network layer (DNS, TCP, TLS, application)
2. Test connectivity from pod using curl/nslookup
3. Check firewall rules and network policies
4. Verify service/endpoint configuration
5. Check for transient network issues (retry)

---

### Scenario 7: Monitoring System Down

**Symptoms**:
- No metrics in Grafana
- Prometheus targets unreachable
- Alerts not firing

**Diagnosis**:
```bash
# Check Prometheus status
kubectl get pods -n monitoring -l app=prometheus

# Check ServiceMonitor
kubectl get servicemonitor dashflow -n dashflow -o yaml

# Test metrics endpoint directly
kubectl port-forward -n dashflow svc/dashflow 8080:8080
curl http://localhost:8080/metrics

# Check Prometheus targets
# Access Prometheus UI -> Status -> Targets
kubectl port-forward -n monitoring svc/prometheus 9090:9090
```

**Common Causes**:

1. **ServiceMonitor Misconfiguration**
   - Symptom: Target shows as "down" in Prometheus UI
   - Fix: Verify ServiceMonitor selector matches service labels

2. **Prometheus Not Scraping**
   - Symptom: No recent scrapes in Prometheus UI
   - Fix: Check Prometheus configuration, restart Prometheus

3. **Metrics Endpoint Not Responding**
   - Symptom: Curl to `/metrics` fails or times out
   - Fix: Check if application is healthy, verify port configuration

**Resolution Steps**:
1. Verify application `/metrics` endpoint works
2. Check ServiceMonitor configuration and labels
3. Verify Prometheus can reach application pods
4. Check Prometheus scrape configuration
5. Restart Prometheus if needed

---

## Rollback Procedures

### Automated Rollback (Kubernetes)

**Rollback to Previous Version**:
```bash
# View rollout history
kubectl rollout history deployment/dashflow -n dashflow

# Rollback to previous version
kubectl rollout undo deployment/dashflow -n dashflow

# Rollback to specific revision
kubectl rollout undo deployment/dashflow -n dashflow --to-revision=3

# Monitor rollback progress
kubectl rollout status deployment/dashflow -n dashflow --watch

# Verify rollback success
kubectl get pods -n dashflow -l app=dashflow
curl http://<service-ip>:8080/health
```

**Rollback Decision Criteria**:
- High error rate (>10%) for >5 minutes after deployment
- Pod crash loops affecting >50% of pods
- Critical bug discovered in new release
- Performance regression >50% increase in P95 latency
- Security vulnerability introduced

**Rollback Timeline**:
- **Decision**: Within 5 minutes of detection
- **Execution**: 2-3 minutes (Kubernetes rollout)
- **Verification**: 5 minutes post-rollback
- **Total**: ~15 minutes from decision to verified recovery

### Manual Rollback (Docker)

If Kubernetes is unavailable or for standalone deployments:

```bash
# Stop current container
docker stop dashflow

# Start previous version
docker run -d \
  --name dashflow \
  --restart unless-stopped \
  -p 8080:8080 \
  -e RUST_LOG=info \
  -e OPENAI_API_KEY=${OPENAI_API_KEY} \
  ghcr.io/org/dashflow:v0.9.0  # Previous known-good version

# Verify health
curl http://localhost:8080/health

# View logs
docker logs -f dashflow
```

### Database Rollback

**Schema Migrations**:
```bash
# If using sqlx migrations
sqlx migrate revert --source ./migrations

# If using custom migration tool
./migrate down 1

# Verify migration status
sqlx migrate info
```

**Data Rollback** (if applicable):
- Restore from point-in-time backup
- Coordinate with DBA team for large databases
- Test rollback in staging first if possible

### Post-Rollback Actions

1. **Verify Service Health**:
   - Check `/health` endpoint returns 200
   - Check error rate in metrics (<1%)
   - Check latency back to baseline
   - Verify critical user flows work

2. **Communication**:
   - Update incident channel with rollback status
   - Update status page: "Incident resolved via rollback"
   - Notify stakeholders

3. **Investigation**:
   - Preserve logs/metrics from failed deployment
   - Create incident report documenting issue
   - Identify root cause before next deployment

4. **Prevention**:
   - Add test case for bug that triggered rollback
   - Update deployment checklist
   - Improve canary/blue-green deployment if applicable

---

## Scaling Operations

### Horizontal Pod Autoscaling (HPA)

**Current Configuration** (example HPA configuration):
```yaml
# Example horizontal pod autoscaler configuration
minReplicas: 2
maxReplicas: 10
targetCPUUtilizationPercentage: 70
```

**Manual Scaling**:
```bash
# Scale up
kubectl scale deployment dashflow -n dashflow --replicas=5

# Scale down
kubectl scale deployment dashflow -n dashflow --replicas=2

# Check current scale
kubectl get deployment dashflow -n dashflow
kubectl get hpa dashflow -n dashflow
```

**Scaling Decision Criteria**:

**Scale Up** (increase replicas):
- CPU usage >70% sustained for >2 minutes
- P95 latency >2s for >5 minutes
- Request rate increasing rapidly (>2x baseline)
- Anticipating traffic spike (product launch, marketing campaign)

**Scale Down** (decrease replicas):
- CPU usage <30% sustained for >10 minutes
- Low request rate (<50% of capacity)
- Cost optimization during off-peak hours
- Never scale below minimum replicas (2) for availability

**Scaling Timeline**:
- **HPA Automatic**: 1-2 minutes from threshold breach to new pods ready
- **Manual**: 30-60 seconds from command to new pods ready

### Vertical Scaling (Resource Adjustment)

**When to Vertically Scale**:
- Consistent OOM kills even with horizontal scaling
- High CPU usage but low request volume
- Memory leaks discovered (temporary measure until fix)

**Procedure**:
```bash
# Edit deployment
kubectl edit deployment dashflow -n dashflow

# Update resources
spec:
  template:
    spec:
      containers:
      - name: dashflow
        resources:
          requests:
            memory: "128Mi"  # Increase
            cpu: "200m"      # Increase
          limits:
            memory: "256Mi"  # Increase
            cpu: "400m"      # Increase

# Rollout will trigger automatically
kubectl rollout status deployment/dashflow -n dashflow
```

**Monitoring During Scale Operations**:
```bash
# Watch pod status
watch kubectl get pods -n dashflow -l app=dashflow

# Monitor metrics
# Check Grafana dashboard: "DashFlow Overview"
# Watch for:
# - Request rate distribution across pods
# - Error rate (should remain stable)
# - Latency (should improve after scale-up)
```

### Scaling Strategy by Traffic Pattern

**Gradual Growth** (normal traffic increase):
- Let HPA handle automatically
- Monitor metrics every 15 minutes
- Verify new pods handling traffic evenly

**Sudden Spike** (unexpected traffic):
1. Immediately check if traffic is legitimate (not DDoS)
2. If legitimate: Manually scale to handle load
   ```bash
   kubectl scale deployment dashflow -n dashflow --replicas=10
   ```
3. Enable aggressive rate limiting at ingress if needed
4. Monitor error rate closely
5. Scale down gradually after spike subsides

**Scheduled Event** (known traffic spike):
1. Pre-scale 15-30 minutes before event
   ```bash
   kubectl scale deployment dashflow -n dashflow --replicas=8
   ```
2. Monitor during event
3. Scale down 30 minutes after event ends

**Cost Optimization** (off-peak hours):
1. Identify off-peak hours from historical metrics
2. Schedule cron job to scale down (e.g., 2am-6am)
3. Never go below minimum replicas (2)
4. Document schedule in runbook

---

## Dependency Outage Handling

### Upstream LLM Provider Outage (OpenAI, Anthropic, etc.)

**Detection**:
- High error rate with "API error" messages
- Timeouts or 5xx responses from provider
- Check provider status page

**Immediate Actions**:
1. Verify outage via status page (e.g., https://status.openai.com)
2. Check if outage is partial (specific models/regions)
3. Notify users via status page update

**Mitigation Options**:

**Option 1: Failover to Alternative Provider**:
```rust
// Example: Fallback from OpenAI to Anthropic
if openai_error.is_rate_limit() || openai_error.is_service_unavailable() {
    return anthropic_client.invoke(prompt).await;
}
```

**Option 2: Use Cached Responses** (if applicable):
- Check if request matches recent cache entry
- Serve stale cached response with warning header
- Document cache TTL and staleness policy

**Option 3: Graceful Degradation**:
- Return error with helpful message: "LLM provider temporarily unavailable"
- Suggest retry with exponential backoff
- Queue requests for later processing (if durable queue available)

**Option 4: Circuit Breaker**:
- Open circuit after N consecutive failures (e.g., 10)
- Fail fast for M seconds (e.g., 30)
- Attempt half-open state to test recovery

**Monitoring During Outage**:
```bash
# Check error rate by provider
kubectl logs -n dashflow deployment/dashflow --tail=500 | grep "openai" | grep ERROR | wc -l

# Monitor recovery
watch "curl -s http://prometheus:9090/api/v1/query?query=dashflow_errors_total | jq"
```

**Recovery Verification**:
1. Status page shows "operational"
2. Test request succeeds: `curl -X POST http://localhost:8080/invoke ...`
3. Error rate returns to baseline (<1%)
4. Close circuit breaker (allow traffic)

**Post-Outage**:
- Document outage duration and impact
- Review failover effectiveness
- Consider multi-provider redundancy if not implemented

---

### Database Outage

**Detection**:
- Errors containing "connection refused", "database unavailable"
- Health check fails (if health check includes database)

**Immediate Actions**:
1. Verify database status: `kubectl get pods -n database`
2. Check database logs for root cause
3. Attempt database restart if safe

**Mitigation**:

**Short Outage (<5 minutes)**:
- Connection pool will retry automatically
- Queue requests if durable queue exists
- Return 503 Service Unavailable with Retry-After header

**Long Outage (>5 minutes)**:
- Enable read-only mode if read replica available
- Serve cached data where applicable
- Consider degraded mode (limited functionality)

**Database Recovery**:
```bash
# If using managed database (RDS, Cloud SQL)
# Check cloud console for automated recovery

# If self-hosted PostgreSQL/MySQL
kubectl rollout restart statefulset postgres -n database

# Verify connectivity from application
kubectl exec -n dashflow <pod-name> -- nc -zv postgres.database.svc.cluster.local 5432
```

**Post-Recovery**:
- Verify data consistency (no corruption)
- Check for stuck transactions
- Monitor replication lag (if applicable)
- Run integrity checks (if available)

---

### Vector Store Outage (Pinecone, Qdrant, etc.)

**Detection**:
- Errors containing "vector store", "index not found"
- Embedding search requests fail

**Mitigation**:
- Return cached search results if available
- Degrade to keyword search (if fallback exists)
- Return error with helpful message

**Recovery**:
- Similar to LLM provider outage
- Verify index health after recovery
- Check for index corruption or data loss

---

## Database and Vector Store Recovery

### Database Backup and Restore

**Backup Verification** (Regular Schedule):
```bash
# Check last backup timestamp
aws s3 ls s3://backups/dashflow-db/ --recursive | sort | tail -5

# Verify backup integrity
psql -h backup-host -U postgres -c "SELECT pg_size_pretty(pg_database_size('dashflow'));"
```

**Point-in-Time Recovery**:
```bash
# Restore to timestamp (e.g., before bad deployment)
aws rds restore-db-instance-to-point-in-time \
  --source-db-instance-identifier dashflow-prod \
  --target-db-instance-identifier dashflow-restore \
  --restore-time 2025-11-04T10:00:00Z

# Wait for restore to complete
aws rds wait db-instance-available --db-instance-identifier dashflow-restore

# Update application to use restored database
kubectl set env deployment/dashflow -n dashflow \
  DATABASE_URL="postgres://user:pass@dashflow-restore.region.rds.amazonaws.com:5432/dashflow"
```

### Vector Store Recovery

**Rebuild Vector Index** (if index corrupted):
```bash
# Export embeddings from database
psql -h db-host -U postgres -d dashflow -c "COPY (SELECT id, embedding FROM documents) TO STDOUT CSV" > embeddings.csv

# Re-create index in vector store (example: Pinecone)
# Note: Create a custom script for your vector store provider
# Example for Pinecone using their Python SDK:
# pip install pinecone-client && python -c "
#   import pinecone; import csv
#   pc = pinecone.Pinecone(api_key=os.environ['PINECONE_API_KEY'])
#   index = pc.Index('dashflow-prod')
#   with open('embeddings.csv') as f:
#     for row in csv.reader(f):
#       index.upsert(vectors=[(row[0], [float(x) for x in row[1].split(',')])])
# "

# Verify index count
curl -X GET "https://api.pinecone.io/indexes/dashflow-prod/stats" \
  -H "Api-Key: $PINECONE_API_KEY"
```

**Replication Recovery** (if using replicas):
```bash
# Force resync from primary
# (Specific commands depend on vector store implementation)

# Verify replication lag
# Check vector store admin console or API
```

---

## Performance Degradation

### Memory Leak Detection

**Symptoms**:
- Memory usage steadily increasing over time
- OOM kills after hours/days of uptime
- Memory not reclaimed after load decreases

**Diagnosis**:
```bash
# Check memory usage over time
kubectl top pod <pod-name> -n dashflow --containers

# Check for memory-related OOM kills
kubectl describe pod <pod-name> -n dashflow | grep -i oom

# If available, use memory profiler
# (Requires build with profiling enabled)
```

**Immediate Mitigation**:
```bash
# Restart affected pods to reclaim memory
kubectl delete pod <pod-name> -n dashflow

# Or rolling restart entire deployment
kubectl rollout restart deployment/dashflow -n dashflow
```

**Investigation**:
- Review recent code changes for leaks
- Check for unbounded caches or collections
- Look for unclosed connections/handles
- Profile with tools like `valgrind` or Rust's `heaptrack`

**Long-term Fix**:
- Fix leak in code
- Add memory monitoring/alerts
- Set appropriate memory limits
- Consider scheduled pod restarts (not ideal, but temporary mitigation)

---

### CPU Saturation

**Symptoms**:
- CPU usage at 100% sustained
- High latency despite low request rate
- Throttling in Kubernetes metrics

**Diagnosis**:
```bash
# Check CPU usage
kubectl top pods -n dashflow -l app=dashflow

# Profile CPU usage (requires profiling build)
# Use tools like `perf`, `flamegraph`, or Rust's built-in profiling
```

**Mitigation**:
- Scale horizontally: `kubectl scale deployment dashflow -n dashflow --replicas=5`
- Identify hot code paths and optimize
- Check for inefficient algorithms or blocking operations
- Consider using faster models/services upstream

---

## Security Incidents

### API Key Leak

**Symptoms**:
- Unusual traffic patterns (requests from unknown IPs)
- High usage/costs from LLM provider
- Security alert from secret scanning tool

**Immediate Actions**:
1. **Rotate compromised key immediately**:
   ```bash
   # Generate new key from provider dashboard
   # Update Kubernetes secret
   kubectl create secret generic dashflow-secrets -n dashflow \
     --from-literal=OPENAI_API_KEY=sk-new-key \
     --dry-run=client -o yaml | kubectl apply -f -

   # Restart pods to use new key
   kubectl rollout restart deployment/dashflow -n dashflow
   ```

2. **Revoke old key** via provider dashboard

3. **Audit access**:
   - Check provider usage logs for unauthorized requests
   - Identify source of leak (logs, repository, exposed endpoint)
   - Document timeline and scope

4. **Notify**:
   - Security team
   - Stakeholders
   - Provider if fraud detected

**Prevention**:
- Use secret management (Kubernetes secrets, Vault, AWS Secrets Manager)
- Enable secret scanning in CI/CD (Gitleaks)
- Never log API keys
- Use short-lived tokens where possible
- Rotate keys regularly (quarterly)

---

### DDoS Attack

**Symptoms**:
- Sudden traffic spike from many IPs
- High request rate overwhelming application
- Legitimate users unable to access service

**Immediate Actions**:
1. **Enable aggressive rate limiting**:
   ```bash
   # Reduce ingress rate limit
   kubectl annotate ingress dashflow -n dashflow \
     nginx.ingress.kubernetes.io/rate-limit="10" --overwrite
   ```

2. **Block attack sources** (if identifiable):
   ```bash
   # Update firewall rules or use cloud provider DDoS protection
   # Example: AWS Shield, Cloudflare
   ```

3. **Scale up to absorb attack** (if resources available):
   ```bash
   kubectl scale deployment dashflow -n dashflow --replicas=10
   ```

4. **Enable CAPTCHA** (if available) for suspicious traffic

**Post-Attack**:
- Review access logs for attack patterns
- Implement permanent rate limiting
- Consider DDoS protection service (Cloudflare, AWS Shield)
- Update WAF rules to block similar attacks

---

### Unauthorized Access

**Symptoms**:
- Authentication/authorization errors in logs
- Requests from unexpected sources
- Data access anomalies

**Immediate Actions**:
1. **Block unauthorized access**:
   - Update firewall rules
   - Revoke compromised credentials
   - Enable IP allowlisting if applicable

2. **Audit access logs**:
   ```bash
   kubectl logs -n dashflow deployment/dashflow --tail=10000 | grep -E "(403|401)"
   ```

3. **Check for data exfiltration**:
   - Review outbound network traffic
   - Check for unusual data access patterns

4. **Notify security team** immediately

**Investigation**:
- Determine entry point (credential leak, vulnerability, misconfiguration)
- Assess data exposure (what data was accessed)
- Identify affected users/accounts
- Document timeline and indicators of compromise

**Remediation**:
- Patch vulnerability (if applicable)
- Rotate all credentials
- Strengthen authentication (MFA, IP allowlisting)
- Implement additional monitoring/alerting

---

## On-Call Escalation

### Escalation Path

**Level 1: On-Call Engineer** (First Responder)
- **Response Time**: 15 minutes for P0/P1, 1 hour for P2, next business day for P3
- **Responsibilities**: Triage, initial investigation, follow runbook procedures
- **Escalate to L2 if**: Unable to resolve in 30 minutes (P0) or 2 hours (P1)

**Level 2: Senior Engineer / Team Lead**
- **Response Time**: 30 minutes (during business hours), 1 hour (off-hours)
- **Responsibilities**: Advanced troubleshooting, code-level debugging, architecture decisions
- **Escalate to L3 if**: Issue requires vendor support or C-level decision

**Level 3: Engineering Manager / CTO**
- **Response Time**: 1 hour
- **Responsibilities**: Vendor escalation, business decisions, executive communication
- **Escalate if**: Requires legal, PR, or executive approval

### Contact Information

**On-Call Rotation**:
- Check PagerDuty/Opsgenie for current on-call engineer
- Slack: `@oncall-dashflow`
- Email: oncall-dashflow@example.com

**Escalation Contacts**:
- **L2 (Senior Engineer)**: John Doe - @johndoe - +1-555-0101
- **L3 (Engineering Manager)**: Jane Smith - @janesmith - +1-555-0102
- **Security Team**: security@example.com - Slack: #security
- **Database Team**: dba@example.com - Slack: #database

**Vendor Support**:
- **OpenAI**: https://platform.openai.com/support
- **Anthropic**: support@anthropic.com
- **Cloud Provider**: Support ticket via console
- **Database Provider**: Support via console/phone

### When to Escalate

**Escalate Immediately (P0)**:
- Service completely down for >15 minutes
- Data loss or corruption detected
- Security breach confirmed
- Unable to rollback or mitigate

**Escalate to L2 (P1)**:
- Unable to identify root cause within 30 minutes
- Issue requires code changes or architectural decision
- Vendor support needed
- Multiple simultaneous incidents

**Escalate to L3**:
- Potential legal/compliance implications
- Requires business decision (e.g., disable feature, change SLA)
- Public communication needed (press, social media)
- Vendor escalation required

### Handoff Procedure

When escalating or handing off incident:

1. **Document current state**:
   - What has been tried (commands run, changes made)
   - Current metrics (error rate, latency, affected users)
   - Logs/evidence collected
   - Theories and next steps

2. **Brief next responder**:
   - 2-minute verbal summary
   - Share incident doc link
   - Transfer on-call access (PagerDuty, Opsgenie)

3. **Update incident channel**:
   ```
   HANDOFF: Incident now owned by @janesmith (L2)
   Summary: [Brief description]
   Incident Doc: [Link]
   Next Steps: [What L2 will investigate]
   ```

---

## Maintenance Windows

### Scheduled Maintenance

**Planning** (1 week advance notice):
1. Schedule maintenance window (low-traffic period)
2. Create maintenance plan:
   - What will be done (upgrade, migration, config change)
   - Expected duration
   - Rollback plan
   - Success criteria
3. Notify stakeholders via email and status page
4. Update status page: "Scheduled Maintenance: [Date] [Time]"

**During Maintenance**:
1. Post start update: "Maintenance window started"
2. Follow maintenance plan step-by-step
3. Update status page every 30 minutes
4. Document any deviations from plan

**Post-Maintenance**:
1. Verify success criteria met
2. Post completion update: "Maintenance complete"
3. Monitor for 1 hour post-maintenance
4. Document actual vs planned duration
5. Update runbook if needed

**Example Maintenance Plan**:
```
Maintenance: Kubernetes 1.28 -> 1.29 Upgrade
Date: 2025-11-10 02:00-04:00 UTC
Duration: 2 hours
Impact: Brief pod restarts (30s downtime per pod)

Steps:
1. [02:00] Drain node 1, upgrade, uncordon (30 min)
2. [02:30] Verify pods rescheduled healthy
3. [02:35] Drain node 2, upgrade, uncordon (30 min)
4. [03:05] Drain node 3, upgrade, uncordon (30 min)
5. [03:35] Verify all nodes upgraded
6. [03:40] Run smoke tests
7. [03:50] Monitor metrics for anomalies
8. [04:00] Close maintenance window

Rollback: If nodes fail to upgrade, revert to 1.28 (1 hour)
Success Criteria: All pods healthy, metrics normal, smoke tests pass
```

---

### Emergency Maintenance (No Notice)

**Triggers**:
- Critical security vulnerability discovered
- Data corruption requiring immediate fix
- Cascading failure requiring emergency intervention

**Procedure**:
1. Declare emergency maintenance in incident channel
2. Post status page update: "Emergency Maintenance in Progress"
3. Execute minimum necessary changes
4. Prioritize speed over perfection
5. Document all actions taken
6. Post-maintenance review within 24 hours

---

## Post-Incident Review

### Timeline (Within 48 Hours of Resolution)

**Participants**:
- Incident Commander
- On-call engineers involved
- Team lead
- Relevant stakeholders

**Agenda** (1 hour meeting):
1. Incident timeline (what happened when)
2. Root cause analysis (why it happened)
3. Response effectiveness (what went well, what didn't)
4. Action items (how to prevent recurrence)

### Post-Incident Report Template

```markdown
# Post-Incident Review: [Incident Title]

**Incident ID**: INC-20251104-001
**Date**: 2025-11-04
**Severity**: P1
**Duration**: 2h 15m (10:00 UTC - 12:15 UTC)
**Incident Commander**: John Doe

## Summary
[1-2 paragraph summary of incident and impact]

## Impact
- **Users Affected**: ~1,000 users (10% of daily active users)
- **Revenue Impact**: $5,000 estimated (failed transactions)
- **Requests Failed**: ~50,000 (error rate 85% during peak)
- **Duration**: 2h 15m

## Timeline (UTC)

| Time | Event |
|------|-------|
| 10:00 | Alert fired: HighErrorRate |
| 10:02 | On-call acknowledged, started investigation |
| 10:05 | Identified root cause: OOM kills from memory leak |
| 10:10 | Decision to rollback to v0.9.0 |
| 10:13 | Rollback initiated |
| 10:18 | Pods restarted with previous version |
| 10:20 | Error rate returned to baseline |
| 10:30 | Monitoring for stability |
| 11:00 | Confirmed stable, investigation continued |
| 12:00 | Fix deployed (memory leak patched) |
| 12:15 | Incident closed |

## Root Cause
[Detailed technical explanation of what caused the incident]

Example: Memory leak introduced in v0.10.0 due to unbounded cache growth.
Cache was not configured with TTL or max size, leading to memory exhaustion
after ~2 hours of uptime under normal load. Triggered OOM kills and pod restarts.

## Contributing Factors
- Insufficient memory limit testing in staging (staging load test only ran 30 minutes)
- No memory profiling before release
- Monitoring alert for memory usage was set too high (90%, should be 75%)

## What Went Well
- Alert fired quickly (within 1 minute of error rate spike)
- On-call responded promptly (2 minutes to acknowledge)
- Root cause identified quickly (5 minutes)
- Rollback executed smoothly (8 minutes from decision to resolution)
- Communication clear and timely

## What Didn't Go Well
- Memory leak not caught in testing
- No canary deployment (rolled out to 100% immediately)
- Monitoring alert threshold too high
- Post-deployment monitoring only 15 minutes (should be 1 hour)

## Action Items

| ID | Action | Owner | Due Date | Status |
|----|--------|-------|----------|--------|
| 1 | Add memory profiling to CI/CD | Jane | 2025-11-08 | In Progress |
| 2 | Implement canary deployment (10% traffic for 1h) | John | 2025-11-11 | Not Started |
| 3 | Lower memory alert threshold to 75% | Ops | 2025-11-05 | Complete |
| 4 | Extend staging load test to 2 hours | QA | 2025-11-10 | Not Started |
| 5 | Add TTL to cache configuration | Dev | 2025-11-05 | Complete |
| 6 | Update deployment checklist with post-deploy monitoring | SRE | 2025-11-06 | In Progress |

## Lessons Learned
1. Always test for memory leaks with extended load testing (>2 hours)
2. Canary deployments catch issues before widespread impact
3. Monitoring thresholds should be tuned based on historical data
4. Post-deployment monitoring should extend to 1 hour minimum
5. All caches must have TTL or max size configured

## Related Documentation
- Incident log: [Link]
- Metrics dashboard: [Link]
- Code change that introduced leak: [Link to commit]
- Fix PR: [Link]
```

---

## Appendix

### Monitoring Dashboard URLs

- **Grafana**: https://grafana.example.com/d/dashflow
- **Prometheus**: https://prometheus.example.com
- **Logs**: https://logs.example.com (Loki/ELK/CloudWatch)
- **Status Page**: https://status.example.com

### Runbook Update History

| Version | Date | Changes | Author |
|---------|------|---------|--------|
| 1.0 | 2025-11-04 | Initial production runbook | Claude AI Worker #742 |

### Related Documentation

- [Production Deployment Guide](PRODUCTION_DEPLOYMENT.md)
- [Observability Guide](OBSERVABILITY.md)
- [Security Audit](SECURITY_AUDIT.md)
- [Architecture Guide](ARCHITECTURE.md)

---

**Document Owner**: SRE Team
**Review Cycle**: Quarterly
**Last Reviewed**: 2026-01-03
**Next Review**: 2026-04-03
