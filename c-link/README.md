# C Link

A C application reached through Edge over Orbit Link, with the C binding. It opens no HTTP
listener. The application name is `c-link`, fleet `c`; the example Edge serves it as `c.test`.

## Prepare

C Link declares its Orbit C dependency in `vcpkg.json`. By default, the script
fetches the v0.1.2 overlay port from the Orbit C repository into `build/`, and
vcpkg installs the corresponding release package. Use a C compiler, CMake 3.20
or later and a bootstrapped vcpkg checkout. From the `examples/` directory:

```sh
VCPKG_ROOT=/path/to/vcpkg ./c-link/prepare.sh
```

Set `VCPKG_OVERLAY_PORTS` to use an existing overlay directory. To build with
unreleased changes in a local Orbit C checkout, use `--dev` with its path:

```sh
./c-link/prepare.sh --dev /path/to/orbit-c-checkout
```

Both commands update `c-link/build/c-link`. Each mode keeps its own CMake build directory.

## Run

From `examples/`, check Edge before starting anything:

```sh
scripts/run.sh --status edge-server
scripts/run.sh --start --release --workers 4 edge-server
curl --resolve c.test:8080:127.0.0.1 http://c.test:8080/
curl --resolve c.test:8080:127.0.0.1 --data-binary 'hello' http://c.test:8080/echo
scripts/run.sh --stop edge-server
```

Prepare the Edge and Axum executables first as described in the Edge README.
The Edge's TaskService starts C Link; use the existing Edge when it is running.
For a manually launched C Link instead, disable its task before starting it.

## Execution

`--execution multi` (default) and `--execution current` select the binding's
Tokio I/O driver from the same native library. `--threads N` sets the multi-thread
worker count; current-thread requires `--threads 1`. C callbacks stay on the
calling thread in either mode. No supervisor or public HTTP listener is created.

With the Edge's C Link task disabled, standalone launches can select either mode:

```sh
scripts/run.sh --execution current c-link
scripts/run.sh --execution multi --threads 4 c-link
```

Processes are independent. Launch another binary with `--fleet` and `--name` to
select its Orbit destination; matching fleet/name instances can serve together.
For separate launcher PID/log files, give each instance its own `RUN_LOG_DIR`.
SIGTERM drains only the process receiving it.

## Task-managed processes

The example Edge's `config/tasks.yaml` runs this executable with a Link task.
The task's command is one worker of fleet `c`; the task runs on the Edge
master and owns the count:

```yaml
tasks:
  c-link:
    link:
      fleet: c
      kind: worker
      workers: { policy: static, count: 4 }
      command:
        program: ./build/c-link
        cwd: ../c-link
        args: [--execution, current]
        log: console
```

Do not also launch C Link manually while that task is enabled. To grow the
fleet on Link pressure instead, declare
`workers: { policy: dynamic, min: 4, max: 16, step: 1, retire_idle: off }`:
the task then starts `min` and adds one process per pressure episode.
`dynamic` grows only over receivers that declare `max_in_flight`, as C Link
does; an unbounded receiver never reports pressure.

The Orbit C release uses 32 fleet lanes, a 1024-frame inbox, 512 stream
slots per creator and a 64 KiB window per direction. The application's
admission limits are separate running and waiting limits: `max_in_flight` 128 started
exchanges and `max_queued` 64 waiting in the native Link queue, a window of 192
that stays below the inbox capacity. Started work retains its place until
body/session completion and foreign-handle release.
TaskService owns process count and lifetime, not those transport resources.

## Routes

| Method | Path | Response |
| --- | --- | --- |
| GET | `/` | `C application on Claviron` |
| POST | `/echo` | Streaming request body |
| GET | `/bench/plain`, `/bench/json` | `ok`, `{"message":"ok"}` |
| WebSocket | `/ws` | Echoes each message |

SIGTERM withdraws admission and drains accepted exchanges. Without this process, Edge answers 503.
