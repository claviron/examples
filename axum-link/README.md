# Axum application

An Axum router hosted by Claviron, reached through Orbit as application
`axum-link` in fleet `axum`, and through named TCP and Unix listeners. It has
no Edge dependency. An external Edge can forward over Orbit or a socket.

`main.rs` uses `claviron_runner::Runner::with_router(router)` with the
runner's `axum` feature. The shared runner prepares
policy, optional route contributions, ingress, management and worker launch
inheritance. The application supplies no language owner or manual ingress setup.

`runner.yaml` selects Orbit, and also a TCP port and a Unix socket on the same
process. Its `site:` puts a redirect and static files in front of the router
on every door, with the schema an edge host uses. Shared runner options, including `requests`,
`responses` and `max_in_flight`, can be declared there. Additional service
configuration lives beside it; the application can register those services
with the runner's `with_services` or `with_bootstrap` APIs.
`defaults.listener` supplies socket defaults and Orbit's execution selection;
each named socket listener can override its settings.

## Routes

| Route | Response |
| --- | --- |
| `GET /` | Plain Axum application greeting. |
| `/ws` | A WebSocket echo on every door, one route for all of them. |
| `GET /bench/plain` | `ok` and a newline. |
| `GET /bench/json` | JSON `{"message":"ok"}`. |
| `POST /bench/echo` | Request body returned unchanged. |
| `/_bench/rust`, `/_bench/stats` | Shared native Claviron benchmark handlers. |

The workload router comes from `claviron::bench::routes_with_state`, without the runner's
benchmark registration service. Axum owns route selection and responses.
Claviron owns the process lifecycle, Orbit reception and graceful drain.
Unwritten application-side HTTP policy is unlimited; the external Edge owns
its own ingress policy. Orbit admission comes from the selected launch file.

## Run

Build from this application directory:

```sh
cargo build --release
```

From the repository root:

```sh
scripts/run.sh --release axum-link
scripts/run.sh --release --workers 4 axum-link
```

Standalone is the default. `--name` and `--fleet` override the application-owned
defaults; Edge must target the same pair. `--workers` selects Claviron supervision.
With workers, the master binds the Unix socket once and all workers share it.
The socket listeners appear as `Listener:application-tcp` and
`Listener:application-unix` in service management.

`managed.yaml` is an Orbit-only launch for the example Edge's TaskService.
TaskService starts one runner master; this runner's own
`orbit.link.workers` policy starts four workers and grows them to sixteen. Its 128 running and 192
total admitted exchanges (up to 64 queued) match C Link's budget. No TCP or Unix listener is opened by
this launch. Do not start a second Axum master for the same fleet alongside it.

The Unix listener accepts HTTP/2 prior knowledge:

```sh
curl --http2-prior-knowledge --unix-socket /tmp/axum-link.sock http://localhost/bench/plain
curl --http2-prior-knowledge --unix-socket /tmp/axum-link.sock http://localhost/_bench/stats
```

Use fresh connections to observe different responding workers. An existing
keep-alive connection stays with the worker that accepted it. Process identity
is reported only by `/_bench/stats`, alongside its activity counters: application,
runtime, fleet, PID and worker slot (`null` in standalone mode). Workload responses
remain unchanged.

An Edge host routes to it with:

```yaml
orbit:
  - { application: axum-link, fleet: axum }
```

With the example Edge running, use its listener's address and the `axum.test`
Host header:

```sh
curl -H 'Host: axum.test' http://127.0.0.1:8080/bench/json
curl -H 'Host: axum.test' --data 'Hello from Axum' http://127.0.0.1:8080/bench/echo
```
