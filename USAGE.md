# Little Bell - Lightweight Email Tracking Server

A high-performance, multi-tenant email tracking server built in Rust. Track email opens and clicks with minimal resource usage and zero-configuration deployment.

## Features

- **Open Tracking**: Invisible 1x1 pixel tracking
- **Click Tracking**: URL redirection with event logging
- **Multi-Tenant**: Isolated data per tenant via URL paths
- **Dashboard**: Basic web interface for viewing statistics
- **Zero Config**: Runs with sensible defaults
- **Lightweight**: ~10-20MB memory usage
- **Fast**: Built with Rust and Axum for high performance

## Quick Start

### 1. Build and Run

```bash
# Clone repository
git clone <repository-url>
cd little-bell

# Build and run
cargo build --release
./target/release/little-bell
```

The server starts on `http://localhost:3000` by default.

### 2. Create an Email Record

```bash
curl -X POST http://localhost:3000/your_tenant/emails \
  -H "Content-Type: application/json" \
  -d '{"subject": "Welcome Email", "recipient": "user@example.com"}'
```

Response:
```json
{
  "email_id": 1,
  "tracking_pixel_url": "http://localhost:3000/your_tenant/pixel/1.gif"
}
```

### 3. Add Tracking to Your Emails

#### Open Tracking
Add this invisible pixel to your email HTML:

```html
<img src="http://localhost:3000/your_tenant/pixel/1.gif" width="1" height="1" style="display:block" />
```

#### Click Tracking
Replace your links with tracking URLs:

```
http://localhost:3000/your_tenant/click/1?url=https%3A%2F%2Fexample.com%2Fyour-link
```

### 4. View Dashboard

Visit: `http://localhost:3000/your_tenant/dashboard`

## Configuration

Set environment variables to customize:

```bash
PORT=3000                                    # Server port
DATABASE_URL=sqlite:data/tracking.db        # Database location
BASE_URL=http://localhost:3000              # Base URL for tracking links
```

## API Endpoints

### Core Tracking
- `GET /:tenant_id/pixel/:email_id.gif` - Open tracking pixel
- `GET /:tenant_id/click/:email_id?url=<url>` - Click tracking redirect
- `GET /:tenant_id/dashboard` - Statistics dashboard

### Management
- `POST /:tenant_id/emails` - Create email record
- `GET /:tenant_id/click-url/:email_id?url=<url>` - Generate click tracking URL
- `GET /health` - Health check

## Multi-Tenant Usage

Each tenant is isolated by URL path:

```bash
# Tenant A
curl -X POST http://localhost:3000/company_a/emails -d '{"subject": "Newsletter"}'

# Tenant B  
curl -X POST http://localhost:3000/company_b/emails -d '{"subject": "Promo"}'
```

Data is completely isolated between tenants.

## Deployment

### Single Binary
```bash
# Build for production
cargo build --release --target x86_64-unknown-linux-musl

# Deploy anywhere
scp target/x86_64-unknown-linux-musl/release/little-bell server:/app/
```

### Docker
```dockerfile
FROM scratch
COPY little-bell /app/little-bell
EXPOSE 3000
CMD ["/app/little-bell"]
```

### Free Hosting Options
- **Fly.io**: Free tier with 3 shared VMs
- **Railway**: Free tier with automatic deploys
- **DigitalOcean**: $5/month basic droplet

## Performance

- **Memory**: ~10-20MB typical usage
- **Database**: SQLite with WAL mode for concurrency
- **Throughput**: Thousands of requests per second
- **Cold Start**: < 100ms initialization

## Development

```bash
# Run tests
cargo test

# Run with hot reload
cargo watch -x run

# Check performance
cargo build --release
./target/release/little-bell
```

## Security

- No authentication by default (security through obscurity)
- SQL injection prevention via parameterized queries
- XSS protection with template escaping
- GDPR-compliant minimal data collection

## License

MIT License - see LICENSE file for details.