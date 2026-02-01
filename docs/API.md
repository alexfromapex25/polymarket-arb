# API Reference

REST API endpoints and Prometheus metrics.

## Endpoints

### Health Check

```
GET /health
```

Liveness probe. Always returns 200 if the server is running.

**Response**: `200 OK`
```json
{
  "status": "ok"
}
```

### Readiness Check

```
GET /ready
```

Readiness probe. Returns 200 if the bot is connected and ready to trade.

**Response (ready)**: `200 OK`
```json
{
  "status": "ready",
  "market": "btc-updown-15m-1765301400"
}
```

**Response (not ready)**: `503 Service Unavailable`
```json
{
  "status": "not_ready",
  "reason": "no active market"
}
```

### Prometheus Metrics

```
GET /metrics
```

Prometheus-formatted metrics.

**Response**: `200 OK`
```
# HELP arbitrage_scans_total Total number of arbitrage scans
# TYPE arbitrage_scans_total counter
arbitrage_scans_total 1234

# HELP arbitrage_opportunities_total Total opportunities detected
# TYPE arbitrage_opportunities_total counter
arbitrage_opportunities_total 5

# HELP arbitrage_executions_total Total trade executions
# TYPE arbitrage_executions_total counter
arbitrage_executions_total{result="both_filled"} 3
arbitrage_executions_total{result="partial"} 1
arbitrage_executions_total{result="neither"} 1

# HELP orderbook_fetch_latency_seconds Order book fetch latency
# TYPE orderbook_fetch_latency_seconds histogram
orderbook_fetch_latency_seconds_bucket{le="0.1"} 100
orderbook_fetch_latency_seconds_bucket{le="0.5"} 150
orderbook_fetch_latency_seconds_bucket{le="1.0"} 155
orderbook_fetch_latency_seconds_bucket{le="+Inf"} 155
orderbook_fetch_latency_seconds_sum 12.345
orderbook_fetch_latency_seconds_count 155
```

### Bot Status

```
GET /api/v1/status
```

Current bot status and statistics.

**Response**: `200 OK`
```json
{
  "status": "running",
  "mode": "simulation",
  "market": {
    "slug": "btc-updown-15m-1765301400",
    "time_remaining": "8m 32s"
  },
  "stats": {
    "opportunities_found": 5,
    "trades_executed": 3,
    "total_invested": "14.85",
    "total_shares_bought": "30",
    "expected_profit": "0.15"
  },
  "config": {
    "target_pair_cost": "0.991",
    "order_size": "5",
    "order_type": "FOK",
    "dry_run": true
  }
}
```

## Prometheus Metrics

### Counters

| Metric | Description | Labels |
|--------|-------------|--------|
| `arbitrage_scans_total` | Total arbitrage scans performed | - |
| `arbitrage_opportunities_total` | Total opportunities detected | - |
| `arbitrage_executions_total` | Total execution attempts | `result` |

**Execution result labels**:
- `both_filled`: Both legs filled successfully
- `partial`: Only one leg filled
- `neither`: Neither leg filled
- `simulated`: Dry run execution

### Histograms

| Metric | Description | Buckets |
|--------|-------------|---------|
| `orderbook_fetch_latency_seconds` | Order book fetch latency | 0.05, 0.1, 0.25, 0.5, 1.0, 2.5 |
| `arbitrage_scan_latency_seconds` | Full scan cycle latency | 0.01, 0.05, 0.1, 0.25, 0.5, 1.0 |

### Gauges

| Metric | Description |
|--------|-------------|
| `arbitrage_profit_total_usd` | Total profit in USD |
| `arbitrage_investment_total_usd` | Total investment in USD |
| `account_balance_usd` | Current USDC balance |

## Kubernetes Integration

### Liveness Probe

```yaml
livenessProbe:
  httpGet:
    path: /health
    port: 8080
  initialDelaySeconds: 5
  periodSeconds: 10
```

### Readiness Probe

```yaml
readinessProbe:
  httpGet:
    path: /ready
    port: 8080
  initialDelaySeconds: 10
  periodSeconds: 5
```

### ServiceMonitor (Prometheus Operator)

```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: polymarket-arb
spec:
  selector:
    matchLabels:
      app: polymarket-arb
  endpoints:
  - port: http
    path: /metrics
    interval: 15s
```

## Grafana Dashboard

Example queries for Grafana:

### Opportunities per Minute
```promql
rate(arbitrage_opportunities_total[1m]) * 60
```

### Execution Success Rate
```promql
sum(rate(arbitrage_executions_total{result="both_filled"}[5m]))
/
sum(rate(arbitrage_executions_total[5m]))
```

### Average Order Book Latency
```promql
rate(orderbook_fetch_latency_seconds_sum[5m])
/
rate(orderbook_fetch_latency_seconds_count[5m])
```

### P99 Latency
```promql
histogram_quantile(0.99, rate(orderbook_fetch_latency_seconds_bucket[5m]))
```
