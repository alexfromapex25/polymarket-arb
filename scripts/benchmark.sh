#!/bin/bash
#
# Latency benchmark script for Polymarket Arbitrage Bot
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "=============================================="
echo "POLYMARKET ARB BOT - LATENCY BENCHMARK"
echo "=============================================="
echo ""

cd "$PROJECT_DIR"

# Build release version
echo "1. Building release version..."
cargo build --release --quiet
echo "   Done."
echo ""

# Check if hyperfine is installed
if command -v hyperfine &> /dev/null; then
    USE_HYPERFINE=true
    echo "2. Found hyperfine - using for benchmarks"
else
    USE_HYPERFINE=false
    echo "2. hyperfine not found - using basic timing"
    echo "   Install with: cargo install hyperfine"
fi
echo ""

# Run built-in benchmark
echo "3. Running built-in benchmark..."
echo "----------------------------------------"
./target/release/polymarket-arb benchmark
echo "----------------------------------------"
echo ""

# If hyperfine is available, run additional benchmarks
if [ "$USE_HYPERFINE" = true ]; then
    echo "4. Running hyperfine benchmarks..."
    echo ""

    echo "   4a. Config check latency:"
    hyperfine --warmup 2 --runs 10 \
        './target/release/polymarket-arb check-config' \
        2>/dev/null || echo "   Skipped (requires valid config)"
    echo ""

    echo "   4b. Market discovery latency:"
    hyperfine --warmup 1 --runs 5 \
        './target/release/polymarket-arb discover-market' \
        2>/dev/null || echo "   Skipped (requires network)"
    echo ""
fi

# System info
echo "5. System Information:"
echo "----------------------------------------"
echo "   OS: $(uname -s) $(uname -r)"
echo "   CPU: $(sysctl -n machdep.cpu.brand_string 2>/dev/null || cat /proc/cpuinfo 2>/dev/null | grep 'model name' | head -1 | cut -d: -f2 || echo 'Unknown')"
echo "   Memory: $(free -h 2>/dev/null | awk '/^Mem:/{print $2}' || sysctl -n hw.memsize 2>/dev/null | awk '{print $1/1024/1024/1024 "GB"}' || echo 'Unknown')"
echo "----------------------------------------"
echo ""

# Network latency to Polymarket
echo "6. Network latency to Polymarket servers:"
echo "----------------------------------------"
if command -v ping &> /dev/null; then
    echo "   CLOB API (clob.polymarket.com):"
    ping -c 5 clob.polymarket.com 2>/dev/null | tail -1 || echo "   Unable to ping"
fi
echo "----------------------------------------"
echo ""

echo "=============================================="
echo "BENCHMARK COMPLETE"
echo ""
echo "For production deployments, aim for:"
echo "  - Order book fetch: < 50ms"
echo "  - Order submission: < 100ms"
echo "  - End-to-end: < 150ms"
echo "=============================================="
