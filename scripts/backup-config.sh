#!/bin/bash
#
# Backup configuration and state for Polymarket Arbitrage Bot
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BACKUP_DIR="${BACKUP_DIR:-$PROJECT_DIR/backups}"
DATE=$(date +%Y%m%d_%H%M%S)
BACKUP_NAME="polymarket-arb-backup-${DATE}"

echo "=============================================="
echo "POLYMARKET ARB BOT - CONFIGURATION BACKUP"
echo "=============================================="
echo ""

cd "$PROJECT_DIR"

# Create backup directory
mkdir -p "$BACKUP_DIR"

echo "1. Creating backup archive..."

# Create a temporary directory for backup contents
TEMP_DIR=$(mktemp -d)
BACKUP_TEMP="$TEMP_DIR/$BACKUP_NAME"
mkdir -p "$BACKUP_TEMP"

# Copy configuration files (but NOT the .env with secrets)
if [ -f ".env.example" ]; then
    cp .env.example "$BACKUP_TEMP/"
fi

# Copy documentation
if [ -d "docs" ]; then
    cp -r docs "$BACKUP_TEMP/"
fi

# Copy scripts
if [ -d "scripts" ]; then
    cp -r scripts "$BACKUP_TEMP/"
fi

# Copy monitoring configs
if [ -d "monitoring" ]; then
    cp -r monitoring "$BACKUP_TEMP/"
fi

# Copy Cargo files
cp Cargo.toml "$BACKUP_TEMP/"
cp Cargo.lock "$BACKUP_TEMP/" 2>/dev/null || true

# Copy Docker files
cp Dockerfile "$BACKUP_TEMP/" 2>/dev/null || true
cp docker-compose.yml "$BACKUP_TEMP/" 2>/dev/null || true

# Create manifest
cat > "$BACKUP_TEMP/MANIFEST.txt" << EOF
Polymarket Arbitrage Bot Backup
================================
Created: $(date)
Hostname: $(hostname)
Git commit: $(git rev-parse HEAD 2>/dev/null || echo "unknown")
Git branch: $(git branch --show-current 2>/dev/null || echo "unknown")

Contents:
- .env.example (template only, NOT actual secrets)
- docs/
- scripts/
- monitoring/
- Cargo.toml
- Cargo.lock
- Dockerfile
- docker-compose.yml

IMPORTANT: This backup does NOT contain:
- .env (secrets)
- target/ (build artifacts)
- Private keys or credentials

To restore:
1. Extract this archive
2. Copy files to your project directory
3. Create .env from .env.example with your secrets
4. Run: cargo build --release
EOF

# Create the archive
cd "$TEMP_DIR"
tar -czf "${BACKUP_NAME}.tar.gz" "$BACKUP_NAME"
mv "${BACKUP_NAME}.tar.gz" "$BACKUP_DIR/"

# Cleanup
rm -rf "$TEMP_DIR"

BACKUP_PATH="$BACKUP_DIR/${BACKUP_NAME}.tar.gz"
BACKUP_SIZE=$(ls -lh "$BACKUP_PATH" | awk '{print $5}')

echo "   Done."
echo ""

echo "2. Backup details:"
echo "   Location: $BACKUP_PATH"
echo "   Size: $BACKUP_SIZE"
echo ""

# Optional: Keep only last N backups
MAX_BACKUPS="${MAX_BACKUPS:-10}"
echo "3. Cleaning old backups (keeping last $MAX_BACKUPS)..."

cd "$BACKUP_DIR"
BACKUP_COUNT=$(ls -1 polymarket-arb-backup-*.tar.gz 2>/dev/null | wc -l)

if [ "$BACKUP_COUNT" -gt "$MAX_BACKUPS" ]; then
    TO_DELETE=$((BACKUP_COUNT - MAX_BACKUPS))
    ls -1t polymarket-arb-backup-*.tar.gz | tail -n "$TO_DELETE" | xargs rm -f
    echo "   Removed $TO_DELETE old backup(s)"
else
    echo "   No cleanup needed ($BACKUP_COUNT backups)"
fi
echo ""

echo "=============================================="
echo "BACKUP COMPLETE"
echo ""
echo "Archive: $BACKUP_PATH"
echo ""
echo "NOTE: This backup does NOT include:"
echo "  - .env file (contains secrets)"
echo "  - Build artifacts (target/)"
echo ""
echo "Store your .env file separately and securely!"
echo "=============================================="
