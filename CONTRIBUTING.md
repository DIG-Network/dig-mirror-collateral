# Contributing to dig-mirror-collateral

Thanks for your interest in improving dig-mirror-collateral. This crate implements deterministic,
consensus-critical arithmetic — the collateral requirement for DIG mirror-coin advertisements must be
identical across every independent implementation. Please read this before opening a PR.

## Prerequisites

- [Rust](https://rustup.rs), minimum version **1.75.0** (declared in `Cargo.toml`).
- This crate has minimal dependencies (`serde` and `thiserror` only) and no special build-order
  prerequisites.

## Build & test

```sh
# build the crate
cargo build

# run the full test suite
cargo test --all-features
```

The test suite includes a golden-vector conformance test
([`tests/fixtures/golden_vectors.json`](tests/fixtures/golden_vectors.json)) covering ten epochs
(bootstrap, growth, participation shock, and recovery) — a reimplementation in any language must
reproduce every field from the census inputs alone.

**Note:** Arithmetic that panics in debug can return a wrong value in release. The CI runs tests in
both modes to catch this defect. Run `cargo test --all-features --release` locally before submitting.

## The gate (must pass before a PR is merged)

CI runs these on every PR (`.github/workflows/ci.yml`); run them locally first:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo nextest run --workspace --all-features --retries 2
cargo nextest run --workspace --all-features --retries 2 --release
cargo llvm-cov --workspace --all-features --summary-only --fail-under-lines 80
```

## Commit conventions

- Use clear, imperative commit subjects (e.g. `feat: …`, `fix: …`, `docs: …`, `test: …`). Follow
  [Conventional Commits](https://www.conventionalcommits.org).
- Keep one logical change per commit where practical.
- Every PR bumps the version in `Cargo.toml` and `Cargo.lock` (patch for a fix/docs, minor for a
  feature). The version-increment CI gate enforces this — a non-increasing version fails the PR.

## Where things live

This is a single, focused crate with no workspace. All Rust code lives in `src/`; conformance tests
and fixtures live in `tests/`.

## Security

For anything security-relevant, report vulnerabilities privately to the maintainer rather than
opening a public issue. This crate is consensus-critical: a single differing DIG base unit in any
epoch propagates into every later epoch. That is why floating point is forbidden and both debug and
release arithmetic are tested.

## Pull requests

1. Branch from `main`.
2. Make the gate green locally.
3. Open a PR with a clear description of the change and its rationale. Keep the diff focused.
4. Main is a protected branch — all PRs require a code review and all required status checks to be
   green before merge.
