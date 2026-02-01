# Troubleshooting Guide

Common issues and their solutions.

## "Invalid signature" Error

### Cause 1: Wrong signature type

**Symptom**: Orders rejected with "invalid signature"

**Solution**: Verify `POLYMARKET_SIGNATURE_TYPE` matches your wallet:
- `0` for EOA (MetaMask, hardware wallet)
- `1` for Magic.link (email login)
- `2` for Gnosis Safe

### Cause 2: Missing funder address (Magic.link)

**Symptom**: "invalid signature" with signature_type=1

**Solution**: Set `POLYMARKET_FUNDER` to your Polymarket proxy address:
1. Go to https://polymarket.com/@YOUR_USERNAME
2. Click "Copy address" next to your balance
3. Set that address as `POLYMARKET_FUNDER`

### Cause 3: Wrong funder address

**Symptom**: "invalid signature" even with funder set

**Solution**: Verify `POLYMARKET_FUNDER` is your **Polymarket proxy address**, NOT:
- Your Polygon wallet address
- Your signer address
- Any other address

### Cause 4: Stale API credentials

**Symptom**: "invalid signature" after changing private key

**Solution**: The bot derives API credentials from the private key. If you changed your private key, the old credentials are invalid. Clear any cached credentials.

## "No active BTC 15min market found"

### Cause 1: Between market cycles

**Symptom**: Error appears, then bot finds market after waiting

**Solution**: Normal behavior. Markets open every 15 minutes. The bot will retry automatically.

### Cause 2: Network issues

**Symptom**: Consistent failure to find markets

**Solution**:
1. Check your internet connection
2. Verify https://polymarket.com/crypto/15M loads in browser
3. Check if Polymarket API is down

### Cause 3: All discovery strategies failing

**Symptom**: "tried computed slugs, Gamma API, and page scrape"

**Solution**: Run with verbose logging to see which strategy fails:
```bash
RUST_LOG=debug polymarket-arb discover-market
```

## Balance Shows $0 but I Have Funds

### Cause 1: Wrong signature type

**Symptom**: Balance API returns 0

**Solution**: Verify `POLYMARKET_SIGNATURE_TYPE` is correct for your wallet type.

### Cause 2: Funds in proxy wallet (Magic.link)

**Symptom**: On-chain balance is $0, but Polymarket shows funds

**Solution**: This is normal for Magic.link. Your funds are in the Polymarket proxy contract, not your signer wallet. The bot's balance check via API should show the correct amount.

### Cause 3: Wrong private key

**Symptom**: Derived address doesn't match your Polymarket account

**Solution**: Run `polymarket-arb check-balance` and verify the derived address matches your Polymarket profile.

## Orders Not Filling

### Cause 1: Price moved

**Symptom**: FOK orders rejected

**Solution**: The order book moved between detection and submission. This is normal in fast markets. The bot will detect the next opportunity.

### Cause 2: Insufficient liquidity

**Symptom**: "insufficient liquidity" errors

**Solution**: Reduce `ORDER_SIZE` or increase `TARGET_PAIR_COST` threshold.

### Cause 3: Order size too small

**Symptom**: Orders rejected

**Solution**: Minimum order size is 5 shares. Verify `ORDER_SIZE >= 5`.

## Partial Fills

### Symptom

"Partial fill detected, attempting unwind" in logs

### Explanation

Only one leg (UP or DOWN) filled. The bot attempted to:
1. Cancel the unfilled order
2. Sell the filled position to flatten exposure

### What to Do

1. Check your positions on Polymarket
2. Manually close any open positions if needed
3. Consider using smaller order sizes

## WebSocket Connection Issues

### Cause 1: Firewall blocking WSS

**Symptom**: WebSocket connection fails immediately

**Solution**: Ensure your firewall allows outbound WSS connections to `wss://ws-subscriptions-clob.polymarket.com`

### Cause 2: Proxy not supporting WebSocket

**Symptom**: Connection drops frequently

**Solution**: If behind a corporate proxy, it may not support WebSocket. Use HTTP polling instead (`USE_WSS=false`).

### Cause 3: Connection timeout

**Symptom**: "WebSocket connection timed out"

**Solution**: Check network stability. The bot will automatically reconnect.

## High Latency / Missed Opportunities

### Cause 1: Network latency

**Symptom**: Opportunities detected but orders rejected

**Solution**:
1. Run the bot closer to Polymarket's servers (US East Coast)
2. Use WebSocket mode for lower latency
3. Tighten `TARGET_PAIR_COST` threshold

### Cause 2: Order book stale

**Symptom**: Fill prices don't match expected

**Solution**: Enable WebSocket mode for real-time order book updates:
```env
USE_WSS=true
```

## Bot Not Starting

### Cause 1: Configuration error

**Symptom**: "Configuration validation failed"

**Solution**: Run `polymarket-arb check-config` to identify the issue.

### Cause 2: Port already in use

**Symptom**: "Address already in use"

**Solution**: Change the port or stop the conflicting service:
```bash
polymarket-arb --port 8081
```

### Cause 3: Missing dependencies (Docker)

**Symptom**: Docker build fails

**Solution**: Ensure Docker has sufficient resources and network access.

## Getting Help

1. Check logs with verbose mode: `RUST_LOG=debug polymarket-arb`
2. Run diagnostic commands:
   - `polymarket-arb check-config`
   - `polymarket-arb check-balance`
   - `polymarket-arb discover-market`
3. Review the [Architecture](ARCHITECTURE.md) document
4. Open an issue with:
   - Error message
   - Configuration (redact private key!)
   - Steps to reproduce
