#!/usr/bin/env bash
# Build lib/pkg for the web app. Uses rustup's rustc (not Homebrew's) so wasm32-unknown-unknown is available.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LIB="$ROOT/lib"

if ! command -v wasm-pack >/dev/null 2>&1; then
  echo "error: wasm-pack not found on PATH." >&2
  echo "  Install: cargo install wasm-pack" >&2
  echo "  or:     brew install wasm-pack" >&2
  exit 1
fi

if command -v rustup >/dev/null 2>&1; then
  rustup target add wasm32-unknown-unknown --toolchain stable >/dev/null 2>&1 || true
  TOOLCHAIN_BIN="$(dirname "$(cd "$LIB" && rustup which rustc)")"
  export PATH="${TOOLCHAIN_BIN}:${HOME}/.cargo/bin:${PATH}"
else
  echo "warning: rustup not found; wasm-pack may fail if Homebrew rustc lacks wasm32-unknown-unknown." >&2
  echo "  Install rustup: https://rustup.rs" >&2
fi

if command -v rustc >/dev/null 2>&1; then
  echo "[wasm-build] using $(command -v rustc) ($(rustc --version | cut -d' ' -f2))"
fi

cd "$LIB"
PKG_JSON="pkg/package.json"
# wasm-pack 0.13 reads pkg/package.json as HashMap<String, String> before wasm-opt;
# array fields (files, sideEffects, collaborators) trigger "expected a string, got sequence".
if [[ -f "$PKG_JSON" ]]; then
  cp "$PKG_JSON" "${PKG_JSON}.stub.bak"
  printf '%s\n' '{}' >"$PKG_JSON"
fi

set +e
wasm-pack build --target web --out-dir pkg
STATUS=$?
set -e

if [[ $STATUS -ne 0 && -f "${PKG_JSON}.stub.bak" ]]; then
  mv "${PKG_JSON}.stub.bak" "$PKG_JSON"
  exit "$STATUS"
fi
rm -f "${PKG_JSON}.stub.bak"
exit "$STATUS"
