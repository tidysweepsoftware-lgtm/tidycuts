#!/usr/bin/env bash
# The checks every change must pass before it is committed: formatting,
# clippy with warnings as errors, and the whole test suite. Stops at the
# first failure and exits non-zero.
#
# CI uses the newest stable Rust, whose clippy can be stricter than an older
# one. To check with a specific version, pass it: scripts/check.sh +1.99.0
set -euo pipefail
cd "$(dirname "$0")/.."
toolchain="${1:-}"
cargo $toolchain fmt --all
cargo $toolchain clippy --workspace --all-targets -- -D warnings
cargo $toolchain test --workspace
echo "All checks passed."
