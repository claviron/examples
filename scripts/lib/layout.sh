#!/usr/bin/env bash
# A split checkout links its top-level scripts directory to shared/scripts.
examples_root() {
  local root
  root="$(cd -P -- "$1/.." && pwd)"
  if [[ "$root/../scripts" -ef "$1" ]]; then
    root="$(cd -P -- "$root/.." && pwd)"
  fi
  printf '%s\n' "$root"
}
