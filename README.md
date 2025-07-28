# Doomsday Certificate Monitor

A modern certificate monitoring and tracking system built with Rust and Next.js, providing real-time visibility into certificate expiration across multiple backends.

## 🔒 Features

- **Multi-Backend Support**: Monitor certificates from Vault, CredHub, Ops Manager, and direct TLS endpoints
- **Real-time Dashboard**: Modern web interface with real-time certificate status updates
- **CLI Tool**: Comprehensive command-line interface for automation and scripting
- **Flexible Authentication**: Support for no-auth and username/password authentication
- **Smart Notifications**: Built-in Slack and webhook notifications for expiring certificates
- **Production Ready**: Docker support, health checks, and comprehensive logging

## 🚀 Quick Start

### Using Docker Compose (Recommended)

1. **Clone and configure**:
   ```bash
   git clone https://github.com/tristanpoland/Doomsday-RS
   cd doomsday-rs
   cp ddayconfig.yml.example ddayconfig.yml
   # Edit ddayconfig.yml with your backend configurations
   ```

2. **Deploy**:
   ```bash
   docker-compose up -d
   ```

3. **Access**:
   - Web Dashboard: http://localhost:3000
   - API: http://localhost:8111

### Manual Installation

#### Prerequisites

- Rust 1.75+
- Node.js 18+
- OpenSSL development libraries

#### Backend Setup

```bash
# Build the Rust backend
cargo build --release

# Run the server
./target/release/doomsday-server -c ddayconfig.yml
```

#### Frontend Setup

```bash
cd frontend
npm install
npm run build
npm start
```

## ⚙️ Configuration

### Backend Configuration (`ddayconfig.yml`)

```yaml
backends:
  - type: vault
    name: production-vault
    refresh_interval: 30
    properties:
      url: https://vault.example.com
      token: "hvs.XXXXXXXXXXXXXXXXXXXXXX"
      mount_path: secret
      secret_path: /certificates

  - type: tlsclient
    name: web-endpoints
    refresh_interval: 15
    properties:
      targets:
        - host: example.com
          port: 443

server:
  port: 8111
  auth:
    type: userpass
    properties:
      users:
        admin: "secure_password"
      session_timeout: 60

notifications:
  doomsday_url: https://doomsday.example.com
  backend:
    type: slack
    properties:
      webhook_url: https://hooks.slack.com/services/YOUR/SLACK/WEBHOOK
```

### Supported Backends

#### HashiCorp Vault
```yaml
- type: vault
  properties:
    url: https://vault.example.com
    token: "vault_token"
    mount_path: secret  # KV mount path
    secret_path: /certificates  # Path to search for certificates
```

#### CredHub
```yaml
- type: credhub
  properties:
    url: https://credhub.example.com:8844
    client_id: doomsday_client
    client_secret: "client_secret"
```

#### Ops Manager
```yaml
- type: opsmgr
  properties:
    url: https://opsman.example.com
    username: admin
    password: "admin_password"
```

#### TLS Client (Direct TLS Endpoints)
```yaml
- type: tlsclient
  properties:
    targets:
      - host: example.com
        port: 443
      - host: api.example.com
        port: 443
        server_name: api.example.com  # Optional SNI
```

## 🖥️ CLI Usage

The CLI tool provides full API access for automation:

### Target Management
```bash
# Configure server target
doomsday target production https://doomsday.example.com:8111

# List configured targets
doomsday targets

# Authenticate (if required)
doomsday auth -u admin -p password
```

### Certificate Operations
```bash
# List all certificates
doomsday list

# Filter by expiry time
doomsday list --within 30d
doomsday list --beyond 1y

# Dashboard view
doomsday dashboard

# Refresh cache
doomsday refresh
doomsday refresh --backends vault,tlsclient
```

### Server Information
```bash
# Server info
doomsday info

# Scheduler status
doomsday scheduler
```

## 📊 Web Dashboard

The Next.js frontend provides:

- **Real-time Stats**: Certificate count by status (OK, Expiring Soon, Expired)
- **Filterable Table**: Search and filter certificates by subject, backend, or path
- **Status Indicators**: Color-coded certificate status with expiry information
- **Responsive Design**: Works on desktop and mobile devices
- **Authentication**: Seamless login integration when auth is enabled

## 🔔 Notifications

### Slack Integration
```yaml
notifications:
  backend:
    type: slack
    properties:
      webhook_url: https://hooks.slack.com/services/YOUR/SLACK/WEBHOOK
      channel: "#alerts"
      username: "Doomsday Bot"
  schedule:
    type: cron
    properties:
      expression: "0 9 * * *"  # Daily at 9 AM
```

### Custom Webhooks
```yaml
notifications:
  backend:
    type: shout
    properties:
      url: https://your-webhook-endpoint.com/alerts
```

## 🔐 Security

- **TLS Support**: Full TLS support for server and backend connections
- **Authentication**: Username/password authentication with session management
- **Token Security**: JWT-based session tokens with configurable expiry
- **Input Validation**: Comprehensive input validation and sanitization
- **Secure Defaults**: Security-first configuration defaults

## 🚀 Deployment

### Docker

```dockerfile
# Production deployment
FROM rust:1.75 as builder
# ... build steps ...

FROM ubuntu:22.04
# ... runtime setup ...
COPY --from=builder /app/target/release/doomsday-server ./
CMD ["./doomsday-server"]
```

### Kubernetes

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: doomsday
spec:
  replicas: 2
  selector:
    matchLabels:
      app: doomsday
  template:
    metadata:
      labels:
        app: doomsday
    spec:
      containers:
      - name: doomsday
        image: doomsday:latest
        ports:
        - containerPort: 8111
        volumeMounts:
        - name: config
          mountPath: /app/ddayconfig.yml
          subPath: ddayconfig.yml
```

### Environment Variables

- `RUST_LOG`: Logging level (debug, info, warn, error)
- `RUST_BACKTRACE`: Enable backtraces for debugging
- `BACKEND_URL`: Frontend backend URL override

## 🧪 Development

### Running Tests
```bash
cargo test
cd frontend && npm test
```

### Building
```bash
# Backend
cargo build --release

# Frontend
cd frontend
npm run build
```

### Development Mode
```bash
# Backend with hot-reload
cargo watch -x run

# Frontend with hot-reload
cd frontend
npm run dev
```

## 📈 Monitoring

### Health Checks
- **Backend**: `GET /v1/info` - Server health and version
- **Metrics**: Built-in Prometheus metrics support
- **Logging**: Structured JSON logging with configurable levels

### API Endpoints

- `GET /v1/info` - Server information
- `POST /v1/auth` - Authentication
- `GET /v1/cache` - List certificates
- `POST /v1/cache/refresh` - Refresh certificate cache
- `GET /v1/scheduler` - Scheduler status

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## 📄 License

This project is released into the public domain. See [LICENSE](LICENSE) for details.

## 🆘 Support

- **Issues**: Report bugs and feature requests on GitHub
- **Documentation**: Comprehensive API documentation available
- **Community**: Join our community discussions

---

**Doomsday Certificate Monitor** - Never let another certificate expire unexpectedly! 🔒✨