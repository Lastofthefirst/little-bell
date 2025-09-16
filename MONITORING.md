# Monitoring and Troubleshooting Guide

This guide covers monitoring Little Bell Email Tracking Server and troubleshooting common issues.

## Monitoring Overview

Little Bell provides several endpoints and mechanisms for monitoring:

- **Health Check**: `/health` - Basic service status
- **Metrics**: `/metrics` - Detailed performance metrics
- **Structured Logging**: JSON logs with tracing information
- **Process Monitoring**: Memory, CPU, and resource usage

## Health Monitoring

### Health Check Endpoint

```bash
curl https://your-domain.com/health
```

**Healthy Response:**
```json
{
  "status": "healthy",
  "service": "little-bell",
  "version": "0.1.0",
  "timestamp": "2024-01-01T12:00:00Z"
}
```

### Automated Health Checks

**Docker Health Check:**
```dockerfile
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1
```

**Systemd Service with Health Check:**
```ini
[Unit]
Description=Little Bell Email Tracking Server
After=network.target

[Service]
Type=simple
ExecStart=/app/little-bell
ExecStartPost=/bin/sleep 5
ExecReload=/bin/kill -HUP $MAINPID
Restart=always
RestartSec=10

# Health check
ExecStartPost=/bin/bash -c 'for i in {1..30}; do curl -f http://localhost:3000/health && exit 0; sleep 1; done; exit 1'

[Install]
WantedBy=multi-user.target
```

## Performance Metrics

### Metrics Endpoint

```bash
curl https://your-domain.com/metrics
```

**Sample Response:**
```json
{
  "service": "little-bell",
  "version": "0.1.0",
  "uptime_seconds": 86400,
  "database": {
    "path": "data/tracking.db",
    "size_bytes": 1048576
  },
  "memory_usage": {
    "rss_bytes": 12582912
  },
  "timestamp": "2024-01-01T12:00:00Z"
}
```

### Key Metrics to Monitor

| Metric | Normal Range | Alert Threshold |
|--------|--------------|-----------------|
| Memory Usage (RSS) | 10-50MB | >100MB |
| Database Size | Varies | Growth rate >10MB/day |
| Response Time | <100ms | >1000ms |
| Uptime | Continuous | <99% availability |

## Logging

### Log Format

Little Bell uses structured JSON logging:

```json
{
  "timestamp": "2024-01-01T12:00:00Z",
  "level": "INFO",
  "target": "little_bell",
  "message": "Email opened",
  "fields": {
    "tenant_id": "company_a",
    "email_id": 123,
    "ip_address": "192.168.1.1"
  }
}
```

### Log Levels

- **ERROR**: System errors, database failures
- **WARN**: Invalid requests, missing emails
- **INFO**: Normal operations, tracking events
- **DEBUG**: Detailed debugging information
- **TRACE**: Very verbose debugging

### Configuring Logging

```bash
# Environment variable
export RUST_LOG=little_bell=info,tower_http=debug

# Or in systemd service
Environment=RUST_LOG=little_bell=info
```

### Log Analysis Examples

**Count events by type:**
```bash
cat /var/log/little-bell.log | jq -r 'select(.message == "Email opened") | .fields.tenant_id' | sort | uniq -c
```

**Find errors:**
```bash
cat /var/log/little-bell.log | jq 'select(.level == "ERROR")'
```

**Track tenant activity:**
```bash
cat /var/log/little-bell.log | jq -r 'select(.fields.tenant_id == "company_a") | .message'
```

## Alerting

### Prometheus + AlertManager

**prometheus.yml:**
```yaml
scrape_configs:
  - job_name: 'little-bell'
    static_configs:
      - targets: ['localhost:3000']
    metrics_path: /metrics
    scrape_interval: 30s
```

**Sample Alerts:**
```yaml
# alerting_rules.yml
groups:
- name: little-bell
  rules:
  - alert: LittleBellDown
    expr: up{job="little-bell"} == 0
    for: 5m
    labels:
      severity: critical
    annotations:
      summary: "Little Bell service is down"
      
  - alert: HighMemoryUsage
    expr: little_bell_memory_bytes > 100000000
    for: 10m
    labels:
      severity: warning
    annotations:
      summary: "Little Bell memory usage is high"
      
  - alert: DatabaseGrowthHigh
    expr: increase(little_bell_database_size_bytes[24h]) > 10000000
    labels:
      severity: warning
    annotations:
      summary: "Database growing rapidly"
```

### Simple Script-Based Monitoring

```bash
#!/bin/bash
# monitor-little-bell.sh

HEALTH_URL="http://localhost:3000/health"
METRICS_URL="http://localhost:3000/metrics"
ALERT_EMAIL="admin@yourdomain.com"

# Check health
if ! curl -f $HEALTH_URL > /dev/null 2>&1; then
    echo "ALERT: Little Bell health check failed" | mail -s "Little Bell Down" $ALERT_EMAIL
    exit 1
fi

# Check memory usage
MEMORY=$(curl -s $METRICS_URL | jq -r '.memory_usage.rss_bytes')
if [ "$MEMORY" -gt 100000000 ]; then
    echo "ALERT: Little Bell memory usage high: ${MEMORY} bytes" | mail -s "Little Bell High Memory" $ALERT_EMAIL
fi

# Check database size
DB_SIZE=$(curl -s $METRICS_URL | jq -r '.database.size_bytes')
if [ "$DB_SIZE" -gt 1000000000 ]; then  # 1GB
    echo "ALERT: Little Bell database size large: ${DB_SIZE} bytes" | mail -s "Little Bell Large DB" $ALERT_EMAIL
fi
```

## Troubleshooting

### Common Issues

#### 1. Service Won't Start

