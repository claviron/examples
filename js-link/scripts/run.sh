#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../" && pwd)"
APP_ROOT="$REPO_ROOT"
action=run
case "${1:-}" in start|run|status|stop) action="$1"; shift ;; esac
runtime=deno
case "${1:-}" in node|bun|deno) runtime="$1"; shift ;; esac
cd "$APP_ROOT"
source "$REPO_ROOT/scripts/lib/application.sh"
application_init "$APP_ROOT" "js-link-$runtime" "$APP_ROOT/logs" "$action" "$@"
set -- "${APP_ARGS[@]}"
sync=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-build) ;; # Kept for the shared examples launcher; no addon build occurs.
    --sync) sync=true ;;
    --release|--native) ;; # Shared launcher flags; the published addon is portable and prebuilt.
    --bench) export EXAMPLES_BENCH=1 ;;
    --help|-h)
      echo 'Usage: scripts/run.sh [start|run|status|stop] [node|bun|deno] [--sync] [--bench]'
      echo '--sync installs locked npm dependencies, including the published native addon.'
      echo 'ORBIT_WORKERS (or WORKERS) selects child processes; default 8. ORBIT_FLEET and ORBIT_APP select the Link target.'
      echo 'Each runtime has a separate PID/log record. Stop forwards SIGTERM only to its owned children.'
      exit 0 ;;
    *) printf 'Unknown option: %s\n' "$1" >&2; exit 2 ;;
  esac
  shift
done
application_guard
if [[ "$sync" == true ]]; then npm ci; fi
export RUST_LOG="${RUST_LOG:-warn}"
APP_CONFIG_INFO="$APP_ROOT/.env (benchmark mode uses the supplied environment)"
APP_FLEET_INFO="${ORBIT_FLEET:-js}"
application_exec "$(command -v node)" "$REPO_ROOT/scripts/lib/javascript.mjs" \
  "$runtime" "$APP_ROOT" "${ORBIT_WORKERS:-${WORKERS:-8}}"
