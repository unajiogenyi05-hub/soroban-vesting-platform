# ── Soroban Vesting Platform — Makefile ──────────────────────────────────────
# Usage: make <target>

CONTRACTS := vesting multisig token
NETWORK   ?= testnet
SOURCE    ?= $(shell grep SOURCE_ACCOUNT .env 2>/dev/null | cut -d= -f2)

.PHONY: help setup build build-optimized test fmt lint clean \
        deploy-testnet deploy-mainnet fund \
        backend-install backend-dev backend-start backend-test

# Default target
help:
	@echo ""
	@echo "  Soroban Vesting Platform"
	@echo ""
	@echo "  Contracts"
	@echo "    make setup            Install Rust target + stellar-cli"
	@echo "    make build            Build all contracts (debug)"
	@echo "    make build-optimized  Build + optimize WASM sizes"
	@echo "    make test             Run all contract unit tests"
	@echo "    make fmt              Format Rust code"
	@echo "    make lint             fmt-check + clippy"
	@echo "    make clean            Remove build artifacts"
	@echo ""
	@echo "  Deploy"
	@echo "    make fund             Fund SOURCE_ACCOUNT on testnet"
	@echo "    make deploy-testnet   Deploy all contracts to testnet"
	@echo "    make deploy-mainnet   Deploy all contracts to mainnet"
	@echo ""
	@echo "  Backend"
	@echo "    make backend-install  npm ci in backend/"
	@echo "    make backend-dev      Start backend in dev mode (nodemon)"
	@echo "    make backend-start    Start backend in production mode"
	@echo "    make backend-test     Run backend tests"
	@echo ""

# ── Setup ─────────────────────────────────────────────────────────────────────
setup:
	rustup target add wasm32-unknown-unknown
	cargo install --locked stellar-cli --features opt

# ── Build ─────────────────────────────────────────────────────────────────────
build:
	cargo build --all --target wasm32-unknown-unknown --release

build-optimized: build
	@for c in $(CONTRACTS); do \
	  wasm="target/wasm32-unknown-unknown/release/$${c}.wasm"; \
	  if [ -f "$$wasm" ]; then \
	    stellar contract optimize --wasm "$$wasm"; \
	    echo "Optimized: $$wasm"; \
	  fi; \
	done

# ── Test ──────────────────────────────────────────────────────────────────────
test:
	cargo test --all

# ── Lint ──────────────────────────────────────────────────────────────────────
fmt:
	cargo fmt --all

lint:
	cargo fmt --all -- --check
	cargo clippy --all --all-targets -- -D warnings

# ── Clean ─────────────────────────────────────────────────────────────────────
clean:
	cargo clean

# ── Deploy ────────────────────────────────────────────────────────────────────
fund:
	./scripts/fund.sh $(SOURCE)

deploy-testnet:
	@for c in $(CONTRACTS); do \
	  ./scripts/deploy.sh $$c testnet; \
	done

deploy-mainnet:
	@echo "⚠  Deploying to MAINNET in 5 seconds. Ctrl-C to abort."
	@sleep 5
	@for c in $(CONTRACTS); do \
	  ./scripts/deploy.sh $$c mainnet; \
	done

# ── Backend ───────────────────────────────────────────────────────────────────
backend-install:
	cd backend && npm ci

backend-dev:
	cd backend && npm run dev

backend-start:
	cd backend && npm start

backend-test:
	cd backend && npm test
