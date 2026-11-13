# Examples

Application examples for Claviron `0.2.3`. Rust dependencies come
from crates.io, JavaScript from npm, Java from Maven Central, and PHP from
Packagist. The C and Go bindings provide native libraries in GitHub Releases.

See [benchmark results](BENCHMARKS.md) for measured Edge and Orbit application
paths, hardware, configuration and comparison limits.

| Example | What it demonstrates | Connection |
| --- | --- | --- |
| [Edge](edge-server/) | HTTP ingress, host routing and static assets | Public HTTP listener |
| [Demo certificates](edge-server/src/demo_certs.rs) | Component that makes the certificates a `tls.yaml` names at boot | Component inside Edge |
| [C module](c-module/README.md) | C and C++ web applications, as compiled libraries | Loaded into Pentaglot's Edge process |
| [Pentaglot](pentaglot/) | Rust, Lua, Deno, PHP and Gufi-ASGI applications | Embedded together in one Edge, on separate URL prefixes |
| [FastAPI](fastapi-link/) | Python/ASGI through Gufi, with WebSockets | Separate process connected to Edge through Orbit Link |
| [Octane](octane-link/README.md) | Laravel through Lumenize, with SQLite and native broadcasting | Separate process connected to Edge through Orbit Link |
| [Symfony](symfony-link/README.md) | Persistent Symfony through Skene | Separate process connected to Edge through Orbit Link |
| [JavaScript](js-link/README.md) | React/Vite and Hono on Deno, Node or Bun | Separate process connected to Edge through Orbit Link |
| [Axum](axum-link/README.md) | Native Rust router hosted directly by Claviron | Separate process connected to Edge through Orbit Link |
| [Go](go-link/README.md) | Standard `net/http` handlers through the native Go binding | Separate process connected to Edge through Orbit Link |
| [Java](java-link/README.md) | Java HTTP handlers through the Orbit Java binding | Separate process connected to Edge through Orbit Link |
| [C](c-link/README.md) | C handlers through the native C binding | Separate process connected to Edge through Orbit Link |
| [libuv](uv-link/README.md) | C handlers dispatched by an application-owned libuv loop | Separate process connected to Edge through Orbit Link |

## Run

Use `scripts/run.sh` from the examples root in either checkout layout. `--build`
builds the selected example. Add `--run` or `--start` to launch it after building:

```sh
scripts/run.sh --build --release axum-link
scripts/run.sh --build --start --release edge-server
scripts/run.sh --release --workers 4 pentaglot
scripts/run.sh --start --release fastapi-link
scripts/run.sh --release symfony-link
scripts/run.sh --status fastapi-link
scripts/run.sh --stop fastapi-link
```

`APP_NAME` is the final argument. `--release` selects `target/release` instead
of `target/debug`. Without `--build` or `--prepare`, the launcher uses existing
artifacts. Other
options, including `--fleet` and `--workers`, pass unchanged to the application.
`--config` is a runner's launch file, whose directory is its configuration directory;
an Edge reads a directory of files and takes `--config-dir`. Its options and configuration own the defaults. The working
directory is the application's directory.

`CARGO_TARGET_DIR` selects an existing Cargo output directory. Launcher PID and
console logs live in the application's `logs/`; `RUN_LOG_DIR` can select another
directory for an isolated instance.

For C, libuv, Go and Java, set the dependency paths in the example's README,
then run `scripts/run.sh --build <name>` or `scripts/run.sh --prepare <name>`.
C, libuv and Go launch from
`build/<name>`. Java uses `build/java-link.jar` and `ORBIT_JAVA_CLASSPATH`.

```sh
scripts/run.sh --prepare c-link
scripts/run.sh --prepare java-link
scripts/run.sh java-link
scripts/run.sh --prepare js-link
scripts/run.sh --start --runtime=node js-link
scripts/run.sh --stop --runtime=node js-link
```

JS preparation installs the locked npm dependencies and builds the frontend.
JS runs with Deno by default. Select `--runtime=node` or `--runtime=bun` to
switch. Use the same runtime for start, status and stop. `ORBIT_WORKERS`
sets the process count, which defaults to eight.

Open [examples.code-workspace](examples.code-workspace) or an individual
application folder for development.

## Open it in a browser

`edge-server` makes a demo CA and the certificates named in its
`config/tls.yaml` the first time it boots; they stay in its `.ssl`, which is
never committed. To open the hosts, trust that CA and map the names:

```sh
scripts/run.sh --start edge-server   # makes the certificates
scripts/trust-ca.sh                  # asks for your password
scripts/trust-ca.sh --dry-run        # or see what it would do
scripts/trust-ca.sh --remove         # undo
```

`/etc/hosts` has no wildcards, so `*.test` cannot be mapped at once. The script
writes each host `tls.yaml` names, with a marker it can remove later. Add a host
there and run it again. Then open `https://fastapi.test:8443`, with the application
running. Firefox keeps its own trust store unless
`security.enterprise_roots.enabled` is on.

## Publish a combined snapshot

From the source checkouts, run `scripts/publish-examples.py <version>` with
Python 3.12 or newer. It exports the locally available `origin/main` commits
with `git archive`, checks dependencies, and prints the temporary directory
containing the archive and the exact source revisions. Use
`--ref <example>=<commit>` to select a different registry-ready revision.

Review the dry run, then rerun with the same revisions and `--push` to append
one commit to `claviron/examples` on GitHub. Git uses its configured identity
and credentials.
