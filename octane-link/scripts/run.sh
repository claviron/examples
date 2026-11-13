#!/usr/bin/env bash
set -euo pipefail

examples="$(cd -P -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
launcher="$examples/scripts/run.sh"
if [[ ! -x "$launcher" ]]; then
  printf 'Shared launcher missing: %s. See the Claviron examples repository.\n' "$launcher" >&2
  exit 1
fi
# The old per-example launchers also accepted the application name last.
if [[ $# -gt 0 && "${!#}" == octane-link ]]; then
  set -- "${@:1:$#-1}"
fi
exec "$launcher" "$@" octane-link
