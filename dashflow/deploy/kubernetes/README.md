# DashFlow Kubernetes Deployment

This directory contains Kubernetes manifests for deploying DashFlow using Kustomize.

## Directory Structure

```
kubernetes/
├── base/                    # Base manifests (shared across environments)
│   ├── configs/             # Configuration files
│   │   ├── prometheus.yml   # Prometheus scrape config
│   │   ├── alertmanager.yml # Alertmanager routing
│   │   └── grafana-datasources.yaml
│   ├── namespace.yaml       # DashFlow namespace
│   ├── configmap.yaml       # Non-sensitive configuration
│   ├── secret.yaml          # Secrets template
│   ├── rbac.yaml            # Service accounts and RBAC
│   ├── redis.yaml           # Redis StatefulSet
│   ├── kafka.yaml           # Kafka + Zookeeper StatefulSets
│   ├── websocket-server.yaml    # WebSocket server Deployment
│   ├── quality-monitor.yaml     # Quality monitor Deployment
│   ├── prometheus-exporter.yaml # Prometheus exporter Deployment
│   ├── observability.yaml   # Jaeger, Prometheus, Grafana
│   ├── ingress.yaml         # Ingress configuration
│   └── kustomization.yaml   # Kustomize base config
├── overlays/
│   ├── dev/                 # Development environment
│   ├── staging/             # Staging environment
│   └── production/          # Production environment
│       ├── hpa.yaml         # Horizontal Pod Autoscalers
│       ├── pdb.yaml         # Pod Disruption Budgets
│       └── kustomization.yaml
└── README.md
```

## Prerequisites

- Kubernetes cluster (1.25+)
- kubectl configured
- kustomize (or kubectl with kustomize support)
- (Optional) Nginx Ingress Controller for external access

## Quick Start

### 1. Configure Secrets

Edit `base/secret.yaml` and add your API keys (base64 encoded):

```bash
# Encode your API key
echo -n 'sk-your-openai-key' | base64
```

### 2. Deploy to Development

```bash
# Preview what will be deployed
kubectl kustomize deploy/kubernetes/overlays/dev

# Apply to cluster
kubectl apply -k deploy/kubernetes/overlays/dev
```

### 3. Deploy to Production

```bash
# Preview
kubectl kustomize deploy/kubernetes/overlays/production

# Apply
kubectl apply -k deploy/kubernetes/overlays/production
```

## Environment Differences

| Setting | Dev | Staging | Production |
|---------|-----|---------|------------|
| WebSocket replicas | 1 | 2 | 3 (auto-scales to 10) |
| Quality Monitor replicas | 1 | 1 | 2 (auto-scales to 5) |
| Log level | debug | info | warn |
| Resource limits | Low | Medium | High |
| HPA | No | No | Yes |
| PDB | No | No | Yes |

## Accessing Services

### Port Forwarding (Development)

```bash
# Grafana Dashboard
kubectl port-forward -n dashflow svc/grafana 3000:3000

# Jaeger Tracing
kubectl port-forward -n dashflow svc/jaeger 16686:16686

# Prometheus
kubectl port-forward -n dashflow svc/prometheus 9090:9090

# WebSocket Server
kubectl port-forward -n dashflow svc/websocket-server 3002:3002
```

### Via Ingress (Production)

Configure your DNS to point to your Ingress controller:

- `dashflow.example.com/grafana` - Grafana UI
- `dashflow.example.com/jaeger` - Jaeger UI
- `dashflow.example.com/ws` - WebSocket endpoint
- `ws.dashflow.example.com` - Dedicated WebSocket hostname (recommended)

## Configuration

### Environment Variables

Edit `base/configmap.yaml` to modify:

- `RUST_LOG` - Log verbosity
- `KAFKA_TOPIC` - Kafka topic name
- `WEBSOCKET_HOST` / `WEBSOCKET_PORT` - WebSocket binding

### Secrets

Store sensitive data in `base/secret.yaml`:

- `OPENAI_API_KEY` - For LLM-based quality monitoring
- `ANTHROPIC_API_KEY` - Alternative LLM provider
- `GRAFANA_ADMIN_PASSWORD` - Grafana admin password

## Monitoring

### Prometheus Metrics

All DashFlow services expose metrics at `/metrics`:

- `dashflow_events_total` - Total events processed
- `dashflow_latency_seconds` - Processing latency
- `dashflow_errors_total` - Error counts
- `dashflow_quality_score` - LLM output quality scores

### Grafana Dashboards

Pre-configured dashboards:

1. **DashFlow Overview** - System health and throughput
2. **Quality Monitoring** - LLM output quality trends
3. **Streaming Performance** - Kafka and WebSocket metrics

### Alerting

Alertmanager is pre-configured with:

- Critical alerts (service down, high error rate)
- Warning alerts (high latency, low quality scores)

Configure receivers in `base/configs/alertmanager.yml`.

## Scaling

### WebSocket Server Scaling (M-415)

Do **not** scale `dashflow-websocket-server` beyond `replicas=1` unless you have implemented a
shared backplane (or another strategy that guarantees every client sees the full stream).
With a shared `KAFKA_GROUP_ID`, Kafka partitions are distributed across pods, so a client
connected to a single pod will only see a subset of the stream.

### Automatic Scaling (Production)

Production overlay includes HPA configurations that scale based on:

- CPU utilization (target: 70%)
- Memory utilization (target: 80%)

## Troubleshooting

### Check Pod Status

```bash
kubectl get pods -n dashflow
kubectl describe pod <pod-name> -n dashflow
```

### View Logs

```bash
kubectl logs -n dashflow deployment/dashflow-websocket-server
kubectl logs -n dashflow deployment/dashflow-quality-monitor -f
```

### Debug Network Issues

```bash
kubectl run -n dashflow debug --rm -it --image=nicolaka/netshoot -- bash
# Then use curl, nslookup, etc.
```

## Production Considerations

### High Availability

- Deploy 3+ replicas of stateless services
- Use Redis Cluster or AWS ElastiCache for HA Redis
- Consider managed Kafka (AWS MSK, Confluent Cloud)
- Enable PodDisruptionBudgets (included in production overlay)

### Security

- Use external secrets management (HashiCorp Vault, AWS Secrets Manager)
- Enable TLS on Ingress
- Configure NetworkPolicies (included in `rbac.yaml`)
- Run containers as non-root (configured in deployments)

### Backup

- Enable persistent volumes for Kafka data
- Configure Redis persistence (AOF enabled by default)
- Back up Prometheus data for historical metrics

## Version Information

- DashFlow Version: 1.11.3
- Kubernetes API: v1.25+
- Kustomize: v5.0+
