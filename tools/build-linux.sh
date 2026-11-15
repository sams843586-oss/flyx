#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -s)" != Linux || "$(uname -m)" != x86_64 ]]; then
  echo 'This builder targets Linux x86_64 (amd64).' >&2
  exit 1
fi
# Override the Windows developer machine's local .cargo target-dir setting.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target-linux}"
cargo test --workspace --locked
cargo build --release --workspace --locked
python3 tools/package-linux.py --binaries "$CARGO_TARGET_DIR/release"
