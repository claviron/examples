#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
: "${ORBIT_C_DIR:?Set ORBIT_C_DIR to the checkout of the C binding}"
: "${ORBIT_C_LIBRARY_DIR:?Set ORBIT_C_LIBRARY_DIR to the native library directory}"
binding="$(cd -- "$ORBIT_C_DIR" && pwd)"
library="$(cd -- "$ORBIT_C_LIBRARY_DIR" && pwd)"
mkdir -p include
cp -- "$binding/include/orbit.h" include/orbit.h
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release \
  -DORBIT_C_LIBRARY_DIR="$library"
cmake --build build
echo "built: build/uv-link"