**Symptoms:**
- Process exits immediately
- "Failed to bind to address" error

**Diagnosis:**
```bash
# Check if port is in use
netstat -tulpn | grep :3000

# Check logs
journalctl -u little-bell -n 50

# Test configuration
./little-bell --help  # If supported
```

**Solutions:**
- Change port with `PORT` environment variable
- Check firewall settings
- Verify database permissions

#### 2. Database Locked Error

**Symptoms:**
- "database is locked" errors in logs
- Slow response times

**Diagnosis:**
```bash
# Check for multiple processes
ps aux | grep little-bell

# Check database file permissions
ls -la data/tracking.db

# Check for stale lock files
ls -la data/tracking.db-*
```

**Solutions:**
- Ensure only one instance is running
- Stop service and remove .db-wal and .db-shm files
- Check file system health

#### 3. High Memory Usage

**Symptoms:**
- Memory usage >100MB
- OOM killer activating

**Diagnosis:**
```bash
# Check metrics
curl localhost:3000/metrics | jq .memory_usage

# Monitor with top/htop
top -p $(pgrep little-bell)

# Check for memory leaks
valgrind ./little-bell  # If debugging build available
```

**Solutions:**
- Restart service periodically
- Archive old database data
- Increase available memory
- Monitor for memory leaks

#### 4. Slow Response Times

**Symptoms:**
- API calls taking >1 second
- Timeout errors

**Diagnosis:**
```bash
# Test response times
time curl localhost:3000/health

# Check database size
ls -lh data/tracking.db

# Monitor disk I/O
iostat -x 1
```

**Solutions:**
- Archive old tracking data
- Optimize database (VACUUM)
- Move to faster storage (SSD)
- Consider database sharding

#### 5. Missing Tracking Events

**Symptoms:**
- Email opens/clicks not recorded
- Dashboard shows zero events

**Diagnosis:**
```bash
# Check recent logs for errors
tail -f /var/log/little-bell.log | grep ERROR

# Test tracking manually
curl -v "localhost:3000/test/pixel/1.gif"

# Check database content
sqlite3 data/tracking.db "SELECT COUNT(*) FROM events;"
```

**Solutions:**
- Verify email ID exists
- Check tenant ID in URLs
- Ensure database is writable
- Verify tracking URLs are correct

### Performance Optimization

#### Database Optimization

```sql
-- Run these commands in SQLite to optimize performance
PRAGMA optimize;
VACUUM;
REINDEX;

-- Check database statistics
.schema
.dbinfo
```

#### System Optimization

```bash
# File descriptor limits
ulimit -n 65536

# Memory settings for SQLite
echo 'vm.dirty_ratio = 5' >> /etc/sysctl.conf
echo 'vm.dirty_background_ratio = 2' >> /etc/sysctl.conf
sysctl -p
```

### Backup and Recovery

#### Automated Backup

```bash
#!/bin/bash
# backup-database.sh

BACKUP_DIR="/var/backups/little-bell"
DB_PATH="data/tracking.db"
DATE=$(date +%Y%m%d_%H%M%S)

mkdir -p $BACKUP_DIR

# Online backup using SQLite backup API
sqlite3 $DB_PATH ".backup $BACKUP_DIR/tracking_$DATE.db"

# Compress old backups
find $BACKUP_DIR -name "*.db" -mtime +1 -exec gzip {} \;

# Clean up old backups (keep 30 days)
find $BACKUP_DIR -name "*.gz" -mtime +30 -delete

echo "Backup completed: $BACKUP_DIR/tracking_$DATE.db"
```

#### Recovery Procedure

```bash
# 1. Stop the service
sudo systemctl stop little-bell

# 2. Backup current database
cp data/tracking.db data/tracking.db.corrupted

# 3. Restore from backup
cp /var/backups/little-bell/tracking_20240101_120000.db data/tracking.db

# 4. Verify database integrity
sqlite3 data/tracking.db "PRAGMA integrity_check;"

# 5. Start the service
sudo systemctl start little-bell

# 6. Verify service health
curl localhost:3000/health
```

### Load Testing

#### Simple Load Test

```bash
#!/bin/bash
# load-test.sh

URL="http://localhost:3000"
CONCURRENT=10
DURATION=60

echo "Starting load test: $CONCURRENT concurrent for ${DURATION}s"

# Test tracking pixels
for i in $(seq 1 $CONCURRENT); do
    (
        for j in $(seq 1 100); do
            curl -s "$URL/test/pixel/$j.gif" > /dev/null
            sleep 0.1
        done
    ) &
done

wait
echo "Load test completed"
```

#### Advanced Load Testing with Apache Bench

```bash
# Test health endpoint
ab -n 1000 -c 10 http://localhost:3000/health

# Test pixel tracking
ab -n 1000 -c 10 http://localhost:3000/test/pixel/1.gif

# Test with POST requests (create emails)
ab -n 100 -c 5 -p email.json -T application/json http://localhost:3000/test/emails
```

### Monitoring Dashboard

#### Grafana Dashboard JSON

```json
{
  "dashboard": {
    "title": "Little Bell Monitoring",
    "panels": [
      {
        "title": "Memory Usage",
        "type": "stat",
        "targets": [
          {
            "expr": "little_bell_memory_bytes",
            "legendFormat": "Memory (MB)"
          }
        ]
      },
      {
        "title": "Database Size",
        "type": "stat",
        "targets": [
          {
            "expr": "little_bell_database_size_bytes",
            "legendFormat": "DB Size (MB)"
          }
        ]
      },
      {
        "title": "Request Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(http_requests_total[5m])",
            "legendFormat": "Requests/sec"
          }
        ]
      }
    ]
  }
}
```

This comprehensive monitoring and troubleshooting guide should help you maintain a healthy Little Bell deployment in production.