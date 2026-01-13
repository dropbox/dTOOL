# DashFlow Helm Chart

Helm chart for deploying DashFlow on Kubernetes.

## Prerequisites

- Kubernetes 1.25+
- Helm 3.0+

## Installation

### Add Repository (when published)

```bash
helm repo add dashflow https://charts.dashflow.io
helm repo update
```

### Install from Local Chart

```bash
# Install with default values
helm install dashflow ./deploy/helm/dashflow -n dashflow --create-namespace

# Install with custom values
helm install dashflow ./deploy/helm/dashflow -n dashflow --create-namespace \
  --set secrets.openaiApiKey="sk-your-key"

# Install with values file
helm install dashflow ./deploy/helm/dashflow -n dashflow --create-namespace \
  -f my-values.yaml
```

## Configuration

See [values.yaml](dashflow/values.yaml) for all available options.

### Key Configuration Options

| Parameter | Description | Default |
|-----------|-------------|---------|
| `namespace` | Namespace for deployment | `dashflow` |
| `websocketServer.replicaCount` | WebSocket server replicas | `1` |
| `websocketServer.autoscaling.enabled` | Enable HPA | `false` |
| `qualityMonitor.replicaCount` | Quality monitor replicas | `1` |
| `prometheusExporter.enabled` | Deploy Prometheus exporter | `true` |
| `redis.enabled` | Deploy built-in Redis | `true` |
| `redis.external` | Use external Redis | `false` |
| `kafka.enabled` | Deploy built-in Kafka | `true` |
| `kafka.external` | Use external Kafka | `false` |
| `observability.grafana.enabled` | Deploy Grafana | `true` |
| `observability.jaeger.enabled` | Deploy Jaeger | `true` |
| `ingress.enabled` | Enable Ingress | `false` |
| `secrets.openaiApiKey` | OpenAI API key | `""` |

### Using External Services

To use external Redis or Kafka:

```yaml
# values-external.yaml
redis:
  enabled: true
  external: true
  host: redis.example.com
  port: 6379

kafka:
  enabled: true
  external: true
  brokers: kafka1.example.com:9092,kafka2.example.com:9092
```

### Kafka Security (SASL/TLS)

To connect to a secured Kafka cluster:

```yaml
# values-kafka-security.yaml
kafka:
  enabled: true
  external: true
  brokers: kafka.example.com:9093
  security:
    # Security protocol: plaintext, ssl, sasl_plaintext, sasl_ssl
    protocol: sasl_ssl
    sasl:
      mechanism: SCRAM-SHA-256
      username: kafka-user
      password: kafka-password  # Use external secrets in production
    ssl:
      caLocation: /etc/kafka/ca.pem
      # For mTLS (optional):
      # certificateLocation: /etc/kafka/client.pem
      # keyLocation: /etc/kafka/client-key.pem
      # keyPassword: ""
```

**Environment Variables Set:**

| Value | Environment Variable |
|-------|---------------------|
| `kafka.security.protocol` | `KAFKA_SECURITY_PROTOCOL` |
| `kafka.security.sasl.mechanism` | `KAFKA_SASL_MECHANISM` |
| `kafka.security.sasl.username` | `KAFKA_SASL_USERNAME` (secret) |
| `kafka.security.sasl.password` | `KAFKA_SASL_PASSWORD` (secret) |
| `kafka.security.ssl.caLocation` | `KAFKA_SSL_CA_LOCATION` |
| `kafka.security.ssl.certificateLocation` | `KAFKA_SSL_CERTIFICATE_LOCATION` |
| `kafka.security.ssl.keyLocation` | `KAFKA_SSL_KEY_LOCATION` |
| `kafka.security.ssl.keyPassword` | `KAFKA_SSL_KEY_PASSWORD` (secret) |
| `kafka.security.ssl.endpointAlgorithm` | `KAFKA_SSL_ENDPOINT_ALGORITHM` |

### Production Configuration

```yaml
# values-production.yaml
websocketServer:
  replicaCount: 1
  autoscaling:
    enabled: false
    minReplicas: 1
    maxReplicas: 1

qualityMonitor:
  replicaCount: 2
  autoscaling:
    enabled: true

podDisruptionBudget:
  enabled: true

config:
  logLevel: warn

ingress:
  enabled: true
  className: nginx
  hosts:
    - host: dashflow.yourcompany.com
      paths:
        - path: /
          pathType: Prefix
  tls:
    - hosts:
        - dashflow.yourcompany.com
      secretName: dashflow-tls
```

## Upgrade

```bash
helm upgrade dashflow ./deploy/helm/dashflow -n dashflow -f my-values.yaml
```

## Uninstall

```bash
helm uninstall dashflow -n dashflow
```

## Development

### Lint Chart

```bash
helm lint ./deploy/helm/dashflow
```

### Template Preview

```bash
helm template dashflow ./deploy/helm/dashflow -n dashflow
```

### Dry Run

```bash
helm install dashflow ./deploy/helm/dashflow -n dashflow --dry-run --debug
```
