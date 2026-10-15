#!/usr/bin/env bash
set -euo pipefail

echo "==> mon install"

if ! command -v go >/dev/null 2>&1; then
  echo "error: Go toolchain not found. Install from https://go.dev/dl/" >&2
  exit 1
fi

repo_dir="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_dir"

bin_dir="${BIN_DIR:-$HOME/.local/bin}"
mkdir -p "$bin_dir"

echo "==> building release binary"
make build

echo "==> installing mon to $bin_dir"
cp bin/mon "$bin_dir/"

echo ""
echo "installed: $bin_dir/mon"
echo "try: mon doctor"
