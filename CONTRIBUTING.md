# Contributing

## Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust stable + wasm32v1-none | stable | `rustup target add wasm32v1-none` |
| stellar-cli | v28+ | `curl -fsSL https://github.com/stellar/stellar-cli/raw/main/install.sh \| sh` |
| Node.js | 18+ | [nodejs.org](https://nodejs.org) |
| npm | 9+ | bundled with Node.js |

## Local Setup

```bash
git clone https://github.com/unajiogenyi05-hub/soroban-vesting-platform
cd soroban-vesting-platform
cp .env.example .env
cd backend && npm install
```

## Building Contracts

```bash
stellar contract build   # WASM → target/wasm32v1-none/release/
```

## Running Tests

```bash
cargo test --all          # Rust contract tests
cd backend && npm test    # Backend tests
```

## Running CI Checks Locally

Run in this order before pushing — all five must pass:

```bash
cargo fmt --all -- --check
cargo clippy --all --target wasm32v1-none -- -D warnings
cargo test --all --locked
cd backend && npm run lint
cd backend && npm test
```

## Deploying to Testnet

See the **Deploy to Testnet** section in [README.md](README.md) for the full
walkthrough, including `stellar contract deploy` commands for Token, Vesting,
and Multisig. Testnet only — no real value.

## Code Style

**Rust:** code must pass `cargo clippy --all --target wasm32v1-none -- -D warnings`
with no warnings. Add `///` doc comments on every public function and type.

**JavaScript:** ESLint is configured in `backend/.eslintrc.json`. Run
`npm run lint` before committing; fix all errors.

## Branch Naming

| Prefix | Use for |
|--------|---------|
| `feat/` | new features |
| `fix/` | bug fixes |
| `docs/` | documentation only |
| `ci/` | CI/CD pipeline changes |
| `test/` | tests only |

Example: `feat/add-partial-claim`

## Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <short description>
```

Examples:

```
feat(vesting): add partial-claim support
fix(token): prevent double-pause panic
docs(readme): update testnet deploy steps
ci: pin stellar-cli to v28 in workflow
test(multisig): add below-threshold execute test
```

## PR Conventions

- Link the related issue with `Closes #N` in the PR description.
- Keep PRs focused on a single concern; split unrelated changes into separate PRs.
- Include a brief description of what changed and how it was tested.
- Request review only after all CI jobs are green.

## Claiming an Issue

1. Comment on the issue expressing your intent.
2. Wait for a maintainer to assign the issue to you before starting work.
3. No response after a few days? Follow up on the same thread.

## CI Requirement

**A PR is only mergeable when CI is green on ALL jobs.** Fix failing checks
before requesting a review — do not ask for a merge with a red CI.
