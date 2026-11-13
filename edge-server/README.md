# Edge example

This Edge reaches the application examples over Orbit and their configured
socket upstreams. See `Cargo.toml` for the Claviron dependency.

## Task-managed applications

`config/tasks.yaml` starts the prepared C and Axum executables when this Edge
boots. Build them first; the task service does not build applications.

| Task | The command is | Count owned by | Count |
| --- | --- | --- | --- |
| C Link (`kind: worker`) | One worker of fleet `c` | The task, on the Edge master | `static`, 4 |
| Axum Link (`kind: runner`) | A runner that owns its workers | The runner's master | `dynamic`, 4–16 |

Both applications serve a Link fleet, so each is run by a Link task: the
fleet's process count then has one owner. C keeps a fixed count with
`static`. Axum's runner declares `dynamic` in its own `managed.yaml`: it
starts at `min` and adds one worker per pressure episode.

The application owns its Link geometry and admission; the task selects
neither, and the Edge adopts the shape the application declares. Each
application runs at most 128 exchanges per serving process and admits 192 in
all, including up to 64 waiting. One Edge worker holds at most 128 open
exchanges toward a fleet (`lane_capacity`), so the fleet's capacity is reached
by several Edge workers together.

The Edge route selects the receiver with `orbit[].balance: first |
round_robin`, separately from process counts. `c.test` and `axum.test` give a
waiting exchange one `requests.queue_timeout` budget of 100 ms. A full
receiver window refuses at once; a queued exchange is withdrawn at that
deadline.

Prepare C Link with its matching native library as described in its README.
Build Axum Link and this Edge in release mode. Then, from the examples directory:

```sh
scripts/run.sh --status edge-server
scripts/run.sh --release --workers 4 edge-server
```

Do not launch C Link or Axum Link separately while their tasks are enabled.
The foreground Edge owns their shutdown. Requests use `c.test` or `axum.test`:

```sh
curl -H 'Host: c.test' http://127.0.0.1:8080/bench/plain
curl -H 'Host: axum.test' http://127.0.0.1:8080/bench/plain
```
