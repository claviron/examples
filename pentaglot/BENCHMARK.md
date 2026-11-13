# Pentaglot benchmark

The seven JSON routes were measured from the published Claviron 0.2.3 examples
snapshot at commit `71a4b1eb7146c9c44cbbf790c2d824f45779ff91`. The
[README table](README.md#benchmark) summarizes the
[measurement data](public/assets/benchmark.json).

## Configuration

- AlmaLinux 8.10, Linux 5.15.225, x86-64, two Xeon E5-2650 v2 CPUs with 32
  logical CPUs and 125 GiB RAM. The client and server shared the machine, with
  private loopback and shared memory for the benchmark.
- Rust 1.100.0, release build with thin LTO and `-C target-cpu=native`.
  MiMalloc used `MIMALLOC_PURGE_DELAY=0` and `MIMALLOC_PURGE_DECOMMITS=1`.
- One fresh master and 16 workers per route, with 16 listening sockets and
  the default Tokio multi-thread runtime. Every worker loaded all five runtimes
  and both native modules, with one Deno isolate and 16 fixed PHP instances.
  PHP was 8.5.12 ZTS, Python was 3.15.2 and access logging was off.
- wrk 4.2.0 used eight threads and 128 persistent HTTP/1.1 connections for
  five minutes per route. Rust, Lua, Deno, PHP and Python ran in that order.
  C and C++ followed in a separate session with the same release binary
  and settings.

## Reproduce

From the combined examples checkout, prepare the PHP ZTS embed and CPython
build environment as described in the [Pentaglot README](README.md#run).
Build the C and C++ modules and the native release binary:

```sh
cmake -S c-module -B c-module/build
cmake --build c-module/build
RUSTFLAGS='-C target-cpu=native' scripts/run.sh --build --release pentaglot
```

In an isolated environment, launch a fresh server for each route:

```sh
MIMALLOC_PURGE_DELAY=0 MIMALLOC_PURGE_DECOMMITS=1 \
  RUST_LOG=warn,claviron_php_embed::warmup=info \
  scripts/run.sh --start --release --workers 16 --production pentaglot
```

Check all five runtime routes and the selected module route, if any. Confirm
16 listening sockets and 16 completed PHP OPcache warmups. For the selected
route, validate 65,536 untimed responses over 128 connections and confirm
that all 16 serving PIDs responded. Then run:

```sh
python3 pentaglot/scripts/benchmark.py \
  --url http://127.0.0.1:8080 --runtime rust \
  --workers 16 --connections 128 --threads 8 --duration 5m \
  --idle-sample-seconds 10 30 60 120 \
  --binary pentaglot/target/release/pentaglot --native \
  --label "CPU, RAM and OS" \
  --output pentaglot/logs/rust-report.json \
  --raw-dir pentaglot/logs/benchmark
```

Use `--runtime c` for `/c` or `--runtime cpp` for `/cpp`.
Stop the server after the last idle sample and verify shutdown before starting
the next route. The script measures the same master and workers throughout
the load and idle windows. CPU per request divides their combined user and
system CPU time by completed responses. RSS counts shared pages once per
process. The idle samples were taken without forcing GC or purging memory.
