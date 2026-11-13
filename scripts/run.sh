#!/usr/bin/env bash
set -eo pipefail

SCRIPTS="$(cd -P -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPTS/lib/layout.sh"
ROOT="$(examples_root "$SCRIPTS")"

usage() {
  cat <<'HELP'
Usage: scripts/run.sh [--build] [--run|--start|--status|--stop|--prepare] [options...] APP_NAME

Run a Claviron example from its own directory. APP_NAME is the final argument.

  --run       Run in the foreground (default).
  --start     Run in the background.
  --status    Show the recorded process.
  --stop      Request graceful shutdown of the recorded process.
  --build     Build only, or build first when combined with --run or --start.
  --prepare   Run the example's prepare.sh, or install and build js-link.
  --release   Select target/release instead of target/debug for Rust binaries.
  --runtime=deno|node|bun  Select the js-link runtime (default: deno).

Other options are forwarded to the application or its preparation script.

Environment:
  CARGO_TARGET_DIR  Existing Cargo output directory (default: APP_NAME/target).
  RUN_LOG_DIR      Launcher PID/log directory (default: APP_NAME/logs).

Examples:
  scripts/run.sh --start --release fastapi-link
  scripts/run.sh --build --start --release edge-server
  scripts/run.sh --status fastapi-link
  scripts/run.sh --prepare c-link
  scripts/run.sh --start --runtime=node js-link
  scripts/run.sh java-link
HELP
}

if [[ $# -eq 0 || ( $# -eq 1 && ( "$1" == --help || "$1" == -h ) ) ]]; then
  usage
  exit 0
fi

app="${!#}"
if [[ ! "$app" =~ ^[a-zA-Z0-9][a-zA-Z0-9_-]*$ ]]; then
  printf 'APP_NAME must be the final argument. Use --help for usage.\n' >&2
  exit 2
fi
APP_ROOT="$ROOT/$app"
if [[ "$app" != java-link && "$app" != js-link && ! -f "$APP_ROOT/Cargo.toml" &&
      ! -f "$APP_ROOT/go.mod" && ! -f "$APP_ROOT/main.c" ]]; then
  printf 'Unknown runnable example: %s. Use --help for usage.\n' "$app" >&2
  exit 2
fi

action=
build=false
profile=debug
app_args=()
for ((i = 1; i < $#; i++)); do
  argument="${!i}"
  case "$argument" in
    --run|--start|--status|--stop|--prepare) action="${argument#--}" ;;
    --build) build=true ;;
    --release) profile=release ;;
    *) app_args+=("$argument") ;;
  esac
done

prepare() {
  if [[ "$app" == js-link ]]; then
    if [[ $# -ne 0 ]]; then
      printf 'js-link preparation takes no arguments.\n' >&2
      return 2
    fi
    (cd "$APP_ROOT" && npm ci && npm run build)
  elif [[ -x "$APP_ROOT/prepare.sh" ]]; then
    "$APP_ROOT/prepare.sh" "$@"
  else
    printf 'No preparation script for %s. See the example README.\n' "$app" >&2
    return 2
  fi
}

if [[ "$action" == prepare ]]; then
  prepare "${app_args[@]}"
  exit
fi

launch_name="$app"
if [[ "$app" == js-link ]]; then
  runtime=deno
  sync=false
  for argument in "${app_args[@]}"; do
    case "$argument" in
      --runtime=deno|--runtime=node|--runtime=bun) runtime="${argument#*=}" ;;
      --sync) sync=true ;;
      --no-build|--native) ;; # Existing JS launcher flags.
      --bench) export EXAMPLES_BENCH=1 ;;
      --help|-h) usage; exit 0 ;;
      *) printf 'Unknown js-link option: %s\n' "$argument" >&2; exit 2 ;;
    esac
  done
  launch_name="js-link-$runtime"
fi

if $build; then
  case "$action" in
    status|stop) printf '%s cannot be combined with --build.\n' "--$action" >&2; exit 2 ;;
  esac
  if [[ -f "$APP_ROOT/Cargo.toml" ]]; then
    cd "$APP_ROOT"
    if [[ "$(uname -s)" == Linux && "$(df -PT . | awk 'NR == 2 {print $2}')" == 9p ]]; then
      if ! declare -F set_cargo_target >/dev/null && [[ -f "$HOME/.config/ai/set-cargo-target.sh" ]]; then
        source "$HOME/.config/ai/set-cargo-target.sh"
      fi
      if declare -F set_cargo_target >/dev/null; then set_cargo_target; fi
    fi
    cargo_args=(build)
    if [[ "$profile" == release ]]; then cargo_args+=(--release); fi
    if [[ -z "$action" ]]; then cargo_args+=("${app_args[@]}"); fi
    cargo "${cargo_args[@]}"
  elif [[ -z "$action" && "$app" != js-link ]]; then
    prepare "${app_args[@]}"
  else
    prepare
  fi
  if [[ -z "$action" ]]; then exit 0; fi
fi
cd "$APP_ROOT"
source "$SCRIPTS/lib/application.sh"
application_init "$APP_ROOT" "$launch_name" "${RUN_LOG_DIR:-$APP_ROOT/logs}" \
  "${action:-run}" "${app_args[@]}"

case "$app" in
  js-link)
    application_guard
    if [[ "$sync" == true && "$build" == false ]]; then npm ci; fi
    export RUST_LOG="${RUST_LOG:-warn}"
    APP_CONFIG_INFO="$APP_ROOT/.env (benchmark mode uses the supplied environment)"
    APP_FLEET_INFO="${ORBIT_FLEET:-orj}"
    application_exec node "$SCRIPTS/lib/javascript.mjs" \
      "$runtime" "$APP_ROOT" "${ORBIT_WORKERS:-${WORKERS:-8}}"
    ;;
  java-link)
    : "${ORBIT_JAVA_CLASSPATH:?Set ORBIT_JAVA_CLASSPATH to the Orbit Java API and native JARs}"
    if [[ ! -f build/java-link.jar ]]; then
      printf 'Run scripts/run.sh --prepare java-link first.\n' >&2
      exit 1
    fi
    application_guard
    application_exec "${JAVA_HOME:+$JAVA_HOME/bin/}java" --enable-native-access=ALL-UNNAMED \
      --illegal-native-access=deny -cp "build/java-link.jar:$ORBIT_JAVA_CLASSPATH" \
      example.javalink.Main "${app_args[@]}"
    ;;
  *)
    binary="${CARGO_TARGET_DIR:-$APP_ROOT/target}/$profile/$app"
    if [[ -f "$APP_ROOT/go.mod" || -f "$APP_ROOT/main.c" ]]; then
      binary="$APP_ROOT/build/$app"
    fi
    if [[ ! -x "$binary" ]]; then
      printf 'Executable not found: %s\nPrepare the application before running it.\n' "$binary" >&2
      exit 1
    fi
    application_guard
    application_exec "$binary" "${app_args[@]}"
    ;;
esac
