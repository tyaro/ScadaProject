#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

TAG_SERVER_PORT="${TAG_SERVER_PORT:-18780}"
MQTT_PORT="${MQTT_PORT:-19999}"
TAG_SERVER_ADDR="127.0.0.1:${TAG_SERVER_PORT}"
TAG_SERVER_URL="http://${TAG_SERVER_ADDR}"
LOG_FILE="${LOG_FILE:-/tmp/preview-runtime-mqtt-resync.log}"

cleanup() {
  if [[ -n "${PREVIEW_PID:-}" ]]; then
    kill "${PREVIEW_PID}" >/dev/null 2>&1 || true
  fi
  if [[ -n "${TAG_SERVER_PID:-}" ]]; then
    kill "${TAG_SERVER_PID}" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

cargo build -p tag-server -p preview-runtime >/dev/null

target/debug/tag-server --serve --addr "${TAG_SERVER_ADDR}" >/tmp/tag-server-mqtt-resync.log 2>&1 &
TAG_SERVER_PID=$!
sleep 1

target/debug/preview-runtime \
  --snapshot-screen \
  --screen config/screens/mock-main.screen.json \
  --tag-server "${TAG_SERVER_URL}" \
  --mqtt-subscribe \
  --mqtt-host 127.0.0.1 \
  --mqtt-port "${MQTT_PORT}" \
  --mqtt-timeout-secs 1 \
  >"${LOG_FILE}" 2>&1 &
PREVIEW_PID=$!

sleep 4
kill "${PREVIEW_PID}" >/dev/null 2>&1 || true
wait "${PREVIEW_PID}" >/dev/null 2>&1 || true

grep -q "preview-runtime mqtt-subscribe receive failed" "${LOG_FILE}"
grep -q "preview-runtime mqtt-subscribe refreshing snapshot" "${LOG_FILE}"
grep -q "snapshot values=" "${LOG_FILE}"
grep -q "preview-runtime mqtt-subscribe reconnect backoff=1s" "${LOG_FILE}"

echo "ok: preview-runtime mqtt resync logs verified (${LOG_FILE})"
