#!/usr/bin/env bash
set -euo pipefail

if [[ $# -eq 1 && ( $1 == --help || $1 == -h ) ]]; then
    printf 'Usage: %s [--dev /path/to/orbit-c-checkout]\n' "$0"
    exit 0
fi

if [[ $# -eq 2 && $1 == --dev ]]; then
    source_dir=$(cd -- "$2" && pwd -P)
elif [[ $# -ne 0 ]]; then
    printf 'Usage: %s [--dev /path/to/orbit-c-checkout]\n' "$0" >&2
    exit 2
fi

cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
build_root="$PWD/build"
mkdir -p -- "$build_root"

if [[ $# -eq 2 ]]; then
    build_dir="$build_root/dev"
    native_dir="$build_dir/cargo"
    package_dir="$build_dir/package"
    (
        cd -- "$source_dir"
        if [[ $(uname -s) != Darwin && $(df -PT . | awk 'NR == 2 {print $2}') == 9p ]]; then
            if ! declare -F set_cargo_target >/dev/null && [[ -f "$HOME/.config/ai/set-cargo-target.sh" ]]; then
                source "$HOME/.config/ai/set-cargo-target.sh"
            fi
            if declare -F set_cargo_target >/dev/null; then
                set_cargo_target
                native_dir=${CARGO_TARGET_DIR:-$native_dir}
            fi
        fi
        cargo build --release --lib --target-dir "$native_dir"
        cmake -S . -B "$build_dir/binding" -DCMAKE_BUILD_TYPE=Release \
            -DORBIT_NATIVE_DIR="$native_dir/release"
        cmake --install "$build_dir/binding" --prefix "$package_dir"
    )
    cmake -S . -B "$build_dir" -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_PREFIX_PATH="$package_dir"
else
    : "${VCPKG_ROOT:?Set VCPKG_ROOT to a vcpkg checkout}"
    build_dir="$build_root/vcpkg"
    if [[ -z ${VCPKG_OVERLAY_PORTS:-} ]]; then
        orbit_c_port="$build_root/orbit-c-v0.1.2"
        if [[ ! -f "$orbit_c_port/packaging/vcpkg/orbit-c/portfile.cmake" ]]; then
            git -c advice.detachedHead=false clone --depth 1 --branch v0.1.2 \
                https://github.com/claviron/c.git "$orbit_c_port"
        fi
        VCPKG_OVERLAY_PORTS="$orbit_c_port/packaging/vcpkg"
    fi
    cmake -S . -B "$build_dir" -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_TOOLCHAIN_FILE="$VCPKG_ROOT/scripts/buildsystems/vcpkg.cmake" \
        -DVCPKG_OVERLAY_PORTS="$VCPKG_OVERLAY_PORTS"
fi

cmake --build "$build_dir" --config Release
install -m 755 "$build_dir/c-link" "$build_root/c-link"
