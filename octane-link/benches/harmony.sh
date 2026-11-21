#!/usr/bin/env bash
# Harmony: Rust and PHP claim from one shared atomic under load, and the value
# is checked at the end.
#
#   benches/harmony.sh [options]
#
#   --url BASE        the application's base URL        (default https://127.0.0.1:8103)
#   --requests N      claims to send                    (default 1000000)
#   --clients C       connections                       (default 8)
#   --streams M       concurrent streams per connection (default 1)
#   --threads T       client threads                    (default: the number of clients)
#   --side WHICH      random, rust or php               (default random)
#   --seed S          seed of the random order          (default: the current time)
#
# The first request starts a new round, which sets the value to zero. Every
# claim after it, to the Rust route or to the PHP route, is one atomic add.
# When the load is done one more claim reads the value: it must equal the
# number of claims the server answered with success, in the same round.
# Exit status 0 means it did.
set -euo pipefail

url=https://127.0.0.1:8103
requests=1000000
clients=8
streams=1
threads=
side=random
seed=$(date +%s)

while (($#)); do
  case "$1" in
    --url) url="$2"; shift 2 ;;
    --requests) requests="$2"; shift 2 ;;
    --clients) clients="$2"; shift 2 ;;
    --streams) streams="$2"; shift 2 ;;
    --threads) threads="$2"; shift 2 ;;
    --side) side="$2"; shift 2 ;;
    --seed) seed="$2"; shift 2 ;;
    -h|--help) sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) printf 'unknown option: %s\n' "$1" >&2; exit 2 ;;
  esac
done
threads="${threads:-$clients}"
((threads > clients)) && threads="$clients"

for tool in h2load curl python3; do
  command -v "$tool" >/dev/null || { printf 'missing command: %s\n' "$tool" >&2; exit 2; }
done

rust="$url/_lumenize/rust/tiles"
php="$url/_lumenize/php/tiles"
work="$(mktemp -d "${TMPDIR:-/tmp}/harmony.XXXXXXXX")"
trap 'rm -rf -- "$work"' EXIT

# The order of the two routes: h2load walks this list again and again.
python3 - "$side" "$seed" "$rust" "$php" > "$work/urls" <<'PY'
import random, sys
side, seed, rust, php = sys.argv[1:]
if side == "rust":
    print(rust)
elif side == "php":
    print(php)
elif side == "random":
    random.seed(int(seed))
    for _ in range(200_000):
        print(random.choice((rust, php)))
else:
    sys.exit(f"unknown side: {side}")
PY

field() { python3 -c 'import json, sys; print(json.load(sys.stdin)[sys.argv[1]])' "$1"; }

round="$(curl -fsSk -X DELETE "$rust" | field round)"
printf 'round %s started; %s claims, %s side, %s connections x %s streams, seed %s\n' \
  "$round" "$requests" "$side" "$clients" "$streams" "$seed"

h2load -n "$requests" -c "$clients" -t "$threads" -m "$streams" -i "$work/urls" > "$work/load" 2>&1 || {
  cat "$work/load" >&2
  exit 2
}
grep -E '^(finished in|requests:|status codes:)' "$work/load"

succeeded="$(sed -n 's/^status codes: \([0-9]*\) 2xx.*/\1/p' "$work/load")"
answer="$(curl -fsSk "$rust")"
value="$(printf '%s' "$answer" | field index)"
ended="$(printf '%s' "$answer" | field round)"

printf 'answered with success: %s\nshared value:          %s\n' "$succeeded" "$value"
if [[ "$ended" != "$round" ]]; then
  printf 'NOT VERIFIED: the round changed from %s to %s during the run; something else started one\n' "$round" "$ended"
  exit 1
fi
if [[ "$value" != "$succeeded" ]]; then
  printf 'NOT VERIFIED: the value differs from the claims answered by %s\n' "$((value - succeeded))"
  exit 1
fi
printf 'VERIFIED: every claim answered with success is in the value, and nothing else is\n'
