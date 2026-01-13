# WASM Executor - Operations Manual

**Day-to-day Operations and Maintenance**

---

## Daily Operations

### Health Checks

```bash
# Service status
sudo systemctl status wasm-executor

# Check logs for errors
sudo journalctl -u wasm-executor --since "1 hour ago" | grep -i error

# Prometheus metrics
curl http://localhost:9090/metrics | grep wasm_executions
```

### Monitor Key Metrics

1. **Execution Rate**: Should be stable, spikes indicate load changes
2. **Failure Rate**: Should be <1%, higher indicates issues
3. **Concurrent Executions**: Monitor for capacity planning
4. **Auth Failures**: >5/min indicates potential attack

---

## Weekly Maintenance

### Log Review

```bash
# Check audit logs
sudo tail -1000 /var/log/wasm-executor/audit.log | jq '.'

# Check for repeated failures
sudo grep "FAILED" /var/log/wasm-executor/audit.log | tail -100
```

### Performance Review

```bash
# Check resource usage
ps aux | grep wasm-executor
free -h
df -h /var/log
```

---

## Monthly Maintenance

### Security Updates

```bash
# Update system packages
sudo apt-get update && sudo apt-get upgrade -y

# Rebuild with latest dependencies
cd ~/dashflow
cargo update
cargo build --release -p dashflow-wasm-executor
sudo systemctl restart wasm-executor
```

### Certificate Renewal

```bash
# Renew Let's Encrypt certs (auto-renewal should handle this)
sudo certbot renew --dry-run
```

---

## Incident Response

### High Failure Rate Alert

1. Check logs: `sudo journalctl -u wasm-executor -n 1000`
2. Identify failing pattern (invalid WASM, fuel exhaustion, timeout)
3. If malicious: Block source IP, review auth logs
4. If legitimate: Increase resource limits or optimize WASM modules

### Service Down Alert

1. Check service: `sudo systemctl status wasm-executor`
2. Restart: `sudo systemctl restart wasm-executor`
3. Check logs: `sudo journalctl -u wasm-executor -n 500`
4. If crash: File bug report with logs

### High Memory Usage Alert

1. Check memory: `free -h`
2. Check concurrent executions: `curl localhost:9090/metrics | grep concurrent`
3. If excessive: Reduce MAX_MEMORY_BYTES or MAX_FUEL in config
4. Restart service: `sudo systemctl restart wasm-executor`

---

## Capacity Planning

### When to Scale

**Scale up (more resources) when:**
- Concurrent executions consistently >80% of capacity
- Execution duration increasing (>100ms p95)
- CPU usage >80% sustained

**Scale out (more instances) when:**
- Single instance at capacity
- Need higher availability
- Geographic distribution required

---

## Backup and Recovery

### Daily Backup Verification

```bash
# Verify backups exist
ls -lh /backup/wasm-audit-*.tar.gz | tail -7

# Test restore (dry-run)
tar -tzf /backup/wasm-audit-$(date +%Y%m%d).tar.gz
```

### Disaster Recovery

```bash
# 1. Stop service
sudo systemctl stop wasm-executor

# 2. Restore audit logs
sudo tar -xzf /backup/wasm-audit-YYYYMMDD.tar.gz -C /

# 3. Restore config
sudo cp -r /backup/wasm-executor-config-YYYYMMDD /etc/wasm-executor

# 4. Start service
sudo systemctl start wasm-executor
```

---

## Compliance Maintenance

### HIPAA Audit Log Retention

- Retain logs for 7 years (2,555 days)
- Verify log rotation: `/etc/logrotate.d/wasm-executor`
- Monthly: Check backup integrity

### SOC 2 Control Monitoring

- Weekly: Review access logs
- Monthly: Vulnerability scan (`cargo audit`)
- Quarterly: Penetration testing
- Annual: SOC 2 audit

---

## Common Tasks

### Rotate JWT Secret

```bash
# 1. Generate new secret
NEW_SECRET=$(openssl rand -base64 48 | tr -d '\n')

# 2. Update config
sudo sed -i "s/JWT_SECRET=.*/JWT_SECRET=$NEW_SECRET/" /etc/wasm-executor/config.env

# 3. Restart service
sudo systemctl restart wasm-executor

# 4. Notify clients (tokens will be invalidated)
```

### Adjust Resource Limits

```bash
# 1. Edit config
sudo vi /etc/wasm-executor/config.env

# Example changes:
# MAX_FUEL=10000000  # Double CPU limit
# MAX_MEMORY_BYTES=134217728  # Double memory limit

# 2. Restart service
sudo systemctl restart wasm-executor
```

### Enable Debug Logging

```bash
# 1. Set log level
sudo sed -i 's/LOG_LEVEL=.*/LOG_LEVEL=debug/' /etc/wasm-executor/config.env

# 2. Restart service
sudo systemctl restart wasm-executor

# 3. View detailed logs
sudo journalctl -u wasm-executor -f

# 4. Revert to info level when done
sudo sed -i 's/LOG_LEVEL=.*/LOG_LEVEL=info/' /etc/wasm-executor/config.env
sudo systemctl restart wasm-executor
```

---

## Monitoring Dashboard

### Grafana Queries (PromQL)

```promql
# Execution rate (req/sec)
rate(wasm_executions_total[5m])

# Failure rate (%)
rate(wasm_executions_failed_total[5m]) / rate(wasm_executions_total[5m]) * 100

# P95 execution duration
histogram_quantile(0.95, rate(wasm_execution_duration_seconds_bucket[5m]))

# Concurrent executions
wasm_concurrent_executions

# Auth success rate (%)
rate(wasm_auth_success_total[5m]) / (rate(wasm_auth_success_total[5m]) + rate(wasm_auth_failed_total[5m])) * 100
```

---

## Contacts

- **On-Call Engineer**: [PagerDuty rotation]
- **Security Team**: security@example.com
- **Compliance Officer**: compliance@example.com
- **GitHub Issues**: https://github.com/dashflow-ai/dashflow/issues
