#!/usr/bin/env bash
set -eo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

usage() {
  cat <<'HELP'
Usage: scripts/run.sh [--start|--run|--status|--stop] [--release] [application options...] APP_NAME

Run an existing Claviron application binary from its own directory.
APP_NAME is the final argument and names both the directory and binary.

  --run       Run in the foreground (default).
  --start     Run in the background.
  --status    Show the recorded process.
  --stop      Request graceful shutdown of the recorded process.
  --release   Select target/release instead of target/debug.

All other options are forwarded unchanged. The application owns its defaults.
No build, dependency installation or source overrides are performed.

Environment:
  CARGO_TARGET_DIR  Existing Cargo output directory (default: APP_NAME/target).
  RUN_LOG_DIR      Launcher PID/log directory (default: APP_NAME/logs).

Examples:
  scripts/run.sh app-name
  scripts/run.sh --release --workers 4 pentaglot
  scripts/run.sh --start --release fastapi-app
  scripts/run.sh --fleet demo --workers 2 octane-app
  scripts/run.sh --status fastapi-app
  scripts/run.sh --stop fastapi-app
  scripts/run.sh --help axum-app
HELP
}

if [[ $# -eq 1 && ( "$1" == --help || "$1" == -h ) ]]; then
  usage
  exit 0
fi

# This copy lives inside its application: the application is the default.
if [[ "${!#}" != "$(basename "$ROOT")" ]]; then
  set -- "$@" "$(basename "$ROOT")"
fi

arguments=("$@")
app_index=$(($# - 1))
app="${arguments[$app_index]}"
unset 'arguments[app_index]'
if [[ ! "$app" =~ ^[a-zA-Z0-9][a-zA-Z0-9_-]*$ ]]; then
  printf 'APP_NAME must be the final argument. Use --help for usage.\n' >&2
  exit 2
fi
APP_ROOT="$ROOT/$app"
[[ "$(basename "$ROOT")" == "$app" ]] && APP_ROOT="$ROOT"
if [[ ! -f "$APP_ROOT/Cargo.toml" ]]; then
  printf '%s has no standalone Claviron binary.\n' "$app" >&2
  exit 2
fi

action=run
profile=debug
server_args=()
for argument in "${arguments[@]}"; do
  case "$argument" in
    --run|--start|--status|--stop) action="${argument#--}" ;;
    --release) profile=release ;;
    *) server_args+=("$argument") ;;
  esac
done

cd "$APP_ROOT"
source "$ROOT/scripts/lib/application.sh"
application_init "$APP_ROOT" "$app" "${RUN_LOG_DIR:-$APP_ROOT/logs}" \
  "$action" "${server_args[@]}"

binary="${CARGO_TARGET_DIR:-$APP_ROOT/target}/$profile/$app"
if [[ ! -x "$binary" ]]; then
  printf 'Executable not found: %s\nPrepare the application before running it.\n' "$binary" >&2
  exit 1
fi

application_guard
application_exec "$binary" "${server_args[@]}"
