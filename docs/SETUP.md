# Setup Guide

Complete step-by-step guide for setting up the BTC 15-Minute Polymarket Arbitrage Bot.

## Prerequisites

- **Rust 1.70+** (or Docker)
- **Polymarket account** with USDC balance
- **Wallet private key** (EOA, Magic.link, or Gnosis Safe)

## Installation

### Option 1: Build from Source

```bash
# Clone the repository
git clone <repository-url>
cd polymarket-arb

# Build release binary
cargo build --release

# Binary is at: target/release/polymarket-arb
```

### Option 2: Docker

```bash
# Build Docker image
docker build -t polymarket-arb .

# Or use docker-compose
docker-compose build
```

## Wallet Setup

### EOA Wallet (MetaMask, Hardware Wallet)

1. Export your private key from MetaMask:
   - Click account icon > Account Details > Export Private Key
   - Enter password and copy the key

2. Set environment variables:
   ```env
   POLYMARKET_PRIVATE_KEY=0x_your_private_key_here
   POLYMARKET_SIGNATURE_TYPE=0
   ```

### Magic.link (Email Login on Polymarket)

If you log into Polymarket with email, you have a **Magic.link proxy wallet**.

1. **Find your proxy wallet address:**
   - Go to https://polymarket.com/@YOUR_USERNAME
   - Click "Copy address" next to your balance
   - This is your `POLYMARKET_FUNDER`

2. **Get your private key:**
   - This is the key that signs transactions, NOT the proxy address
   - Contact Polymarket support or check your Magic.link dashboard

3. Set environment variables:
   ```env
   POLYMARKET_PRIVATE_KEY=0x_your_signer_private_key
   POLYMARKET_SIGNATURE_TYPE=1
   POLYMARKET_FUNDER=0x_your_proxy_wallet_address
   ```

### Gnosis Safe (Multi-sig)

```env
POLYMARKET_PRIVATE_KEY=0x_your_signer_private_key
POLYMARKET_SIGNATURE_TYPE=2
```

## API Credentials

The bot derives API credentials from your private key automatically. However, you can optionally provide pre-generated credentials:

```env
POLYMARKET_API_KEY=your_api_key
POLYMARKET_API_SECRET=your_api_secret
POLYMARKET_API_PASSPHRASE=your_passphrase
```

## Configuration

### 1. Create .env file

```bash
cp .env.example .env
```

### 2. Edit .env with your settings

```env
# Required
POLYMARKET_PRIVATE_KEY=0x_your_private_key_here

# Wallet type (see above)
POLYMARKET_SIGNATURE_TYPE=0

# Trading parameters
TARGET_PAIR_COST=0.991     # Buy when UP + DOWN < $0.991
ORDER_SIZE=5               # 5 shares per trade (minimum)
ORDER_TYPE=FOK             # Fill-or-Kill

# Start in simulation mode
DRY_RUN=true
SIM_BALANCE=100
```

### 3. Verify configuration

```bash
./target/release/polymarket-arb check-config
```

### 4. Check balance

```bash
./target/release/polymarket-arb check-balance
```

## Running the Bot

### Simulation Mode (Recommended First)

```bash
# With cargo
cargo run --release

# With binary
./target/release/polymarket-arb

# With Docker
docker-compose up -d
```

### Live Trading

1. Edit `.env`:
   ```env
   DRY_RUN=false
   ```

2. Run the bot:
   ```bash
   ./target/release/polymarket-arb
   ```

## Verifying Setup

### 1. Check Configuration
```bash
polymarket-arb check-config
```

Expected output:
```
======================================================================
BTC 15M ARB BOT - CONFIGURATION CHECK
======================================================================
Loading configuration... OK
Validating configuration... OK
Checking private key... OK
  Wallet address: 0x1234...
----------------------------------------------------------------------
Configuration Summary:
  Signature Type: 0 (EOA - Standard wallet)
  Target Pair Cost: $0.991
  Order Size: 5 shares
  ...
======================================================================
CONFIGURATION CHECK PASSED
======================================================================
```

### 2. Check Balance
```bash
polymarket-arb check-balance
```

Expected output:
```
======================================================================
BTC 15M ARB BOT - BALANCE CHECK
======================================================================
1. Creating client... OK
2. Getting wallet address... OK
   Address: 0x1234...
3. Getting USDC balance... OK
   USDC Balance: $100.000000
4. Getting positions... OK
   Total positions: 0
======================================================================
BALANCE CHECK COMPLETED
======================================================================
```

### 3. Discover Market
```bash
polymarket-arb discover-market
```

Expected output:
```
======================================================================
BTC 15M ARB BOT - MARKET DISCOVERY
======================================================================
Searching for active BTC 15min market...

MARKET FOUND
----------------------------------------------------------------------
  Slug: btc-updown-15m-1765301400
  UP Token: 21742633143463906290569050155826241533067272736897614950488156847949938836455
  DOWN Token: 48331043336612883890938759509493159234755048973500640148014422747788308965732
  Time Remaining: 12m 34s
======================================================================
```

## Troubleshooting

See [TROUBLESHOOTING.md](TROUBLESHOOTING.md) for common issues and solutions.

## Next Steps

1. Run in simulation mode for a few market cycles
2. Monitor the logs for arbitrage opportunities
3. Verify the bot correctly detects and (simulates) trades
4. When confident, switch to live trading with small order sizes
5. Gradually increase order size as you gain confidence
