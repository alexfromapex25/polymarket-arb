# BTC 15-Minute Polymarket Arbitrage Bot (Rust)

High-performance arbitrage bot for **Bitcoin 15-minute UP/DOWN markets** on Polymarket, written in Rust for low latency and production reliability.

## Strategy

**Pure arbitrage**: Buy both sides (UP + DOWN) when total cost < $1.00 to guarantee profit regardless of outcome.

```
BTC goes up (UP):     $0.48
BTC goes down (DOWN): $0.51
─────────────────────────────
Total:                $0.99  < $1.00
Profit:               $0.01 per share (1.01%)
```

At market close, ONE side pays $1.00. If you paid $0.99 for both, you keep $0.01 guaranteed profit.

## Features

- **Auto-discovers** active BTC 15min market (3 discovery strategies)
- **Depth-aware pricing**: walks order book to calculate worst-case fill price
- **Concurrent execution**: fetches both order books in parallel
- **Paired verification**: confirms both legs fill, handles partial fills
- **Automatic unwind**: attempts to flatten exposure on partial fills
- **Simulation mode**: test strategies without real orders
- **WebSocket support**: optional real-time order book updates
- **Production ready**: health checks, Prometheus metrics, graceful shutdown
- **Docker support**: multi-stage build for small images

## Quick Start

### Prerequisites

- Rust 1.70+ (or Docker)
- Polymarket account with USDC balance
- Wallet private key

### 1. Clone and build

```bash
git clone <repository-url>
cd polymarket-arb
cargo build --release
```

### 2. Configure environment

```bash
cp .env.example .env
# Edit .env with your credentials
```

Required variables:
```env
POLYMARKET_PRIVATE_KEY=0x_your_private_key_here
```

### 3. Run in simulation mode

```bash
# Simulation (no real orders)
cargo run --release

# Or with Docker
docker-compose up -d
```

### 4. Run live trading

```bash
# Edit .env: set DRY_RUN=false
cargo run --release
```

## CLI Commands

```bash
# Main bot loop
polymarket-arb

# Check configuration validity
polymarket-arb check-config

# Check wallet balance
polymarket-arb check-balance

# Discover current market
polymarket-arb discover-market

# Run with verbose logging
polymarket-arb --verbose
```

## Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `POLYMARKET_PRIVATE_KEY` | Wallet private key (0x...) | **Required** |
| `POLYMARKET_SIGNATURE_TYPE` | 0=EOA, 1=Magic.link, 2=Gnosis | `0` |
| `POLYMARKET_FUNDER` | Proxy address (Magic.link only) | - |
| `TARGET_PAIR_COST` | Max cost to trigger arbitrage | `0.991` |
| `ORDER_SIZE` | Shares per trade (min 5) | `5` |
| `ORDER_TYPE` | FOK, FAK, or GTC | `FOK` |
| `DRY_RUN` | Simulation mode | `true` |
| `COOLDOWN_SECONDS` | Min seconds between trades | `10` |

See [docs/CONFIGURATION.md](docs/CONFIGURATION.md) for full list.

## API Endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /health` | Liveness probe (always 200) |
| `GET /ready` | Readiness probe |
| `GET /metrics` | Prometheus metrics |
| `GET /api/v1/status` | Bot status and stats |

## Docker

```bash
# Build image
docker build -t polymarket-arb .

# Run with docker-compose
docker-compose up -d

# View logs
docker-compose logs -f polymarket-arb
```

## Documentation

- [Setup Guide](docs/SETUP.md) - Detailed installation and wallet setup
- [Architecture](docs/ARCHITECTURE.md) - Design decisions and data flow
- [Configuration](docs/CONFIGURATION.md) - All environment variables
- [Troubleshooting](docs/TROUBLESHOOTING.md) - Common issues and solutions
- [API Reference](docs/API.md) - REST endpoints and metrics

## Project Structure

```
polymarket-arb/
├── src/
│   ├── main.rs           # Entry point, CLI
│   ├── config.rs         # Configuration loading
│   ├── error.rs          # Error types
│   ├── signing/          # Wallet signing utilities
│   ├── market/           # Market discovery, client
│   ├── orderbook/        # Order book management, WebSocket
│   ├── arbitrage/        # Detection, calculation, execution
│   ├── trading/          # Order submission, positions
│   ├── api/              # HTTP handlers, routes
│   └── utils/            # Shutdown, helpers
├── tests/                # Unit and integration tests
├── docs/                 # Documentation
├── Dockerfile            # Multi-stage build
└── docker-compose.yml    # Container orchestration
```

## Safety

- Always start with `DRY_RUN=true`
- Use small `ORDER_SIZE` initially (5 shares)
- Monitor positions on Polymarket
- Never share your private key

## Warnings

- Markets close every 15 minutes - don't accumulate positions
- Partial fills can leave you exposed - bot attempts to unwind
- Spreads can eliminate profit - verify liquidity before increasing size
- This software is for educational purposes - use at your own risk

## License

MIT
