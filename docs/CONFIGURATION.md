# Configuration Reference

Complete list of all environment variables and their usage.

## Required Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `POLYMARKET_PRIVATE_KEY` | Wallet private key (hex with 0x prefix) | `0x1234...abcd` |

## Wallet Configuration

| Variable | Description | Default | Values |
|----------|-------------|---------|--------|
| `POLYMARKET_SIGNATURE_TYPE` | Wallet signature type | `0` | `0`=EOA, `1`=Magic.link, `2`=Gnosis |
| `POLYMARKET_FUNDER` | Proxy wallet address (Magic.link only) | - | `0x...` |

### Signature Types Explained

- **0 (EOA)**: Standard externally owned account (MetaMask, hardware wallet)
- **1 (Magic.link)**: Email login on Polymarket - requires `POLYMARKET_FUNDER`
- **2 (Gnosis Safe)**: Multi-signature wallet

## Optional API Credentials

These are derived from the private key automatically, but can be provided manually:

| Variable | Description |
|----------|-------------|
| `POLYMARKET_API_KEY` | Pre-generated API key |
| `POLYMARKET_API_SECRET` | Pre-generated API secret |
| `POLYMARKET_API_PASSPHRASE` | Pre-generated API passphrase |

## Trading Parameters

| Variable | Description | Default | Range |
|----------|-------------|---------|-------|
| `TARGET_PAIR_COST` | Max combined cost to trigger arbitrage | `0.991` | `0.0` - `1.0` |
| `ORDER_SIZE` | Shares per trade | `5` | `5` minimum |
| `ORDER_TYPE` | Time-in-force for orders | `FOK` | `FOK`, `FAK`, `GTC` |
| `BALANCE_MARGIN` | Safety margin multiplier | `1.2` | `1.0`+ |
| `COOLDOWN_SECONDS` | Minimum seconds between trades | `10` | `0`+ |

### Order Types Explained

- **FOK (Fill-or-Kill)**: Order must fill completely or cancel entirely
- **FAK (Fill-and-Kill)**: Fill what's available, cancel remainder
- **GTC (Good-Til-Cancelled)**: Order stays open until filled or cancelled

**Recommendation**: Use `FOK` to avoid partial fills leaving one leg open.

### Target Pair Cost

The bot triggers when: `UP_price + DOWN_price < TARGET_PAIR_COST`

Example with `TARGET_PAIR_COST=0.991`:
- UP = $0.48, DOWN = $0.51, Total = $0.99 → Triggers (0.99 < 0.991)
- UP = $0.50, DOWN = $0.50, Total = $1.00 → Does NOT trigger

## Operation Modes

| Variable | Description | Default |
|----------|-------------|---------|
| `DRY_RUN` | Simulation mode (no real orders) | `true` |
| `SIM_BALANCE` | Starting balance for simulation | `100` |
| `VERBOSE` | Enable verbose logging | `false` |

## Market Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `POLYMARKET_MARKET_SLUG` | Force specific market (skip discovery) | - |
| `POLYMARKET_CLOB_URL` | CLOB API base URL | `https://clob.polymarket.com` |

## WebSocket Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `USE_WSS` | Enable WebSocket market feed | `false` |
| `POLYMARKET_WS_URL` | WebSocket base URL | `wss://ws-subscriptions-clob.polymarket.com` |

## Server Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `PORT` | HTTP server port | `8080` |
| `RUST_LOG` | Log level | `info` |

### Log Levels

- `trace`: Very verbose, includes all details
- `debug`: Detailed information for debugging
- `info`: Normal operational messages
- `warn`: Warning messages
- `error`: Error messages only

## Example Configurations

### Conservative (Recommended for Starting)

```env
POLYMARKET_PRIVATE_KEY=0x_your_key_here
POLYMARKET_SIGNATURE_TYPE=0

TARGET_PAIR_COST=0.985    # More conservative threshold
ORDER_SIZE=5              # Minimum size
ORDER_TYPE=FOK            # Fill-or-kill
COOLDOWN_SECONDS=30       # Longer cooldown

DRY_RUN=true              # Start in simulation
SIM_BALANCE=100
```

### Aggressive (Higher Risk/Reward)

```env
POLYMARKET_PRIVATE_KEY=0x_your_key_here
POLYMARKET_SIGNATURE_TYPE=0

TARGET_PAIR_COST=0.995    # Tighter threshold
ORDER_SIZE=50             # Larger orders
ORDER_TYPE=FOK
COOLDOWN_SECONDS=5        # Faster execution

DRY_RUN=false             # Live trading
```

### WebSocket Mode

```env
POLYMARKET_PRIVATE_KEY=0x_your_key_here
POLYMARKET_SIGNATURE_TYPE=0

USE_WSS=true
POLYMARKET_WS_URL=wss://ws-subscriptions-clob.polymarket.com

TARGET_PAIR_COST=0.991
ORDER_SIZE=10
DRY_RUN=false
```

## Validation

The bot validates configuration at startup:

1. `POLYMARKET_PRIVATE_KEY` must be present and start with `0x`
2. `ORDER_SIZE` must be at least 5
3. `TARGET_PAIR_COST` must be less than 1.0
4. Private key must be valid (32 bytes, valid hex)

Run `polymarket-arb check-config` to verify your configuration.
