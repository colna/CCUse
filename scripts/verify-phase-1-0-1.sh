#!/usr/bin/env bash
# Phase 1.0.1 demo verification (T1.0.1.27).
#
# Boots the proxy on a fresh ephemeral port, exercises every wire
# contract Phase 1.0.1 ships, and prints PASS/FAIL per check. End-to-
# end completion-routing is intentionally not exercised here — the
# `/v1/*` handlers still return 503 (`providers_not_configured`)
# until T1.0.2.15 wires `SwitchEngine` into the route layer.
#
# Run from the repo root: `bash scripts/verify-phase-1-0-1.sh`.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

CARGO_TOML="${ROOT_DIR}/apps/desktop/src-tauri/Cargo.toml"
EXAMPLE="${ROOT_DIR}/apps/desktop/src-tauri/examples/run_proxy.rs"

PASS=0
FAIL=0

green() { printf "\033[32m%s\033[0m\n" "$*"; }
red()   { printf "\033[31m%s\033[0m\n" "$*"; }
gray()  { printf "\033[90m%s\033[0m\n" "$*"; }

check() {
  local label="$1" cond="$2"
  if [[ "${cond}" == "ok" ]]; then
    green "  ✓ ${label}"; PASS=$((PASS + 1))
  else
    red   "  ✗ ${label}"; FAIL=$((FAIL + 1))
  fi
}

mkdir -p "$(dirname "${EXAMPLE}")"
cat > "${EXAMPLE}" <<'EOF'
//! Boots the proxy with auth on an ephemeral port and prints
//! `<base_url> <api_key>` so a verifier script can curl it.
//! Exits cleanly when stdin closes (script kills the child).

use std::time::Duration;

use ccuse_desktop_lib::proxy::ProxyRuntime;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = ProxyRuntime::new(0, 1);
    let config = runtime.start().await?;
    println!("{} {}", config.base_url, config.api_key);
    // Keep the runtime alive until the parent process closes stdin
    // (or kills us). 60s ceiling is a belt-and-braces exit.
    tokio::time::sleep(Duration::from_secs(60)).await;
    runtime.stop().await?;
    Ok(())
}
EOF

gray "[1/4] Building proxy verification binary…"
(
  cd "$(dirname "${CARGO_TOML}")"
  cargo build --quiet --example run_proxy
)

LOG=$(mktemp)
gray "[2/4] Booting proxy on ephemeral port…"
(
  cd "$(dirname "${CARGO_TOML}")"
  cargo run --quiet --example run_proxy
) > "${LOG}" 2>&1 &
PROXY_PID=$!

trap 'kill ${PROXY_PID} 2>/dev/null || true; rm -f "${LOG}"' EXIT

# Wait up to 5s for the proxy to print its base + key line.
for _ in $(seq 1 50); do
  if [[ -s "${LOG}" ]]; then break; fi
  sleep 0.1
done
if [[ ! -s "${LOG}" ]]; then
  red "proxy never printed its config; aborting"
  exit 1
fi

read -r BASE_URL API_KEY < "${LOG}"
gray "    base = ${BASE_URL}"
gray "    key  = ${API_KEY:0:14}…${API_KEY: -4}"

gray "[3/4] Probing wire contracts…"

# /healthz is unauthenticated.
status=$(curl -s -o /dev/null -w "%{http_code}" "${BASE_URL}/healthz")
[[ "${status}" == "200" ]] && check "/healthz returns 200" ok || check "/healthz returns 200 (got ${status})" fail

# /v1/models without a key is rejected.
status=$(curl -s -o /dev/null -w "%{http_code}" "${BASE_URL}/v1/models")
[[ "${status}" == "401" ]] && check "/v1/models requires api key (401)" ok || check "/v1/models requires api key (got ${status})" fail

# /v1/models with the right Bearer key is accepted.
status=$(curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer ${API_KEY}" "${BASE_URL}/v1/models")
[[ "${status}" == "200" ]] && check "/v1/models accepts Bearer (200)" ok || check "/v1/models accepts Bearer (got ${status})" fail

# /v1/chat/completions with auth = 503 providers_not_configured (Phase 1.0.1 stub).
body=$(curl -s -H "Authorization: Bearer ${API_KEY}" -H "Content-Type: application/json" \
  -d '{"model":"gpt-5.5-instant","messages":[]}' \
  "${BASE_URL}/v1/chat/completions")
echo "${body}" | grep -q '"providers_not_configured"' && \
  check "/v1/chat/completions returns 503 stub with OpenAI-shaped error" ok || \
  check "/v1/chat/completions stub body (got: ${body})" fail

# x-api-key (Anthropic style) on /v1/messages.
status=$(curl -s -o /dev/null -w "%{http_code}" \
  -H "x-api-key: ${API_KEY}" \
  -H "Content-Type: application/json" \
  -d '{"model":"claude","messages":[]}' \
  "${BASE_URL}/v1/messages")
[[ "${status}" == "503" ]] && check "/v1/messages accepts x-api-key (503 stub)" ok || check "/v1/messages stub status (got ${status})" fail

# CORS: foreign origin should NOT receive Access-Control-Allow-Origin.
allow=$(curl -sI -X OPTIONS \
  -H "Origin: https://evil.example.com" \
  -H "Access-Control-Request-Method: POST" \
  "${BASE_URL}/v1/chat/completions" | tr -d '\r' | awk -F': ' 'tolower($1)=="access-control-allow-origin"{print $2}')
[[ -z "${allow}" ]] && check "CORS rejects foreign origin (no ACAO)" ok || check "CORS rejects foreign origin (saw ACAO=${allow})" fail

gray "[4/4] Summary"
gray "    pass=${PASS}  fail=${FAIL}"

if [[ "${FAIL}" -gt 0 ]]; then
  red "Phase 1.0.1 verification FAILED"
  exit 1
fi
green "Phase 1.0.1 verification PASSED"
