#!/usr/bin/env bash
set -euxo pipefail

toolchain="$(awk -F '"' '/^channel/ { print $2; exit }' rust-toolchain.toml)"
rustup toolchain install --profile minimal "$toolchain"
rustup component add --toolchain "$toolchain" rustfmt clippy rust-analyzer
cargo fetch --locked
./scripts/doctor
./scripts/check
./scripts/test
