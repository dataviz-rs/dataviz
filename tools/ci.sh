#!/bin/sh
# The checks of .github/workflows (rust.yml, python.yml), run locally
# before pushing: formatting, clippy (all targets and crates), tests, docs.
set -eu
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo clippy -p dataviz-mcp --all-targets -- -D warnings
cargo clippy -p dataviz-py -- -D warnings
cargo test --all-features
cargo test -p dataviz-mcp
cargo build --no-default-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
if [ -x python/.venv/bin/maturin ]; then
  (cd python && .venv/bin/maturin develop -q --release && .venv/bin/python -m pytest -q tests)
fi
echo "all CI checks passed"
