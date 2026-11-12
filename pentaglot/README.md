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
| `/python` | Python / `claviron-asgi` | `apps/python/app.py` |

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
`apps/php/src/Application.php`. Each of eight fixed instances creates one object,
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
existing public composition APIs. `claviron-asgi` owns Python's startup and
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

These archived Linux measurements used the earlier elastic PHP application
model (eight initial instances, ceiling 64 per serving worker), including the
former embedded Symfony application. They have not been rerun for the current
framework-free, fixed-instance application.

Linux x86-64; two Xeon E5-2650 v2 CPUs, 16 physical cores / 32 logical CPUs,
125 GiB RAM. Release/native with mimalloc release; Tokio default multi_thread.
Each language uses a fresh master and 16 workers, 128 connections and 5 minutes
of load, followed by 120 seconds idle with the server still running.

| Language | Successful req/s | CPU µs/request | p99 (ms) | RSS start → end → +120s (MiB) | Errors |
| --- | ---: | ---: | ---: | ---: | ---: |
| Rust | 280,113 | 69.39 | 8.151 | 2,942.3 → 2,994.8 → 2,994.8 | 0 |
| Lua | 108,810 | 198.69 | 5.072 | 2,936.6 → 3,014.9 → 3,014.9 | 0 |
| Deno | 95,398 | 236.77 | 8.110 | 2,464.0 → 3,326.5 → 3,307.9 | 0 |
| PHP / Symfony 8.2-dev | 32,547 | 919.39 | 12.005 | 3,439.3 → 3,548.5 → 3,482.3 | 0 |
| PHP / plain | 134,978 | 156.60 | 10.391 | 2,798.5 → 2,905.3 → 2,901.2 | 0 |
| Python / Gufi-ASGI | 123,932 | 194.95 | 6.694 | 2,936.7 → 3,015.9 → 3,018.0 | 0 |

CPU is aggregate master/worker CPU time per completed request. RSS sums those
17 processes with all five runtimes loaded; shared pages count per process.
The Deno row uses one isolate per worker. Rust, Lua and Python retain the
original binary; both PHP rows use the optimized embed bridge and eight Deno
isolates per worker. PHP starts with eight warm contexts in every case.
Plain PHP retains its earlier pool and warmup packages. Symfony uses 8.2-dev,
production cache warmup and the explicit OPcache settings above.
The [original measurements](public/assets/benchmark.json),
[earlier Plain PHP report](public/assets/benchmark-php-plain.json),
[updated Plain PHP report](public/assets/benchmark-php-plain-optimized.json),
[Symfony 8.2 report](public/assets/benchmark-symfony82.json),
and [updated Deno report](public/assets/benchmark-deno-multiplexed.json) include
settings, source revisions, CPU totals and idle RSS samples. See
[BENCHMARK.md](BENCHMARK.md) for the full history.

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
