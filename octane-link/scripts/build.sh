#!/usr/bin/env bash
# Build this example. Running it is the shared launcher's job, from the
# examples directory: scripts/run.sh octane-link
#
#   scripts/build.sh              debug build
#   scripts/build.sh --release    release build
#
# Other arguments go to Cargo, for example --features legacy-linux.
set -euo pipefail

examples="$(cd -P -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
launcher="$examples/scripts/run.sh"
if [[ ! -x "$launcher" ]]; then
  printf 'Shared launcher missing: %s. See the Claviron examples repository.\n' "$launcher" >&2
  exit 1
fi
exec "$launcher" --build "$@" octane-link
