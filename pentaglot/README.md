# Pentaglot

Five applications embedded in one Claviron Edge. The serving process hosts
Rust handlers, a persistent Lua application, a Deno Core/V8 application, a
persistent framework-free PHP application and one Python ASGI application.

| URL prefix | Application | Source |
| --- | --- | --- |
| `/rust` | Native Rust | `src/app/native.rs` |
| `/lua` | Lua / Olua | `apps/lua/app.lua` |
| `/deno` | JavaScript/TypeScript / Deno Core | `apps/deno/app.ts` |
| `/php` | Native PHP application | `apps/php/app.php` |
| `/python` | Python / `claviron-python` | `apps/python/app.py` |

Every application answers JSON at its prefix and accepts `POST <prefix>/echo`.
`<prefix>/bench/plain` and `<prefix>/bench/json` answer `ok` and `{"message":"ok"}`.
The original URL remains visible to the application. The home page calls all
five. `X-Pentaglot-Pid` identifies the Edge process serving each response;
standalone mode uses one PID for every language.

Rust routes use the Axum adapter. Lua, Deno, PHP and Python enter the host
pipeline through the `routes` declarations. `config/python.yaml` names the one
Python application, and `/python/ws` is its WebSocket.

Two compiled libraries run beside them, with no engine between: `/c` and `/cpp`, from
[`c-module`](../c-module/README.md). `config/modules.yaml` names them, so build them as its README says before
running this.

## Run

This is an independent executable workspace. From the repository root:

```sh
scripts/run.sh pentaglot
```

Open `http://127.0.0.1:8080`, as declared in the example's `.env` file. Claviron
loads that file and its selected environment variant; the launcher does not
inject a listener address. The shared launcher selects an existing debug or `--release` binary and accepts
`--start`, `--status` and `--stop`. It does not build or prepare dependencies.
Application flags such as `--workers N` and `--production` pass through unchanged;
the application's options define the standalone default and fleet.
`PENTAGLOT_LISTEN` is resolved from the environment-file recipe; the example's
`.env` overrides an inherited value. For an isolated listener, copy the config
and set `listeners.demo.listen` explicitly. `--fleet`, `--config-dir` and `--workers` select this instance's runtime resources.
Pass these options before `pentaglot` in the shared launcher command.

Pentaglot uses MiMalloc for Rust allocations. Allocator environment settings are
inherited at process startup; the launcher does not select an allocator policy.

Building requires a compatible PHP ZTS embed installation (`PHP` and
`PHP_CONFIG`), CPython 3.15 (`PYO3_PYTHON`) and the Deno Core/V8 build
prerequisites. The Python example uses only the standard library. It creates
no virtual environment and installs no Python packages; `config/python.yaml`
can select a venv for an application that needs one.

The PHP application requires no Composer packages or framework preparation.
`apps/php/app.php` loads `Pentaglot\Php\Application` from
`apps/php/src/Application.php`. Each fixed instance creates one object. The
default environment sets `PHP_INSTANCES=16` per serving process. Each instance
receives requests with `claviron_application_receive()` and returns buffered JSON
with `claviron_application_respond()`. Its counter persists between requests.
The class handles `GET /php` and `POST /php/echo`, including UTF-8 bodies, and
returns 404/405 for unmatched paths or methods. Claviron owns ingress and drain.

PHP scans `config/php.d`: OPcache is enabled with 256 MiB, 20,000 accelerated
files, 16 MiB for interned strings and `opcache.validate_timestamps=0`.
Xdebug is not loaded from that directory. The selected PHP build must provide
OPcache as a built-in module or load it from its primary INI. Background
`engine.warmup` compiles `apps/php`; restart after PHP source changes.

The Symfony application is separate in [symfony-link](../symfony-link/README.md).
It runs through Skene and connects to the standalone Edge over Orbit.
Deno enables its standard Fetch and Streams implementation for request and
response bodies.

All configuration paths follow the process working directory, which the
launcher sets to this example. Rust, Lua, Deno, PHP and Python use their
existing public composition APIs. `claviron-python` owns Python's startup and
final shutdown, using a blocking executor for native boot/join.

## Check

Validate configuration without starting the listener:

```sh
scripts/run.sh --check-config pentaglot
```

After launching an isolated instance, run the small acceptance check:

```sh
python3 pentaglot/scripts/smoke.py --url http://127.0.0.1:8080
```

It checks GET/POST dispatch in every language, UTF-8 body round trips, segment
boundaries and the Python streamed response. This is an executable demo check,
not a library test or a performance benchmark. Stop the server with Ctrl-C or
the launcher's `stop` command after clients have finished.

## Benchmark

Measured from the published Claviron 0.2.3 examples snapshot on AlmaLinux 8.10
x86-64, with two Xeon E5-2650 v2 CPUs (32 logical CPUs) and 125 GiB RAM. Each
path used a fresh master and 16 workers, 128 persistent HTTP/1.1 connections,
eight wrk threads and five minutes of load. Each worker loaded all five
runtimes and both native modules, with one Deno isolate and 16 fixed PHP
instances.

| Application | Successful req/s | CPU µs/request | p99 (ms) | RSS start → end → +120s (MiB) | Errors |
| --- | ---: | ---: | ---: | ---: | ---: |
| Rust | 396,655 | 41.05 | 6.508 | 2,029.4 → 2,071.0 → 2,071.1 | 0 |
| Lua | 130,192 | 147.61 | 3.685 | 2,032.6 → 2,078.6 → 2,078.3 | 0 |
| Deno | 183,446 | 112.02 | 22.039 | 2,420.1 → 3,821.1 → 3,808.8 | 0 |
| Plain PHP | 227,377 | 89.32 | 5.987 | 2,023.9 → 2,109.9 → 2,109.7 | 0 |
| Python / ASGI | 155,864 | 147.66 | 4.969 | 2,021.8 → 2,078.6 → 2,078.4 | 0 |
| C module | 306,174 | 60.94 | 2.946 | 2,004.8 → 2,058.3 → 2,058.2 | 0 |
| C++ module | 303,422 | 61.94 | 2.868 | 2,019.8 → 2,073.8 → 2,074.2 | 0 |

CPU is aggregate master and worker CPU time per completed response. RSS sums
the same 17 processes and counts shared pages per process. The +120s sample
was taken with the server still running and idle. See [BENCHMARK.md](BENCHMARK.md)
for the procedure and [measurement data](public/assets/benchmark.json) for
per-process samples.

## Ownership

Each serving worker embeds all five applications. Worker processes can scale
the composition; they are not language-specific processes. PHP instances have
their own Zend contexts, Deno owns its isolate threads, Lua owns its managed
state, and ASGI owns an event-loop thread. They share the host process and are
not crash/security isolation boundaries.

CPython and its environment are process-wide. The host drains ingress before
ASGI lifespan shutdown and joining its loop. No forced drain deadline,
filesystem watcher or automatic interpreter reload is added. Restart the demo
to apply Python source or environment changes. Lifecycle controls for each
runtime remain owned by the corresponding adapter; this example adds no
management server.
