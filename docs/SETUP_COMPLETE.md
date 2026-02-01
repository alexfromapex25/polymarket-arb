# Polymarket Arbitrage Bot - Complete Setup Guide

This guide covers everything you need to deploy and operate the BTC 15-minute Polymarket arbitrage bot in production.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Wallet Setup](#wallet-setup)
3. [Environment Configuration](#environment-configuration)
4. [Local Development](#local-development)
5. [Cloud Deployment - AWS](#cloud-deployment---aws)
6. [Cloud Deployment - GCP](#cloud-deployment---gcp)
7. [Cloud Deployment - DigitalOcean](#cloud-deployment---digitalocean)
8. [System Tuning](#system-tuning)
9. [Monitoring Setup](#monitoring-setup)
10. [Troubleshooting](#troubleshooting)
11. [Risk Management](#risk-management)
12. [Performance Benchmarking](#performance-benchmarking)
13. [Maintenance](#maintenance)

---

## Prerequisites

### Software Requirements

- **Rust 1.75+**: Install via [rustup](https://rustup.rs/)
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  rustup update stable
  ```

- **Docker** (optional, for containerized deployment)
  ```bash
  # macOS
  brew install --cask docker

  # Ubuntu/Debian
  curl -fsSL https://get.docker.com -o get-docker.sh
  sudo sh get-docker.sh
  ```

- **Git**: For version control

### Polymarket Requirements

1. **Polygon Wallet**: EOA wallet with private key
2. **USDC on Polygon**: Minimum $100 recommended for testing
3. **Polymarket Account**: Deposit USDC and enable trading

---

## Wallet Setup

### Option 1: New Wallet (Recommended for bots)

Generate a dedicated trading wallet:

```bash
# Using cast (from Foundry)
cast wallet new

# Or using Node.js
node -e "console.log(require('crypto').randomBytes(32).toString('hex'))"
```

**Important**: Save the private key securely. Never share it or commit it to git.

### Option 2: Existing Wallet

Export your private key from MetaMask or another wallet. The key should be 64 hex characters (optionally prefixed with `0x`).

### Funding Your Wallet

1. Bridge USDC from Ethereum to Polygon using:
   - [Polygon Bridge](https://wallet.polygon.technology/bridge)
   - [Hop Protocol](https://hop.exchange/)

2. Deposit USDC to Polymarket:
   - Go to [polymarket.com](https://polymarket.com)
   - Connect your wallet
   - Deposit USDC

3. Verify deposit appears in your Polymarket balance

### Signature Types

| Type | Value | Description |
|------|-------|-------------|
| EOA | 0 | Standard externally owned account (most common) |
| Proxy | 1 | Magic.link proxy wallet (requires funder address) |
| GnosisSafe | 2 | Multi-sig wallet |

---

## Environment Configuration

Create a `.env` file from the example:

```bash
cp .env.example .env
```

### Required Variables

```bash
# Wallet (REQUIRED)
POLYMARKET_PRIVATE_KEY=0x...your64charkey...

# Signature type (default: 0 for EOA)
POLYMARKET_SIGNATURE_TYPE=0

# For Magic.link users only
# POLYMARKET_FUNDER=0x...your_funder_address...
```

### Trading Parameters

```bash
# Target combined cost threshold (default: 0.991 = 0.9% profit)
TARGET_PAIR_COST=0.991

# Shares per trade (minimum 5)
ORDER_SIZE=10

# Order type: FOK (Fill or Kill), FAK, or GTC
ORDER_TYPE=FOK

# Seconds between trades
COOLDOWN_SECONDS=10
```

### Low-Latency Tuning

```bash
# HTTP settings
HTTP_TIMEOUT_MS=2000
HTTP_POOL_SIZE=10

# Order execution timing
ORDER_TIMEOUT_MS=500
ORDER_POLL_INTERVAL_MS=50

# WebSocket settings
WS_RECONNECT_MAX_DELAY_S=30
WS_HEARTBEAT_INTERVAL_S=30
```

### Metrics & Monitoring

```bash
# Enable Prometheus metrics
METRICS_ENABLED=true
METRICS_PORT=9090
```

### Operation Modes

```bash
# Simulation mode (no real orders)
DRY_RUN=true

# Starting balance for simulation
SIM_BALANCE=100

# WebSocket mode (lower latency)
USE_WSS=true
```

### All Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `POLYMARKET_PRIVATE_KEY` | - | Wallet private key (required) |
| `POLYMARKET_SIGNATURE_TYPE` | 0 | 0=EOA, 1=Proxy, 2=GnosisSafe |
| `POLYMARKET_FUNDER` | - | Funder address for Magic.link |
| `POLYMARKET_CLOB_URL` | https://clob.polymarket.com | CLOB API URL |
| `POLYMARKET_WS_URL` | wss://ws-subscriptions-clob.polymarket.com | WebSocket URL |
| `TARGET_PAIR_COST` | 0.991 | Max combined cost for arbitrage |
| `ORDER_SIZE` | 5 | Shares per side |
| `ORDER_TYPE` | FOK | FOK, FAK, or GTC |
| `BALANCE_MARGIN` | 1.2 | Safety margin multiplier |
| `DRY_RUN` | true | Simulation mode |
| `SIM_BALANCE` | 100 | Simulation starting balance |
| `COOLDOWN_SECONDS` | 10 | Seconds between trades |
| `USE_WSS` | false | Use WebSocket for data |
| `HTTP_TIMEOUT_MS` | 2000 | HTTP request timeout |
| `HTTP_POOL_SIZE` | 10 | HTTP connection pool size |
| `ORDER_TIMEOUT_MS` | 500 | Order status timeout |
| `ORDER_POLL_INTERVAL_MS` | 50 | Order polling interval |
| `WS_RECONNECT_MAX_DELAY_S` | 30 | Max WebSocket reconnect delay |
| `WS_HEARTBEAT_INTERVAL_S` | 30 | WebSocket heartbeat interval |
| `METRICS_ENABLED` | true | Enable Prometheus metrics |
| `METRICS_PORT` | 9090 | Prometheus metrics port |
| `PORT` | 8080 | HTTP server port |
| `RUST_LOG` | info | Log level |

---

## Local Development

### Build

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release
```

### Test

```bash
# Run all tests
cargo test

# Run with logging
RUST_LOG=debug cargo test -- --nocapture
```

### Run

```bash
# Check configuration
cargo run -- check-config

# Check balance
cargo run -- check-balance

# Discover current market
cargo run -- discover-market

# Run in simulation mode
cargo run -- run --dry-run

# Run with WebSocket (lower latency)
cargo run -- run --websocket --dry-run

# Test WebSocket connection
cargo run -- ws-test

# Run latency benchmark
cargo run -- benchmark
```

### Development Tips

1. Always start in `DRY_RUN=true` mode
2. Use `RUST_LOG=debug` for verbose output
3. Test WebSocket connection with `ws-test` command
4. Run benchmarks to measure your network latency

---

## Cloud Deployment - AWS

### EC2 Instance Setup

1. **Launch Instance**:
   - AMI: Ubuntu 22.04 LTS
   - Instance type: `t3.small` or `c6i.large` for production
   - Region: `us-east-1` (closest to Polymarket servers)
   - Storage: 20GB gp3

2. **Security Group**:
   ```
   Inbound:
   - SSH (22): Your IP
   - HTTP (8080): Anywhere (health checks)
   - Prometheus (9090): Your monitoring server

   Outbound:
   - All traffic: Anywhere
   ```

3. **Connect and Setup**:
   ```bash
   ssh -i your-key.pem ubuntu@<instance-ip>

   # Install Rust
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env

   # Install build dependencies
   sudo apt update
   sudo apt install -y build-essential pkg-config libssl-dev

   # Clone and build
   git clone <your-repo>
   cd polymarket-arb
   cargo build --release

   # Configure
   cp .env.example .env
   nano .env  # Add your private key
   ```

4. **Run as Service**:
   ```bash
   sudo nano /etc/systemd/system/polymarket-arb.service
   ```

   ```ini
   [Unit]
   Description=Polymarket Arbitrage Bot
   After=network.target

   [Service]
   Type=simple
   User=ubuntu
   WorkingDirectory=/home/ubuntu/polymarket-arb
   ExecStart=/home/ubuntu/polymarket-arb/target/release/polymarket-arb run --websocket
   Restart=always
   RestartSec=5
   Environment=RUST_LOG=info

   [Install]
   WantedBy=multi-user.target
   ```

   ```bash
   sudo systemctl enable polymarket-arb
   sudo systemctl start polymarket-arb
   sudo systemctl status polymarket-arb
   ```

### AWS Best Practices

- Use IAM roles for AWS services (CloudWatch, Secrets Manager)
- Store private key in AWS Secrets Manager
- Enable CloudWatch logs for monitoring
- Use Auto Scaling for high availability
- Consider Spot instances for cost savings (with fallback)

---

## Cloud Deployment - GCP

### Compute Engine Setup

1. **Create VM**:
   ```bash
   gcloud compute instances create polymarket-arb \
     --machine-type=e2-small \
     --zone=us-east1-b \
     --image-family=ubuntu-2204-lts \
     --image-project=ubuntu-os-cloud \
     --boot-disk-size=20GB
   ```

2. **Firewall Rules**:
   ```bash
   gcloud compute firewall-rules create allow-arb-health \
     --allow=tcp:8080,tcp:9090 \
     --source-ranges=0.0.0.0/0 \
     --description="Allow health check and metrics"
   ```

3. **SSH and Setup**:
   ```bash
   gcloud compute ssh polymarket-arb --zone=us-east1-b

   # Follow same setup as AWS
   ```

### GCP Best Practices

- Use Secret Manager for private key
- Enable Cloud Logging
- Use Managed Instance Groups for HA
- Consider Preemptible VMs for cost savings

---

## Cloud Deployment - DigitalOcean

### Droplet Setup

1. **Create Droplet**:
   - Image: Ubuntu 22.04
   - Plan: Basic $12/mo (2GB RAM)
   - Datacenter: NYC1 or NYC3
   - Enable monitoring

2. **Setup**:
   ```bash
   ssh root@<droplet-ip>

   # Create non-root user
   adduser polymarket
   usermod -aG sudo polymarket
   su - polymarket

   # Install Rust and build (same as AWS)
   ```

3. **Firewall**:
   ```bash
   sudo ufw allow 22
   sudo ufw allow 8080
   sudo ufw allow 9090
   sudo ufw enable
   ```

---

## System Tuning

### Network Stack Optimization

Run the system tuning script:

```bash
sudo ./scripts/tune-system.sh
```

Or apply manually:

```bash
# Increase socket buffer sizes
sudo sysctl -w net.core.rmem_max=16777216
sudo sysctl -w net.core.wmem_max=16777216
sudo sysctl -w net.core.rmem_default=1048576
sudo sysctl -w net.core.wmem_default=1048576

# TCP optimizations
sudo sysctl -w net.ipv4.tcp_rmem="4096 1048576 16777216"
sudo sysctl -w net.ipv4.tcp_wmem="4096 1048576 16777216"
sudo sysctl -w net.ipv4.tcp_nodelay=1
sudo sysctl -w net.ipv4.tcp_low_latency=1

# Connection handling
sudo sysctl -w net.core.somaxconn=65535
sudo sysctl -w net.ipv4.tcp_max_syn_backlog=65535
```

Make permanent by adding to `/etc/sysctl.conf`.

### File Descriptor Limits

```bash
# Check current limits
ulimit -n

# Increase for current session
ulimit -n 65535

# Make permanent
echo "* soft nofile 65535" | sudo tee -a /etc/security/limits.conf
echo "* hard nofile 65535" | sudo tee -a /etc/security/limits.conf
```

### CPU Affinity (Optional)

For dedicated servers, pin the process to specific cores:

```bash
# Run on cores 0-1
taskset -c 0,1 ./target/release/polymarket-arb run
```

---

## Monitoring Setup

### Quick Start with Docker

```bash
cd monitoring
docker-compose -f docker-compose.monitoring.yml up -d
```

Access:
- Grafana: http://localhost:3000 (admin/admin)
- Prometheus: http://localhost:9091

### Manual Setup

1. **Prometheus** (`/etc/prometheus/prometheus.yml`):
   ```yaml
   global:
     scrape_interval: 15s

   scrape_configs:
     - job_name: 'polymarket-arb'
       static_configs:
         - targets: ['localhost:9090']
   ```

2. **Grafana**:
   - Import the dashboard from `monitoring/grafana-dashboard.json`
   - Configure Prometheus as data source

### Key Metrics to Monitor

| Metric | Description | Alert Threshold |
|--------|-------------|-----------------|
| `order_submit_latency_ms` | Order submission time | P95 > 200ms |
| `ws_message_latency_ms` | WebSocket processing | P95 > 50ms |
| `opportunities_detected_total` | Opportunities found | Rate < 1/min |
| `orders_failed_total` | Failed orders | Rate > 0 |
| `ws_reconnects_total` | WebSocket reconnections | Rate > 1/hour |

### Alerting

Configure alerts in Grafana or use Prometheus Alertmanager:

```yaml
# alertmanager/alerts.yml
groups:
  - name: polymarket-arb
    rules:
      - alert: HighOrderLatency
        expr: histogram_quantile(0.95, order_submit_latency_ms) > 200
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High order submission latency"

      - alert: OrdersFailingError
        expr: rate(orders_failed_total[5m]) > 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Orders are failing"
```

---

## Troubleshooting

### Common Issues

#### "No active market found"
- Markets open every 15 minutes on the hour (XX:00, XX:15, XX:30, XX:45)
- Check if BTC 15min markets are currently active on Polymarket
- Verify internet connectivity

#### "Order submission failed"
- Check USDC balance is sufficient
- Verify private key is correct
- Check Polymarket API status
- Review `SIGNATURE_TYPE` setting

#### "WebSocket connection failed"
- Check firewall allows outbound WSS (port 443)
- Verify `POLYMARKET_WS_URL` is correct
- Check for network issues with `ws-test` command

#### High Latency
1. Run benchmark: `cargo run -- benchmark`
2. Check network path to Polymarket servers
3. Consider closer data center (US East)
4. Apply system tuning optimizations

#### Memory Issues
- Monitor with `htop` or `top`
- Check for memory leaks in logs
- Increase swap if needed

### Debug Commands

```bash
# Verbose logging
RUST_LOG=debug cargo run -- run

# Test configuration only
cargo run -- check-config

# Test connectivity
cargo run -- check-balance

# Test WebSocket
cargo run -- ws-test

# Benchmark latency
cargo run -- benchmark
```

### Logs

```bash
# View systemd logs
sudo journalctl -u polymarket-arb -f

# View with timestamps
sudo journalctl -u polymarket-arb --since "1 hour ago"
```

---

## Risk Management

### Position Limits

Configure maximum exposure:

```bash
# In .env
ORDER_SIZE=10          # Max 10 shares per side
BALANCE_MARGIN=1.2     # Keep 20% buffer
```

### Circuit Breakers

The bot includes automatic protections:

1. **Cooldown Timer**: Prevents rapid-fire trading
2. **Balance Check**: Verifies funds before each trade
3. **Partial Fill Handling**: Automatically unwinds single-leg fills

### Best Practices

1. **Start Small**: Begin with minimum order size (5 shares)
2. **Use Dry Run**: Test thoroughly before live trading
3. **Monitor Continuously**: Set up alerts for anomalies
4. **Regular Audits**: Review trade history weekly
5. **Backup Strategy**: Have manual intervention plan

### Emergency Procedures

```bash
# Stop the bot immediately
sudo systemctl stop polymarket-arb

# Cancel all open orders (if supported)
# Manual intervention via Polymarket UI may be needed
```

---

## Performance Benchmarking

### Running Benchmarks

```bash
# Built-in benchmark
cargo run --release -- benchmark

# Using hyperfine (install: cargo install hyperfine)
./scripts/benchmark.sh
```

### Expected Latencies

| Operation | Target | Acceptable |
|-----------|--------|------------|
| Order book fetch | < 50ms | < 100ms |
| Order submission | < 100ms | < 200ms |
| Signing | < 5ms | < 10ms |
| WebSocket update | < 10ms | < 25ms |
| End-to-end (detect + execute) | < 150ms | < 300ms |

### Optimization Checklist

- [ ] Using release build (`cargo build --release`)
- [ ] System tuning applied
- [ ] Deployed in US East region
- [ ] WebSocket mode enabled
- [ ] HTTP connection pooling active
- [ ] Signer caching enabled

---

## Maintenance

### Updates

```bash
# Pull latest changes
git pull origin main

# Rebuild
cargo build --release

# Restart service
sudo systemctl restart polymarket-arb
```

### Log Rotation

Add to `/etc/logrotate.d/polymarket-arb`:

```
/var/log/polymarket-arb/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
    create 644 ubuntu ubuntu
}
```

### Backups

```bash
# Backup configuration
./scripts/backup-config.sh

# Automated daily backup
0 2 * * * /home/ubuntu/polymarket-arb/scripts/backup-config.sh
```

### Health Checks

```bash
# Manual health check
./scripts/health-check.sh

# Or via curl
curl -s http://localhost:8080/health | jq .
```

### Scheduled Maintenance Windows

- **Weekly**: Review logs and metrics
- **Monthly**: Update dependencies, security patches
- **Quarterly**: Full system audit

---

## Support

For issues:
1. Check this guide's troubleshooting section
2. Review logs: `journalctl -u polymarket-arb`
3. Run diagnostics: `cargo run -- check-config`
4. Open an issue with logs and configuration (redact private key!)

---

## License

MIT License - See LICENSE file for details.
