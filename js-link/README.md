# JavaScript Link application

A React/Vite frontend with a separate Hono application, reached through Claviron
over [Orbit Link](https://crates.io/crates/orbitive). Deno, Node and Bun use the
same native [`@claviron/orbit`](https://www.npmjs.com/package/@claviron/orbit)
addon, installed from npm with a prebuilt binary for the target OS and CPU.
Nothing is compiled during installation. The addon and the Edge must speak
the same Link: `@claviron/orbit` 0.1.7 is built for Claviron 0.2.2.

## Prepare and run

From this directory:

```sh
npm install
npm run build
# The checked-in .env contains only public demo settings.
npm run serve       # Deno
# npm run serve:node
# npm run serve:bun
```

From the examples root, `scripts/run.sh --prepare js-link` installs the locked
dependencies and builds the frontend. `scripts/run.sh --start --runtime=node js-link`
starts the application's processes (`ORBIT_WORKERS`, 8 unless set). Use
`--runtime=deno` or `--runtime=bun` to select another runtime. Use the same
runtime when checking status or stopping.
The default identity is application `js-link` in fleet `orj`. `ORBIT_APP` and
`ORBIT_FLEET` can override it; update the edge's route to match if changed.

## Through the example edge

Start `scripts/run.sh --start edge-server` from the examples root. Its `orbit.test` vhost
serves `/assets` from this application's generated `public/` directory and
routes other requests to the named application over SHM, not TCP or UDS.
Resolve `orbit.test` to loopback for browser use. The commands below assume the
edge listener is configured on loopback port 8080.

```sh
curl --resolve orbit.test:8080:127.0.0.1 http://orbit.test:8080/api/status
curl --resolve orbit.test:8080:127.0.0.1 http://orbit.test:8080/api/echo \
  --data 'Hello over Orbit'
```

The page shows the runtime, process and lane, echoes a request body, and can
publish `example.message.v1`. Open two tabs to observe events through
`/api/events/stream` (Server-Sent Events). The stream is event-driven, not polled;
the in-memory recent-event list is bounded and not durable history. Multiple
application processes can join the same named fleet.

It also subscribes to `demo.shared.v1`, the Octane example's `SharedEvent` topic.
After preparing that example's database and seeding its demo user, its
`/events/shared` route can publish the PHP event for subscribed linked consumers.

`/api/db` reads the prepared [SQLite sample](database/demo.sqlite) included in
this project. It reports user/todo/record counts without opening a remote DB
connection. Node 24 and Deno use built-in `node:sqlite`; Bun uses
`bun:sqlite`, adapted to Kysely's SQLite interface. No new native addon needs
compilation for database access. Node currently marks its SQLite API experimental.
The database is opened read-only and must already exist; startup does not create
or migrate it. `EXAMPLES_DATABASE` can explicitly select another local file.

No Revolution installation or business event topics are required. The .env
contains public demonstration values only; do not add credentials. These
unauthenticated diagnostics and event publishing routes are for a local
demonstration, not a public deployment.

## Optional receiver admission

With a binding/native build supporting bounded admission, set both
`ORBIT_MAX_IN_FLIGHT` (exchanges that run at once, positive) and
`ORBIT_MAX_QUEUED` (exchanges that may wait, running ones not included; `0`
is no queue). Omit both to preserve unbounded admission. `ORBIT_MAX_WAITING`
is refused: it was replaced by `ORBIT_MAX_QUEUED`. Native execution can be
selected before launch with `ORBIT_JS_EXECUTION=current`; this selects the
native I/O runtime, independently of the JavaScript engine. These controls
require the updated binding API and are not available in older prebuilds.
Worker process counts remain the external controller's responsibility.
