.PHONY: build test lint fmt check run docker clean help

# Default target
all: check

## Build
build:
	cargo build --release

build-dev:
	cargo build

## Testing
test:
	cargo test --all-features

test-verbose:
	cargo test --all-features -- --nocapture

## Linting and formatting
lint:
	cargo clippy --all-targets --all-features -- -D warnings

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

## Combined check (format, lint, test)
check: fmt lint test

## Run
run:
	cargo run --release

run-dev:
	cargo run

run-dry:
	DRY_RUN=true cargo run

## Docker
docker:
	docker build -t polymarket-arb .

docker-run:
	docker-compose up -d

docker-stop:
	docker-compose down

## Clean
clean:
	cargo clean

## Documentation
doc:
	cargo doc --open

## Help
help:
	@echo "Polymarket Arbitrage Bot - Makefile Commands"
	@echo ""
	@echo "Build:"
	@echo "  make build       - Build release binary"
	@echo "  make build-dev   - Build debug binary"
	@echo ""
	@echo "Test:"
	@echo "  make test        - Run all tests"
	@echo "  make test-verbose - Run tests with output"
	@echo ""
	@echo "Quality:"
	@echo "  make lint        - Run clippy linter"
	@echo "  make fmt         - Format code"
	@echo "  make fmt-check   - Check formatting"
	@echo "  make check       - Run fmt, lint, and test"
	@echo ""
	@echo "Run:"
	@echo "  make run         - Run release binary"
	@echo "  make run-dev     - Run debug binary"
	@echo "  make run-dry     - Run in dry-run mode"
	@echo ""
	@echo "Docker:"
	@echo "  make docker      - Build Docker image"
	@echo "  make docker-run  - Start with docker-compose"
	@echo "  make docker-stop - Stop docker-compose"
	@echo ""
	@echo "Other:"
	@echo "  make clean       - Clean build artifacts"
	@echo "  make doc         - Generate and open docs"
