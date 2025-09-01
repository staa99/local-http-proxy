#!/usr/bin/env bash
set -euo pipefail

# End-to-end test for local-http-proxy
# - Builds the binary
# - Uses a temporary config file so it won't affect user config
# - Starts ephemeral upstream HTTP servers that echo their name and request path
# - Exercises `list`, `add`, `remove`, `set-mode` commands
# - Starts the proxy and verifies routing in Path and Domain modes
# - Verifies 404 (route not found) and 502 (bad gateway) behaviors

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$HERE/../.." && pwd)"
BIN_DEBUG="$REPO_ROOT/target/debug/local-http-proxy"
BIN_RELEASE="$REPO_ROOT/target/release/local-http-proxy"

log() { printf "\n[INFO] %s\n" "$*"; }
fail() { printf "\n[FAIL] %s\n" "$*"; exit 1; }
pass() { printf "\n[PASS] %s\n" "$*"; }

a_contains_b() { case "$1" in *"$2"*) return 0;; *) return 1;; esac }

# Find a free localhost TCP port using Python
free_port() {
  python3 - <<'PY'
import socket
s=socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
s.close()
PY
}

# Start a tiny upstream HTTP server that echoes service name and path.
# Args: <name> <port>
start_upstream() {
  local name="$1"; local port="$2"
  # Write a temp python file to avoid heredoc backgrounding issues on macOS bash
  local script="$TMP/up_${name}.py"
  cat > "$script" <<'PY'
import os
from http.server import BaseHTTPRequestHandler, HTTPServer

name = os.environ.get('UPSTREAM_NAME', 'svc')
port = int(os.environ['UPSTREAM_PORT'])

class H(BaseHTTPRequestHandler):
    def _ok(self):
        self.send_response(200)
        self.send_header('Content-Type','text/plain')
        self.end_headers()
        self.wfile.write(f"SVC={name} PATH={self.path}".encode())
    do_GET = _ok
    do_POST = _ok
    do_PUT = _ok
    do_DELETE = _ok
    def log_message(self, fmt, *args):
        return

HTTPServer(('127.0.0.1', port), H).serve_forever()
PY
  UPSTREAM_NAME="$name" UPSTREAM_PORT="$port" python3 -u "$script" >/dev/null 2>&1 &
  echo $!
}

# Wait until a URL returns the expected HTTP status (default 200) or timeout
wait_for_http() {
  local url="$1"; local expected="${2:-200}"; local timeout="${3:-10}"
  local start=$(date +%s)
  while true; do
    code=$(curl -sS --max-time 2 -o /dev/null -w "%{http_code}" "$url" || true)
    if [[ "$code" == "$expected" ]]; then return 0; fi
    now=$(date +%s); if (( now - start > timeout )); then return 1; fi
    sleep 0.2
  done
}

# Kill PIDs if running
kill_if_running() { for pid in "$@"; do [[ -n "${pid:-}" ]] && kill "$pid" 2>/dev/null || true; done; }

