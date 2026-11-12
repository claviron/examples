# libuv Link

A C application over Orbit Link, with callbacks dispatched by its own libuv loop.
It opens no HTTP listener and starts no supervisor. It uses the same native
library and handlers API as C Link. Tokio still drives Orbit I/O.

## Prepare

Build the C binding first. With a C compiler, CMake, pkg-config and libuv installed:

```sh
ORBIT_C_DIR=/path/to/orbit-c \
ORBIT_C_LIBRARY_DIR=/path/to/native-library-directory \
uv-link/prepare.sh
```

`ORBIT_C_DIR` must contain the matching ABI 4 header. Preparation only builds the
binary; it does not start applications or install dependencies. `prepare.sh`
refreshes `include/orbit.h` from that binding; CMake uses the local header.

## Run

From the examples directory:

```sh
scripts/run.sh --execution current uv-link
scripts/run.sh --execution multi --threads 4 uv-link
```

Defaults are fleet `uv`, application `uv-link`, one current-thread native I/O
driver and a dispatch budget of 64 completions per loop callback. `--dispatch N`
changes that budget; it is not a timer. `--fleet` and `--name` select the Orbit
destination. Each process is independent and can be managed externally.

The example Edge routes `uv.test` to application `uv-link` on fleet `uv`.
When Edge is already running:

```sh
curl --resolve uv.test:8080:127.0.0.1 http://uv.test:8080/bench/plain
```

The application handles `/`, `/bench/plain`, `/bench/json`, streaming POST
`/echo` and `/bench/echo`, and message echo on `/ws` and `/wt`. WebTransport
uses the reliable-message Link profile; Edge owns the public handshake.

## Loop ownership

`orbit_loop.wake` calls `uv_async_send` from native producers. The libuv callback
calls `orbit_dispatcher_poll`; all application handlers run on that loop thread.
Notifications may coalesce. The dispatcher schedules continuation when its
budget ends, so no timer or idle polling is needed.

SIGINT/SIGTERM withdraw admission and keep the loop running until accepted work
drains. Terminal dispatch detaches native wake calls before the libuv handles
close. The application then releases its dispatcher, application and runtime.
