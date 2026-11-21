# ── Soroban Vesting Platform — Makefile ──────────────────────────────────────
# Usage: make <target>

CONTRACTS := vesting multisig token
NETWORK   ?= testnet
SOURCE    ?= $(shell grep SOURCE_ACCOUNT .env 2>/dev/null | cut -d= -f2)

.PHONY: help setup build build-optimized test fmt lint ci-check clean \
        deploy-testnet deploy-mainnet fund \
        backend-install backend-dev backend-start backend-test

# Default target
help:
	@echo ""
	@echo "  Soroban Vesting Platform"
	@echo ""
	@echo "  Contracts"
	@echo "    make setup            Install Rust target + stellar-cli"
	@echo "    make build            Build all contracts (WASM via stellar contract build)"
	@echo "    make build-optimized  Build + optimize WASM sizes"
	@echo "    make test             Run all contract unit tests"
	@echo "    make test-coverage    Run tarpaulin and produce HTML coverage report"
	@echo "    make fmt              Format Rust code"
	@echo "    make lint             fmt-check + clippy (--target wasm32v1-none)"
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
	rustup target add wasm32v1-none
	cargo install --locked stellar-cli --features opt

# ── Build ─────────────────────────────────────────────────────────────────────
build:
	stellar contract build

build-optimized: build
	@for c in $(CONTRACTS); do \
	  wasm="target/wasm32v1-none/release/$${c}.wasm"; \
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
	cargo clippy --all --target wasm32v1-none -- -D warnings

# ── CI check (mirrors all CI jobs locally) ────────────────────────────────────
ci-check: ## Run all CI checks locally (fmt, clippy, tests, backend lint+test)
	cargo fmt --all -- --check
	cargo clippy --all --target wasm32v1-none -- -D warnings
	cargo test --all --locked
	cd backend && npm run lint
	cd backend && npm test

# ── Environment ──────────────────────────────────────────────────────────────

# Verify required environment variables are set before deploying.
# Source .env first (make does not read .env automatically), e.g.:
#   set -a; . .env; set +a; make check-env
check-env:
	@test -n "$(VESTING_CONTRACT_ID)" || (echo "Error: VESTING_CONTRACT_ID is not set"; exit 1)
	@test -n "$(TOKEN_CONTRACT_ID)" || (echo "Error: TOKEN_CONTRACT_ID is not set"; exit 1)
	@test -n "$(MULTISIG_CONTRACT_ID)" || (echo "Error: MULTISIG_CONTRACT_ID is not set"; exit 1)
	@echo "All required env vars set."

# ── Test coverage ─────────────────────────────────────────────────────────────
test-coverage:
	cargo tarpaulin --all --engine llvm --out Html --output-dir coverage/
	@echo "Coverage report: coverage/tarpaulin-report.html"

# ── Clean ─────────────────────────────────────────────────────────────────────
clean:
	cargo clean

# ── Deploy ────────────────────────────────────────────────────────────────────
fund:
	./scripts/fund.sh $(SOURCE)

deploy-testnet: check-env
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