main() {
  log "Building project (debug)"
  (cd "$REPO_ROOT" && cargo build -q)

  local BIN
  if [[ -x "$BIN_DEBUG" ]]; then BIN="$BIN_DEBUG"; elif [[ -x "$BIN_RELEASE" ]]; then BIN="$BIN_RELEASE"; else fail "Binary not found after build"; fi
  log "Binary: $BIN"

  local TMP
  TMP="$(mktemp -d)"
  trap 'kill_if_running ${UP1_PID:-} ${UP2_PID:-} ${PROXY_PID_1:-} ${PROXY_PID_2:-}; rm -rf "$TMP"' EXIT INT TERM

  local CFG="$TMP/config.json"

  # Start two upstream services on random ports
  local UP1_PORT UP2_PORT DOWN_PORT
  UP1_PORT=$(free_port)
  UP2_PORT=$(free_port)
  DOWN_PORT=$(free_port) # we intentionally do NOT start a server here to exercise 502

  log "Starting upstreams: app=$UP1_PORT, svc=$UP2_PORT"
  UP1_PID=$(start_upstream app "$UP1_PORT")
  UP2_PID=$(start_upstream svc "$UP2_PORT")

  wait_for_http "http://127.0.0.1:$UP1_PORT/ping" 200 10 || fail "Upstream app not responding"
  wait_for_http "http://127.0.0.1:$UP2_PORT/ping" 200 10 || fail "Upstream svc not responding"

  # 1) Config basics: list -> add -> list -> remove (missing) -> set-mode
  log "Config: initial list (should create default config)"
  out="$($BIN --config-file "$CFG" list || true)"
  a_contains_b "$out" "Mode: path" || fail "list should show default mode=path"
  a_contains_b "$out" "Routes:" || fail "list should print routes header"
  a_contains_b "$out" "No routes configured" || fail "list should say no routes"

  log "Config: add routes (app -> :$UP1_PORT, /Svc -> 127.0.0.1:$UP2_PORT, down -> :$DOWN_PORT)"
  out="$($BIN --config-file "$CFG" add app "$UP1_PORT")"
  a_contains_b "$out" "Added route: app → http://localhost:$UP1_PORT" || fail "add app output mismatch: $out"

  out="$($BIN --config-file "$CFG" add /Svc "127.0.0.1:$UP2_PORT")"
  a_contains_b "$out" "Added route: svc → http://127.0.0.1:$UP2_PORT" || fail "add svc output mismatch: $out"

  out="$($BIN --config-file "$CFG" add down ":$DOWN_PORT")"
  a_contains_b "$out" "Added route: down → http://localhost:$DOWN_PORT" || fail "add down output mismatch: $out"

  log "Config: list after adds"
  out="$($BIN --config-file "$CFG" list)"
  a_contains_b "$out" "Routes:" || fail "list after adds should print routes header"
  a_contains_b "$out" "app → http://localhost:$UP1_PORT" || fail "list missing app route"
  a_contains_b "$out" "svc → http://127.0.0.1:$UP2_PORT" || fail "list missing svc route"
  a_contains_b "$out" "down → http://localhost:$DOWN_PORT" || fail "list missing down route"

  log "Config: remove missing route"
  out="$($BIN --config-file "$CFG" remove nope)"
  a_contains_b "$out" "No route found for 'nope'" || fail "remove missing message mismatch"

  log "Config: set mode to path (idempotent)"
  out="$($BIN --config-file "$CFG" set-mode path)"
  a_contains_b "$out" "Proxy mode set to: path" || fail "set-mode path message mismatch"

  # 2) Path mode proxy
  local PROXY_PORT_1
  PROXY_PORT_1=$(free_port)
  log "Starting proxy in Path mode on port $PROXY_PORT_1"
  "$BIN" --config-file "$CFG" start --port "$PROXY_PORT_1" &
  PROXY_PID_1=$!

  # Wait until the proxy can route to /app/health
  wait_for_http "http://127.0.0.1:$PROXY_PORT_1/app/health" 200 15 || { kill_if_running "$PROXY_PID_1"; fail "proxy (path mode) did not become ready"; }

  log "Path mode: route /app/hello?x=1"
  body="$(curl -sS "http://127.0.0.1:$PROXY_PORT_1/app/hello?x=1")"
  a_contains_b "$body" "SVC=app" || fail "expected upstream app to handle /app/*"
  a_contains_b "$body" "PATH=/hello?x=1" || fail "expected prefix to be stripped in path mode"

  log "Path mode: route /svc -> / (root on upstream)"
  body="$(curl -sS "http://127.0.0.1:$PROXY_PORT_1/svc")"
  a_contains_b "$body" "SVC=svc" || fail "expected upstream svc to handle /svc"
  a_contains_b "$body" "PATH=/" || fail "expected /svc to map to / on upstream"

  log "Path mode: unknown route -> 404"
  code=$(curl -sS -o /dev/null -w "%{http_code}" "http://127.0.0.1:$PROXY_PORT_1/unknown")
  [[ "$code" == "404" ]] || fail "expected 404 for unknown route (got $code)"

  log "Path mode: down route -> 502"
  code=$(curl -sS -o /dev/null -w "%{http_code}" "http://127.0.0.1:$PROXY_PORT_1/down")
  [[ "$code" == "502" ]] || fail "expected 502 for down route (got $code)"

  kill_if_running "$PROXY_PID_1"

  # 3) Domain mode proxy
  log "Config: set mode to domain"
  out="$($BIN --config-file "$CFG" set-mode domain)"
  a_contains_b "$out" "Proxy mode set to: domain" || fail "set-mode domain message mismatch"

  local PROXY_PORT_2
  PROXY_PORT_2=$(free_port)
  log "Starting proxy in Domain mode on port $PROXY_PORT_2"
  "$BIN" --config-file "$CFG" start --port "$PROXY_PORT_2" &
  PROXY_PID_2=$!

  # Use /app/health with Host header; expect 200
  wait_for_http "http://127.0.0.1:$PROXY_PORT_2/app/health" 404 5 || true # might be 404 prior to host header, just to wait a bit
  # Ensure routing works once up
  for i in {1..50}; do
    body=$(curl -sS -H "Host: app.localhost" "http://127.0.0.1:$PROXY_PORT_2/ping?z=9" || true)
    if a_contains_b "$body" "SVC=app" && a_contains_b "$body" "PATH=/ping?z=9"; then break; fi
    sleep 0.2
    [[ "$i" -eq 50 ]] && { kill_if_running "$PROXY_PID_2"; fail "proxy (domain mode) did not route app"; }
  done

  log "Domain mode: route to svc.localhost -> /"
  body="$(curl -sS -H "Host: svc.localhost" "http://127.0.0.1:$PROXY_PORT_2/")"
  a_contains_b "$body" "SVC=svc" || fail "expected upstream svc to handle host svc.localhost"
  a_contains_b "$body" "PATH=/" || fail "expected path preserved as / in domain mode"

  log "Domain mode: unknown host -> 404"
  code=$(curl -sS -o /dev/null -w "%{http_code}" -H "Host: no.localhost" "http://127.0.0.1:$PROXY_PORT_2/")
  [[ "$code" == "404" ]] || fail "expected 404 for unknown host (got $code)"

  log "Domain mode: down host -> 502"
  code=$(curl -sS -o /dev/null -w "%{http_code}" -H "Host: down.localhost" "http://127.0.0.1:$PROXY_PORT_2/")
  [[ "$code" == "502" ]] || fail "expected 502 for down host (got $code)"

  kill_if_running "$PROXY_PID_2"

  pass "All e2e tests passed"
}

main "$@"
