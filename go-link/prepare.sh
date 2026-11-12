#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
: "${ORBIT_GO_LIBRARY_DIR:?Set ORBIT_GO_LIBRARY_DIR to the directory containing libclaviron_go}"
library="$(cd -- "$ORBIT_GO_LIBRARY_DIR" && pwd)"
export CGO_ENABLED=1
export CGO_LDFLAGS="${CGO_LDFLAGS:-} -L${library} -Wl,-rpath,${library}"
mkdir -p build
go build -o build/go-link .
