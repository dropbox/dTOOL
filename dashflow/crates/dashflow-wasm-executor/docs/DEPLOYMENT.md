# WASM Executor - Deployment Guide

**Production Deployment for HIPAA/SOC2 Compliant Code Execution**

This guide covers deploying the WASM executor in production environments with security, compliance, and reliability requirements.

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Configuration](#configuration)
3. [Deployment Methods](#deployment-methods)
4. [Security Hardening](#security-hardening)
5. [Monitoring Setup](#monitoring-setup)
6. [Backup and Recovery](#backup-and-recovery)
7. [Troubleshooting](#troubleshooting)

---

## Prerequisites

### System Requirements

**Minimum:**
- CPU: 2 cores
- RAM: 4GB
- Disk: 20GB
- OS: Linux (Ubuntu 22.04+, RHEL 8+, Debian 11+)

**Recommended (Production):**
- CPU: 4+ cores
- RAM: 16GB
- Disk: 100GB SSD
- OS: Linux (Ubuntu 22.04 LTS)

### Software Requirements

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install system dependencies (Ubuntu/Debian)
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev

# Install system dependencies (RHEL/CentOS)
sudo yum install -y gcc openssl-devel
```

---

## Configuration

### Environment Variables

Create `/etc/wasm-executor/config.env`:

```bash
# REQUIRED: JWT secret (minimum 32 characters)
JWT_SECRET="your-secure-random-secret-at-least-32-characters-long"

# REQUIRED: Audit log path
AUDIT_LOG_PATH="/var/log/wasm-executor/audit.log"

# OPTIONAL: Resource limits
MAX_FUEL=5000000           # CPU limit (5M instructions)
MAX_MEMORY_BYTES=67108864   # Memory limit (64MB)
MAX_STACK_BYTES=1048576     # Stack limit (1MB)
TIMEOUT_SECONDS=5           # Execution timeout

# OPTIONAL: Authentication
JWT_EXPIRY_MINUTES=60       # Token expiration (default: 60)
ENABLE_AUDIT_LOGGING=true   # Enable audit logs (default: true)

# OPTIONAL: Monitoring
METRICS_PORT=9090           # Prometheus metrics port
LOG_LEVEL=info              # info, debug, warn, error
```

### Generate Secure JWT Secret

```bash
# Generate a cryptographically secure random secret
openssl rand -base64 48 | tr -d '\n' > /etc/wasm-executor/jwt-secret.txt
chmod 600 /etc/wasm-executor/jwt-secret.txt

# Add to config.env
echo "JWT_SECRET=$(cat /etc/wasm-executor/jwt-secret.txt)" >> /etc/wasm-executor/config.env
```

---

## Deployment Methods

### Method 1: Systemd Service (Recommended)

#### Step 1: Build Release Binary

```bash
cd ~/dashflow
cargo build --release -p dashflow-wasm-executor

# Binary location: target/release/wasm-executor
```

#### Step 2: Install Binary

```bash
sudo cp target/release/wasm-executor /usr/local/bin/
sudo chmod +x /usr/local/bin/wasm-executor
```

#### Step 3: Create Service User

```bash
sudo useradd --system --no-create-home --shell /bin/false wasm-executor
```

#### Step 4: Create Directories

```bash
sudo mkdir -p /var/log/wasm-executor
sudo mkdir -p /etc/wasm-executor
sudo chown -R wasm-executor:wasm-executor /var/log/wasm-executor
sudo chmod 750 /var/log/wasm-executor
```

#### Step 5: Create Systemd Service

Create `/etc/systemd/system/wasm-executor.service`:

```ini
[Unit]
Description=WASM Executor Service (HIPAA/SOC2 Compliant)
Documentation=https://github.com/dashflow-ai/dashflow
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=wasm-executor
Group=wasm-executor

# Environment
EnvironmentFile=/etc/wasm-executor/config.env

# Execution
ExecStart=/usr/local/bin/wasm-executor
Restart=always
RestartSec=10
KillMode=process
TimeoutStopSec=30

# Security Hardening (Defense in Depth)
# Process isolation
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/log/wasm-executor

# Namespace isolation
PrivateDevices=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true

# Capability restrictions
CapabilityBoundingSet=
AmbientCapabilities=

# System call filtering
SystemCallFilter=@system-service
SystemCallFilter=~@privileged @resources @obsolete
SystemCallErrorNumber=EPERM

# Resource limits
LimitNOFILE=65536
LimitNPROC=512
LimitMEMLOCK=64M

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=wasm-executor

[Install]
WantedBy=multi-user.target
```

#### Step 6: Enable and Start Service

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable service (start on boot)
sudo systemctl enable wasm-executor

# Start service
sudo systemctl start wasm-executor

# Check status
sudo systemctl status wasm-executor

# View logs
sudo journalctl -u wasm-executor -f
```

---

### Method 2: Docker Container

#### Docker Build

Create `Dockerfile`:

```dockerfile
FROM rust:1.85-bookworm as builder

# Install dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy workspace
COPY . .

# Build release binary
RUN cargo build --release -p dashflow-wasm-executor

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create user
RUN useradd --system --no-create-home --shell /bin/false wasm-executor

# Create directories
RUN mkdir -p /var/log/wasm-executor && \
    chown -R wasm-executor:wasm-executor /var/log/wasm-executor

# Copy binary
COPY --from=builder /app/target/release/wasm-executor /usr/local/bin/

# Switch to non-root user
USER wasm-executor

# Expose metrics port
EXPOSE 9090

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:9090/health || exit 1

# Run
ENTRYPOINT ["/usr/local/bin/wasm-executor"]
```

#### Docker Compose

Create `docker-compose.yml`:

```yaml
version: '3.8'

services:
  wasm-executor:
    build: .
    container_name: wasm-executor
    restart: always
    environment:
      - JWT_SECRET=${JWT_SECRET}
      - AUDIT_LOG_PATH=/var/log/wasm-executor/audit.log
      - MAX_FUEL=5000000
      - MAX_MEMORY_BYTES=67108864
      - TIMEOUT_SECONDS=5
      - ENABLE_AUDIT_LOGGING=true
      - METRICS_PORT=9090
      - LOG_LEVEL=info
    volumes:
      - audit-logs:/var/log/wasm-executor
    ports:
      - "9090:9090"  # Prometheus metrics
    security_opt:
      - no-new-privileges:true
    cap_drop:
      - ALL
    read_only: true
    tmpfs:
      - /tmp
    networks:
      - wasm-net
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:9090/health"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 10s

  # Prometheus (optional - for metrics collection)
  prometheus:
    image: prom/prometheus:latest
    container_name: prometheus
    restart: always
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus-data:/prometheus
    ports:
      - "9091:9090"
    networks:
      - wasm-net

  # Grafana (optional - for visualization)
  grafana:
    image: grafana/grafana:latest
    container_name: grafana
    restart: always
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    volumes:
      - grafana-data:/var/lib/grafana
    ports:
      - "3000:3000"
    networks:
      - wasm-net
    depends_on:
      - prometheus

volumes:
  audit-logs:
  prometheus-data:
  grafana-data:

networks:
  wasm-net:
    driver: bridge
```

#### Deploy with Docker Compose

```bash
# Set JWT secret
export JWT_SECRET=$(openssl rand -base64 48 | tr -d '\n')

# Build and start
docker-compose up -d

# View logs
docker-compose logs -f wasm-executor

# Check health
docker-compose ps
```

---

## Security Hardening

### 1. TLS/SSL Configuration

**Generate Certificates:**

```bash
# Self-signed (development only)
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes

# Production: Use Let's Encrypt
sudo apt-get install certbot
sudo certbot certonly --standalone -d wasm-executor.example.com
```

### 2. Firewall Configuration

```bash
# Allow only necessary ports
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw allow 22/tcp   # SSH
sudo ufw allow 9090/tcp # Metrics (restrict to monitoring network)
sudo ufw enable
```

### 3. SELinux/AppArmor (Optional)

**AppArmor Profile** (`/etc/apparmor.d/usr.local.bin.wasm-executor`):

```
#include <tunables/global>

/usr/local/bin/wasm-executor {
  #include <abstractions/base>

  # Binary
  /usr/local/bin/wasm-executor mr,

  # Logs
  /var/log/wasm-executor/** rw,

  # Config
  /etc/wasm-executor/** r,

  # Deny everything else
  deny /home/** rw,
  deny /root/** rw,
  deny /etc/** w,
}
```

Enable:

```bash
sudo apparmor_parser -r /etc/apparmor.d/usr.local.bin.wasm-executor
```

### 4. Log Rotation

Create `/etc/logrotate.d/wasm-executor`:

```
/var/log/wasm-executor/*.log {
    daily
    missingok
    rotate 2555  # 7 years for HIPAA compliance
    compress
    delaycompress
    notifempty
    create 0640 wasm-executor wasm-executor
    sharedscripts
    postrotate
        systemctl reload wasm-executor > /dev/null 2>&1 || true
    endscript
}
```

---

## Monitoring Setup

### Prometheus Configuration

Create `prometheus.yml`:

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'wasm-executor'
    static_configs:
      - targets: ['localhost:9090']
    metrics_path: '/metrics'
```

### Key Metrics to Monitor

- `wasm_executions_total` - Total executions
- `wasm_executions_failed_total` - Failed executions
- `wasm_execution_duration_seconds` - Execution time
- `wasm_concurrent_executions` - Concurrent executions
- `wasm_fuel_consumed_total` - CPU usage
- `wasm_auth_success_total` - Successful authentications
- `wasm_auth_failed_total` - Failed authentications

### Alerts (Prometheus rules)

```yaml
groups:
  - name: wasm_executor
    rules:
      - alert: HighFailureRate
        expr: rate(wasm_executions_failed_total[5m]) > 0.1
        for: 5m
        annotations:
          summary: "High WASM execution failure rate"

      - alert: HighConcurrency
        expr: wasm_concurrent_executions > 100
        for: 1m
        annotations:
          summary: "High concurrent WASM executions"

      - alert: AuthFailures
        expr: rate(wasm_auth_failed_total[5m]) > 5
        for: 5m
        annotations:
          summary: "Multiple authentication failures detected"
```

---

## Backup and Recovery

### Audit Log Backup

```bash
# Daily backup script
#!/bin/bash
DATE=$(date +%Y%m%d)
tar -czf /backup/wasm-audit-$DATE.tar.gz /var/log/wasm-executor/audit.log
find /backup -name "wasm-audit-*.tar.gz" -mtime +2555 -delete  # Keep 7 years
```

### Configuration Backup

```bash
# Backup config
sudo cp -r /etc/wasm-executor /backup/wasm-executor-config-$(date +%Y%m%d)
```

---

## Troubleshooting

### Service Won't Start

```bash
# Check logs
sudo journalctl -u wasm-executor -n 100 --no-pager

# Check permissions
ls -la /var/log/wasm-executor
ls -la /etc/wasm-executor

# Test config
JWT_SECRET="test-secret-at-least-32-characters-long" /usr/local/bin/wasm-executor --test-config
```

### High Memory Usage

```bash
# Check resource limits in config.env
cat /etc/wasm-executor/config.env | grep MAX_MEMORY

# Monitor memory
watch -n 1 'ps aux | grep wasm-executor'
```

### Audit Log Growing Too Large

```bash
# Check log rotation
sudo logrotate -f /etc/logrotate.d/wasm-executor

# Verify rotation config
cat /etc/logrotate.d/wasm-executor
```

---

## Production Checklist

- [ ] JWT secret generated and secured (600 permissions)
- [ ] TLS certificates installed and configured
- [ ] Firewall rules configured (minimal ports open)
- [ ] Log rotation configured (7 year retention)
- [ ] Monitoring configured (Prometheus + Grafana)
- [ ] Alerts configured (PagerDuty/OpsGenie)
- [ ] Backup automation configured (daily)
- [ ] SELinux/AppArmor profile loaded
- [ ] Service user has minimal permissions
- [ ] All tests pass (cargo test)
- [ ] Load testing completed (100+ concurrent)
- [ ] Security audit completed
- [ ] Documentation reviewed
- [ ] Incident response plan documented

---

## Support

For issues or questions:
- GitHub: https://github.com/dashflow-ai/dashflow/issues
- Documentation: See docs/OPERATIONS.md for maintenance procedures
- Compliance: See docs/WASM_HIPAA_SOC2_COMPLIANCE.md for compliance details
