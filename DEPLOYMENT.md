# Production Deployment Guide

This guide covers deploying Little Bell Email Tracking Server in production environments.

## Prerequisites

- Docker and Docker Compose (recommended)
- Or Rust 1.75+ for binary deployment
- Reverse proxy (Nginx, Caddy, etc.) for HTTPS

## Quick Production Deployment

### Option 1: Docker Compose (Recommended)

1. **Clone the repository:**
```bash
git clone https://github.com/Lastofthefirst/little-bell.git
cd little-bell
```

2. **Create production environment file:**
```bash
cat > .env << EOF
PORT=3000
DATABASE_URL=sqlite:data/tracking.db
BASE_URL=https://yourdomain.com
RUST_LOG=little_bell=info
EOF
```

3. **Start the service:**
```bash
docker-compose up -d
```

### Option 2: Binary Deployment

1. **Build for production:**
```bash
cargo build --release --target x86_64-unknown-linux-musl
```

2. **Deploy binary:**
```bash
scp target/x86_64-unknown-linux-musl/release/little-bell server:/app/
```

3. **Create systemd service:**
```ini
# /etc/systemd/system/little-bell.service
[Unit]
Description=Little Bell Email Tracking Server
After=network.target

[Service]
Type=simple
User=appuser
WorkingDirectory=/app
ExecStart=/app/little-bell
Restart=always
RestartSec=10
Environment=PORT=3000
Environment=DATABASE_URL=sqlite:data/tracking.db
Environment=BASE_URL=https://yourdomain.com
Environment=RUST_LOG=little_bell=info

[Install]
WantedBy=multi-user.target
```

4. **Start service:**
```bash
sudo systemctl enable little-bell
sudo systemctl start little-bell
```

## Reverse Proxy Configuration

### Nginx

```nginx
server {
    listen 80;
    server_name track.yourdomain.com;
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name track.yourdomain.com;
    
    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;
    
    location / {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # Caching for tracking pixels
        location ~* \\.gif$ {
            proxy_pass http://localhost:3000;
            proxy_cache_bypass $http_pragma;
            add_header Cache-Control "no-store, no-cache, must-revalidate";
        }
    }
}
```

### Caddy

```caddyfile
track.yourdomain.com {
    reverse_proxy localhost:3000
    
    @pixels path *.gif
    handle @pixels {
        reverse_proxy localhost:3000
        header Cache-Control "no-store, no-cache, must-revalidate"
    }
}
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `3000` | Server port |
| `DATABASE_URL` | `sqlite:data/tracking.db` | Database connection string |
| `BASE_URL` | `http://localhost:3000` | Base URL for tracking links |
| `RUST_LOG` | | Log level (e.g., `little_bell=info`) |

## Monitoring & Health Checks

### Health Check Endpoint
```bash
curl https://track.yourdomain.com/health
```

Expected response:
```json
{
  "status": "healthy",
  "service": "little-bell",
  "version": "0.1.0",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

### Metrics Endpoint
```bash
curl https://track.yourdomain.com/metrics
```

Response includes:
- Memory usage
- Database size
- Uptime
- Timestamp

### Prometheus Integration

Add to your `prometheus.yml`:
```yaml
scrape_configs:
  - job_name: 'little-bell'
    static_configs:
      - targets: ['track.yourdomain.com:443']
    scheme: https
    metrics_path: /metrics
    scrape_interval: 30s
```

## Database Management

### Backup
```bash
# SQLite database can be backed up with simple file copy
cp data/tracking.db data/tracking.db.backup.$(date +%Y%m%d_%H%M%S)
```

### Automated Backup Script
```bash
#!/bin/bash
# backup-little-bell.sh
BACKUP_DIR="/var/backups/little-bell"
DB_PATH="/app/data/tracking.db"
DATE=$(date +%Y%m%d_%H%M%S)

mkdir -p $BACKUP_DIR
cp $DB_PATH $BACKUP_DIR/tracking.db.backup.$DATE

# Keep only last 30 backups
find $BACKUP_DIR -name "tracking.db.backup.*" -mtime +30 -delete
```

### Restore
```bash
# Stop service
sudo systemctl stop little-bell

# Restore database
cp backup/tracking.db.backup.20240101_120000 data/tracking.db

# Start service
sudo systemctl start little-bell
```

## Performance Tuning

### SQLite Optimization

The application automatically uses these SQLite optimizations:
- WAL mode for better concurrency
- Proper indexes on frequently queried columns
- Connection pooling (single connection for SQLite)

### OS-Level Tuning

```bash
# Increase file descriptor limits
echo "* soft nofile 65536" >> /etc/security/limits.conf
echo "* hard nofile 65536" >> /etc/security/limits.conf

# Optimize for small files (SQLite)
echo "vm.dirty_ratio = 5" >> /etc/sysctl.conf
echo "vm.dirty_background_ratio = 2" >> /etc/sysctl.conf
```

## Security Considerations

### Network Security
- Use HTTPS in production
- Consider IP whitelisting for dashboard access
- Use a firewall to restrict direct access to port 3000

### Application Security
- The application uses parameterized queries (SQL injection protection)
- Template escaping prevents XSS
- No authentication by default (security through obscurity)
- Consider adding API keys for write operations if needed

### Data Privacy
- GDPR compliant (minimal data collection)
- IP addresses and user agents are stored but can be disabled
- Data retention policies should be implemented as needed

## Scaling

### Horizontal Scaling
- Multiple instances can run behind a load balancer
- Each instance should have its own SQLite database
- Consider migrating to PostgreSQL for shared storage

### Load Balancing
```nginx
upstream little_bell {
    server 127.0.0.1:3001;
    server 127.0.0.1:3002;
    server 127.0.0.1:3003;
}

server {
    location / {
        proxy_pass http://little_bell;
    }
}
```

## Troubleshooting

### Common Issues

1. **Database locked error:**
   - Ensure only one instance is running
   - Check file permissions on database file

2. **High memory usage:**
   - Monitor metrics endpoint
   - Consider restarting service periodically

3. **Slow response times:**
   - Check database size and consider archiving old data
   - Monitor disk I/O

### Log Analysis
```bash
# View logs with Docker
docker-compose logs -f little-bell

# View logs with systemd
journalctl -u little-bell -f

# Parse JSON logs
tail -f /var/log/little-bell.log | jq .
```

## Free Hosting Options

### Fly.io
```bash
# Install flyctl
curl -L https://fly.io/install.sh | sh

# Deploy
fly launch
fly deploy
```

### Railway
```bash
# Connect GitHub repo and deploy automatically
# Set environment variables in Railway dashboard
```

### DigitalOcean App Platform
```yaml
# .do/app.yaml
name: little-bell
services:
- name: web
  source_dir: /
  github:
    repo: your-username/little-bell
    branch: main
  run_command: ./target/release/little-bell
  environment_slug: rust
  instance_count: 1
  instance_size_slug: basic-xxs
  envs:
  - key: BASE_URL
    value: https://your-app.ondigitalocean.app
```

## Cost Estimation

| Provider | Configuration | Monthly Cost |
|----------|---------------|--------------|
| DigitalOcean | 1GB RAM, 1 vCPU | $5 |
| Fly.io | Shared CPU, 256MB | $0 (free tier) |
| Railway | 512MB RAM | $5 |
| VPS | 1GB RAM | $3-10 |

The application typically uses 10-20MB of memory, making it suitable for the smallest hosting tiers.