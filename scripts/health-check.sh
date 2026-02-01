#!/bin/bash
#
# Health check script for Polymarket Arbitrage Bot
# Returns exit code 0 if healthy, 1 if unhealthy
#

set -e

# Configuration
HEALTH_PORT="${HEALTH_PORT:-8080}"
METRICS_PORT="${METRICS_PORT:-9090}"
HEALTH_URL="http://localhost:${HEALTH_PORT}/health"
METRICS_URL="http://localhost:${METRICS_PORT}/metrics"

# Colors (only if terminal supports it)
if [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    NC='\033[0m' # No Color
else
    RED=''
    GREEN=''
    YELLOW=''
    NC=''
fi

echo "=============================================="
echo "POLYMARKET ARB BOT - HEALTH CHECK"
echo "=============================================="
echo ""

HEALTHY=true

# Check 1: Health endpoint
echo "1. Checking health endpoint ($HEALTH_URL)..."
if curl -sf "$HEALTH_URL" > /dev/null 2>&1; then
    HEALTH_RESPONSE=$(curl -s "$HEALTH_URL")
    echo -e "   ${GREEN}OK${NC}"

    # Parse health response
    if echo "$HEALTH_RESPONSE" | grep -q '"ready":true'; then
        echo -e "   Status: ${GREEN}Ready${NC}"
    else
        echo -e "   Status: ${YELLOW}Not Ready${NC}"
    fi

    # Check for market
    if echo "$HEALTH_RESPONSE" | grep -q '"market"'; then
        MARKET=$(echo "$HEALTH_RESPONSE" | grep -o '"market":"[^"]*"' | cut -d'"' -f4)
        echo "   Market: $MARKET"
    fi
else
    echo -e "   ${RED}FAILED${NC} - Health endpoint not responding"
    HEALTHY=false
fi
echo ""

# Check 2: Metrics endpoint
echo "2. Checking metrics endpoint ($METRICS_URL)..."
if curl -sf "$METRICS_URL" > /dev/null 2>&1; then
    echo -e "   ${GREEN}OK${NC}"

    # Extract some key metrics
    METRICS=$(curl -s "$METRICS_URL")

    # Orders submitted
    ORDERS=$(echo "$METRICS" | grep 'orders_submitted_total' | grep -v '#' | awk '{print $2}' || echo "0")
    echo "   Orders submitted: ${ORDERS:-0}"

    # Opportunities detected
    OPPS=$(echo "$METRICS" | grep 'opportunities_detected_total' | grep -v '#' | awk '{print $2}' || echo "0")
    echo "   Opportunities detected: ${OPPS:-0}"

    # WebSocket reconnects
    WS_RECONNECTS=$(echo "$METRICS" | grep 'ws_reconnects_total' | grep -v '#' | awk '{print $2}' || echo "0")
    if [ "${WS_RECONNECTS:-0}" != "0" ]; then
        echo -e "   WS reconnects: ${YELLOW}${WS_RECONNECTS}${NC}"
    else
        echo "   WS reconnects: 0"
    fi
else
    echo -e "   ${YELLOW}WARNING${NC} - Metrics endpoint not responding"
    echo "   (This may be expected if metrics are disabled)"
fi
echo ""

# Check 3: Process running
echo "3. Checking process..."
if pgrep -f "polymarket-arb" > /dev/null 2>&1; then
    PID=$(pgrep -f "polymarket-arb" | head -1)
    echo -e "   ${GREEN}OK${NC} - Running (PID: $PID)"

    # Memory usage
    if command -v ps &> /dev/null; then
        MEM=$(ps -o rss= -p "$PID" 2>/dev/null | awk '{print $1/1024 "MB"}')
        echo "   Memory: ${MEM:-unknown}"
    fi
else
    echo -e "   ${RED}FAILED${NC} - Process not found"
    HEALTHY=false
fi
echo ""

# Check 4: Network connectivity to Polymarket
echo "4. Checking network connectivity..."
if curl -sf "https://clob.polymarket.com" > /dev/null 2>&1; then
    echo -e "   ${GREEN}OK${NC} - Can reach Polymarket API"
else
    echo -e "   ${RED}FAILED${NC} - Cannot reach Polymarket API"
    HEALTHY=false
fi
echo ""

# Summary
echo "=============================================="
if [ "$HEALTHY" = true ]; then
    echo -e "HEALTH CHECK: ${GREEN}PASSED${NC}"
    exit 0
else
    echo -e "HEALTH CHECK: ${RED}FAILED${NC}"
    exit 1
fi
