#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

if [[ -s "${HOME}/.nvm/nvm.sh" ]]; then
  # Local developer shells may not have the project Node version active.
  # CI images can skip this and provide Node directly on PATH.
  # shellcheck disable=SC1091
  source "${HOME}/.nvm/nvm.sh"
  if ! nvm use 24 >/dev/null 2>&1; then
    echo "warn: nvm could not activate Node 24; falling back to PATH node"
  fi
fi

if ! command -v node >/dev/null 2>&1; then
  echo "error: node is required but not found on PATH"
  exit 1
fi

NODE_MAJOR="$(node -v | sed -E 's/^v([0-9]+).*/\1/')"
if [[ "$NODE_MAJOR" != "24" ]]; then
  echo "error: Node.js 24 is required (current: $(node -v))"
  exit 1
fi

echo "== cargo fmt --check =="
cargo fmt --check

echo "== cargo test =="
cargo test -p scada-core -p preview-runtime -p tag-server -p driver-manager -p mock-driver -p tauri-shell

echo "== runtime-ui check =="
(
  cd apps/runtime-ui
  npm run check
)

if [[ "${RUN_RUNTIME_UI_E2E:-0}" == "1" ]]; then
  echo "== runtime-ui e2e =="
  (
    cd apps/runtime-ui
    npm run test:e2e
  )
else
  echo "skip runtime-ui e2e (set RUN_RUNTIME_UI_E2E=1)"
fi

if [[ "${RUN_BIND_TESTS:-0}" == "1" ]]; then
  echo "== preview-runtime bind-dependent unit tests =="
  cargo test -p preview-runtime -- --ignored

  echo "== preview-runtime mqtt resync log check =="
  scripts/check_preview_runtime_mqtt_resync.sh
else
  echo "skip bind-dependent checks (set RUN_BIND_TESTS=1)"
fi
