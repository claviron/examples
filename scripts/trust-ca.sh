#!/usr/bin/env bash
# Trust edge-server's demo CA and map its hosts to this machine, to open them in
# a browser. Asks for your password; changes your trust store and /etc/hosts.
#
#   scripts/trust-ca.sh             trust the CA and map the hosts
#   scripts/trust-ca.sh --remove    undo both
#   scripts/trust-ca.sh --dry-run   print what would run
set -euo pipefail

SCRIPTS="$(cd -P -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPTS/lib/layout.sh"
EDGE="$(examples_root "$SCRIPTS")/edge-server"
TLS="$EDGE/config/tls.yaml"
MARK="# claviron-examples"
LINUX_NAME="claviron-examples.crt"

remove=false
dry_run=false
for argument in "$@"; do
  case "$argument" in
    --remove) remove=true ;;
    --dry-run) dry_run=true ;;
    *) printf 'Unknown option: %s\n' "$argument" >&2; exit 2 ;;
  esac
done

run() {
  if $dry_run; then printf '+ %s\n' "$*"; else "$@"; fi
}

# What tls.yaml names is what the certificates cover: one source for both.
store="$(sed -n 's/^ *dir: *//p' "$TLS" | head -1)"
authority="$(sed -n 's/^ *local: *//p' "$TLS" | head -1)"
hosts="$(sed -n '/^certs:/,/^[^ ]/s/^ *- *\([^ ]*\) *$/\1/p' "$TLS")"
ca="$EDGE/$store/ca/$authority/ca.crt"

map_hosts() {
  local current
  current="$(grep -vF "$MARK" /etc/hosts || true)"
  if ! $remove; then
    for host in $hosts; do
      current+=$'\n'"127.0.0.1 $host $MARK"
    done
  fi
  if $dry_run; then
    printf '+ write /etc/hosts:\n'
    printf '%s\n' "$current" | grep -F "$MARK" || printf '  (no example hosts)\n'
  else
    printf '%s\n' "$current" | sudo tee /etc/hosts >/dev/null
  fi
}

trust_macos() {
  local keychain="$HOME/Library/Keychains/login.keychain-db" fingerprint
  if $remove; then
    fingerprint="$(openssl x509 -in "$ca" -noout -fingerprint -sha1 | cut -d= -f2 | tr -d :)"
    run security delete-certificate -Z "$fingerprint" "$keychain"
  else
    run security add-trusted-cert -r trustRoot -k "$keychain" "$ca"
  fi
}

trust_linux() {
  if command -v update-ca-certificates >/dev/null; then
    local anchors=/usr/local/share/ca-certificates refresh=(sudo update-ca-certificates)
  elif command -v update-ca-trust >/dev/null; then
    local anchors=/etc/pki/ca-trust/source/anchors refresh=(sudo update-ca-trust)
  else
    printf 'No system trust tool found; trust %s by hand.\n' "$ca" >&2
    exit 1
  fi
  if $remove; then
    run sudo rm -f "$anchors/$LINUX_NAME"
  else
    run sudo cp "$ca" "$anchors/$LINUX_NAME"
  fi
  run "${refresh[@]}"
}

if [[ ! -f "$ca" ]]; then
  printf 'No CA at %s. Start edge-server once: it makes it at boot.\n' "$ca" >&2
  exit 1
fi

case "$(uname -s)" in
  Darwin) trust_macos ;;
  Linux) trust_linux ;;
  *) printf 'Trust %s by hand on this system.\n' "$ca" >&2 ;;
esac
map_hosts
