# Kani Verification (DashFlow)

This directory documents how DashFlow uses **Kani** (Rust model checking) to verify
panic-freedom and functional properties of critical code paths.

## Install

Kani is installed as a Rust tool plus a one-time setup step:

```bash
cargo install --locked kani-verifier
kani setup
```

## Verify toolchain

From repo root:
```bash
./scripts/check_kani.sh
```

This script currently checks that the Kani toolchain is installed and runnable.

## Harnesses

Kani harnesses live under:
- `crates/dashflow/src/kani_harnesses/`

As harnesses are added, `./scripts/check_kani.sh` can be extended to run them in CI.

