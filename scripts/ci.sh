#!/usr/bin/env bash
# Run the CI checks: test display/, build slave/ and master/.
#
# Usage:
#   scripts/ci.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "Testing display..."
(cd "$REPO_ROOT/display" && cargo test)

echo "Building slave..."
(cd "$REPO_ROOT/slave" && cargo build --release)

echo "Building master..."
(cd "$REPO_ROOT/master" && cargo build --release)

echo "Done."
