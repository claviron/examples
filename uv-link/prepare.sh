#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
: "${ORBIT_C_PREFIX:?Set ORBIT_C_PREFIX to the extracted Orbit C release archive}"
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release -DCMAKE_PREFIX_PATH="$ORBIT_C_PREFIX"
cmake --build build
