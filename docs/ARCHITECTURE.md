# Architecture

Technical design overview of the BTC 15-Minute Polymarket Arbitrage Bot.

## High-Level Design

```
┌─────────────────────────────────────────────────────────────────┐
│                         Main Loop                                │
├─────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────────┐    │
│  │   Market     │──▶│  Order Book  │──▶│   Arbitrage      │    │
│  │  Discovery   │   │   Fetching   │   │   Detection      │    │
│  └──────────────┘   └──────────────┘   └────────┬─────────┘    │
│                                                  │               │
│                                                  ▼               │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────────┐    │
│  │   Position   │◀──│    Trade     │◀──│   Arbitrage      │    │
│  │   Tracking   │   │  Execution   │   │   Executor       │    │
│  └──────────────┘   └──────────────┘   └──────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                       HTTP API Server                            │
│  /health  │  /ready  │  /metrics  │  /api/v1/status             │
└─────────────────────────────────────────────────────────────────┘
```

## Module Structure

```
src/
├── main.rs              # Entry point, CLI, subcommands
├── lib.rs               # Library exports
├── config.rs            # Environment configuration
├── error.rs             # Error types (thiserror)
│
├── signing/             # Wallet authentication
│   └── mod.rs           # Private key signing, auth headers
│
├── market/              # Market discovery & client
│   ├── mod.rs           # Public exports
│   ├── types.rs         # Market, Outcome types
│   ├── discovery.rs     # 3-strategy market finder
│   ├── client.rs        # Polymarket API wrapper
│   └── mock.rs          # Mock client for testing
│
├── orderbook/           # Order book management
│   ├── mod.rs           # Public exports
│   ├── types.rs         # PriceLevel, OutcomeBook
│   ├── aggregator.rs    # Fill price calculation
│   └── websocket.rs     # WebSocket L2 feed
│
├── arbitrage/           # Arbitrage logic
│   ├── mod.rs           # Public exports
│   ├── calculator.rs    # Profit/cost calculations
│   ├── detector.rs      # Opportunity detection
│   └── executor.rs      # Trade execution
│
├── trading/             # Order management
│   ├── mod.rs           # Public exports
│   ├── order.rs         # Order types, validation
│   ├── execution.rs     # Order submission
│   └── position.rs      # Position tracking
│
├── api/                 # HTTP server
│   ├── mod.rs           # Public exports
│   ├── routes.rs        # Route definitions
│   └── handlers.rs      # Request handlers
│
└── utils/               # Utilities
    ├── mod.rs           # Public exports
    └── shutdown.rs      # Graceful shutdown
```

## Data Flow

### 1. Market Discovery

```
try_computed_slugs()  ─┐
                       ├──▶ fetch_market_from_slug() ──▶ Market
try_gamma_api()       ─┤
                       │
try_page_scrape()     ─┘
```

Three strategies tried in order:
1. **Computed slugs**: Calculate expected slug from current timestamp
2. **Gamma API**: Query Polymarket's market API
3. **Page scrape**: Parse crypto/15M page HTML

### 2. Order Book Fetching

```
┌─────────────────────────────────────────┐
│            tokio::join!                  │
│  ┌─────────────┐    ┌─────────────┐     │
│  │ GET /book   │    │ GET /book   │     │
│  │ (UP token)  │    │ (DOWN token)│     │
│  └──────┬──────┘    └──────┬──────┘     │
│         │                  │            │
│         ▼                  ▼            │
│    OutcomeBook        OutcomeBook       │
└─────────────────────────────────────────┘
```

Both order books fetched concurrently for minimum latency.

### 3. Arbitrage Detection

```
OutcomeBook (UP)  ──┐
                    ├──▶ calculate_fill_price() ──┐
                    │                              │
OutcomeBook (DOWN) ─┘                              │
                                                   ▼
                                        total_cost = up + down
                                                   │
                                                   ▼
                                    if total_cost < threshold
                                                   │
                                                   ▼
                                        ArbitrageOpportunity
```

### 4. Trade Execution

```
ArbitrageOpportunity
         │
         ▼
┌─────────────────────────────────────────┐
│            tokio::join!                  │
│  ┌─────────────┐    ┌─────────────┐     │
│  │ submit_order│    │ submit_order│     │
│  │ (UP BUY)    │    │ (DOWN BUY)  │     │
│  └──────┬──────┘    └──────┬──────┘     │
│         │                  │            │
│         ▼                  ▼            │
│    order_id_up        order_id_down     │
└─────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│            tokio::join!                  │
│  ┌─────────────┐    ┌─────────────┐     │
│  │ wait_for_   │    │ wait_for_   │     │
│  │ terminal    │    │ terminal    │     │
│  └──────┬──────┘    └──────┬──────┘     │
│         │                  │            │
│         ▼                  ▼            │
│    OrderState         OrderState        │
└─────────────────────────────────────────┘
         │
         ▼
    ExecutionResult
    ├── BothFilled     ──▶ Success!
    ├── PartialFill    ──▶ Attempt unwind
    └── NeitherFilled  ──▶ Log and continue
```

## Key Design Decisions

### 1. Decimal Arithmetic

All financial calculations use `rust_decimal::Decimal`, **never** `f64`:

```rust
use rust_decimal::Decimal;

let price = Decimal::new(48, 2);  // $0.48
let size = Decimal::new(5, 0);    // 5 shares
let cost = price * size;          // $2.40 (exact)
```

### 2. Concurrent Operations

Tokio's `join!` macro for parallel execution:

```rust
let (up_result, down_result) = tokio::join!(
    client.get_order_book(&up_token),
    client.get_order_book(&down_token),
);
```

### 3. Error Handling

Structured errors with `thiserror`:

```rust
#[derive(Error, Debug)]
pub enum TradingError {
    #[error("order submission failed: {0}")]
    SubmissionFailed(String),

    #[error("insufficient balance: need {required}, have {available}")]
    InsufficientFunds { required: Decimal, available: Decimal },
}
```

### 4. Observability

- **Logging**: `tracing` with `#[instrument]` annotations
- **Metrics**: Prometheus via `/metrics` endpoint
- **Health checks**: `/health` (liveness), `/ready` (readiness)

### 5. Configuration

Environment-based with `dotenvy` + `envy`:

```rust
#[derive(Deserialize)]
pub struct Config {
    #[serde(default = "default_target_cost")]
    pub target_pair_cost: Decimal,
}

impl Config {
    pub fn load() -> Result<Self, envy::Error> {
        dotenvy::dotenv().ok();
        envy::from_env()
    }
}
```

## Latency Optimization

1. **Concurrent fetching**: Both order books fetched in parallel
2. **Pre-signed orders**: Sign both orders before submission
3. **Concurrent submission**: Both orders submitted in parallel
4. **FOK orders**: Fill-or-Kill to avoid partial fills leaving exposure
5. **Minimal allocations**: Reuse buffers in hot path

## Safety Mechanisms

1. **Dry run mode**: Test without real orders
2. **Balance check**: Verify sufficient funds before trading
3. **Cooldown**: Minimum time between executions
4. **Partial fill handling**: Attempt to unwind if only one leg fills
5. **Graceful shutdown**: Cancel open orders on shutdown
