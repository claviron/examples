# Laravel application over Orbit

A Laravel application with its own Rust binary, hosted by
[Lumenize](https://crates.io/crates/lumenize) with its `runner` feature in persistent PHP Application mode.
A separate Claviron edge sends HTTP and Pusher WebSocket traffic to application
`octane-link` in fleet `octane` over [Orbit Link](https://crates.io/crates/orbitive).
The edge does not embed PHP; this application does not open a TCP or UDS listener.

## Prepare

Cargo takes Lumenize from crates.io with `runner` and `cli`;
imports use `lumenize::{Options, Runner}`. Composer consumes `claviron/laravel`
`0.1.5` and `claviron/sapi` `0.1.3` from Packagist. These releases must exist before
installation; no sibling integration checkout is selected automatically.
PHP imports use `Lumenize\\` for
Laravel integration; native engine classes retain `Claviron\\`.
The optional benchmark development package is `claviron/bench` `0.1.0` from
Packagist; it does not use a sibling checkout. That release must be available
before updating the Composer lockfile.

From this directory:

```sh
# The checked-in .env and database/demo.sqlite contain public demo values.
"$PHP" /path/to/composer.phar install
npm install
npm run build
```

Use the workspace ZTS embed PHP build, not Homebrew PHP. Set `PHP` and
`PHP_CONFIG` to that installation. The application reads only its own
`php.ini`, which `lumenize.yaml` names without a scan directory; it loads the
`igbinary` and `redis` extensions by name from that installation's
`extension_dir`, so build both for it. The example .env and app key are intentionally public;
never reuse them for a deployment or add real credentials to this file.
Dependencies and generated assets are not copied from the original application.

Todo/database routes use this project's prepared [SQLite demo](database/README.md).
The database belongs to this application; Edge only forwards requests. Inside
Lumenize, Laravel's SQLite connection uses the native `octane` pool declared in
`lumenize.yaml`. Artisan commands use PDO SQLite. The seeded
user is `demo@example.test` / `password`. For later schema changes, from here:

```sh
"$PHP" artisan migrate --database=sqlite --force
"$PHP" artisan db:seed --database=sqlite --force
```

This setup is explicit: the launch script does not install dependencies, create
databases or run migrations.

## Run behind the edge

From this repository's root:

```sh
scripts/run.sh octane-link
# In another terminal, unless the example edge is already running:
scripts/run.sh edge-server
```

The launcher selects an existing `--release` binary; application options such
as `--workers` are forwarded unchanged. Ctrl-C follows the runner's normal shutdown path.
`..` supplies the `octane.test` route and
permits HTTP/1 WebSocket upgrades. It serves `/build` assets directly and sends
application requests, including Pusher sockets, over Orbit.

Resolve `octane.test` to loopback for browser use. The sample `.env` assumes an
edge listener on port 8080; keep `APP_URL` aligned with your actual listener.
The browser opens the socket on the hostname, port and HTTP/HTTPS scheme its
page was served from, so the same page works through the edge and on the
runner's own listener, with no separate broadcast address or TLS certificate.

```sh
curl --resolve octane.test:8080:127.0.0.1 http://octane.test:8080/up
curl --resolve octane.test:8080:127.0.0.1 http://octane.test:8080/_lumenize/rust
```

The front page demonstrates native broadcasts, Laravel notifications and client
whispers. Open two tabs to see delivery. `/session` shows
Laravel sessions stored in the dedicated Claviron `sessions` cache store.
The default Laravel cache uses the separate Claviron `fleet` store. Undeclared Rust paths continue to Laravel
through `application.php`.

## Its own doors

Without the edge, the application answers on three listeners of its own, all
declared in `lumenize.yaml`: plain HTTP/1 on `127.0.0.1:8102`, HTTPS on
`127.0.0.1:8103`, and HTTPS that requires a client certificate on
`127.0.0.1:8104`. Both HTTPS listeners offer HTTP/2 and HTTP/1.1 through ALPN,
h2 first. They serve the `octane.test` certificate from the store the
launch's `tls` section names, `.ssl` in this directory, which is never
committed. At boot the `demo-certs` component (`../demo-certs`) makes what is
missing there before TLS reads it, as the edge example does for its `tls.yaml`: a demo CA, unless one is
already in `.ssl/ca/demo`, then the `octane.test` certificate issued from it.
A CA it creates is logged once, with its path; a browser has to trust it.
To use an authority you already trust, put its `ca.crt` and `ca.key` in
`.ssl/ca/demo` before the first start. A certificate there without its key
stops the boot rather than being replaced.

```sh
curl --http2 --cacert .ssl/ca/demo/ca.crt \
  --resolve octane.test:8103:127.0.0.1 https://octane.test:8103/up
```

### A client certificate

`application-mtls` on `127.0.0.1:8104` is the same HTTPS, with
`tls: {client_auth: {authority: {local: demo}}}` on the listener in
`lumenize.yaml`: a client must present a certificate the store's `demo`
authority signed, and the other two listeners ask for none. `src/main.rs` asks `demo-certs` for one such client,
`client.octane.test`, made at boot in `.ssl/clients/client.octane.test` when it
is missing and kept when it is there. Two routes show what the handshake
verified: `/_lumenize/rust/client` from the connection in Rust, and
`/_lumenize/php/client` from PHP's `SSL_CLIENT_VERIFY`, `SSL_CLIENT_S_DN` and
`SSL_CLIENT_I_DN`.

```sh
curl --http2 --cacert .ssl/ca/demo/ca.crt \
  --cert .ssl/clients/client.octane.test/fullchain.pem \
  --key .ssl/clients/client.octane.test/privkey.pem \
  --resolve octane.test:8104:127.0.0.1 https://octane.test:8104/_lumenize/php/client
# verify=SUCCESS
# subject=CN=client.octane.test
# issuer=CN=Claviron Examples Demo CA, O=Octane Link Demo
```

Without `--cert` the handshake ends with the TLS alert `certificate required`,
and a certificate from another authority with `unknown ca`. On `8103` the same
routes answer `verify=NONE`; on `8102`, which has no TLS, nothing at all.

The front page opens its Pusher socket on the port it was served from, so on
this listener it is `wss://octane.test:8103`: an HTTP/1.1 upgrade, or on an
HTTP/2 connection an extended CONNECT (RFC 8441), which the listener
advertises.

## One counter, two languages

`/hits`, linked from the front page, shows Rust and PHP adding to one shared
value without losing each other's updates. Rust declares it in
`src/app/services/orbital.rs` with `orbital!` as the kept `i64` named `hits`,
and registers it with `Declare<Hits>`: every process opens it once at boot.
Because a counter key is an Orbital name, it is also the Orbit counter `hits`.

| Path | Side | Does |
| --- | --- | --- |
| `GET /_lumenize/rust` | Rust | `fetch_add(1)`, returns the value after it |
| `GET /_lumenize/rust/value` | Rust | returns the value |
| `GET /_lumenize/php` | PHP | `orbit_counter_inc('hits', 1)`, returns the value after it |
| `GET /_lumenize/php/value` | PHP | `orbit_counter_get('hits')` |
| `DELETE /_lumenize/php` | PHP | `orbit_counter_reset('hits')`; needs the page's CSRF token |

Every one answers `hits=<value>`. A run on the page sends the same number of
requests to each side at once and compares the value after with the value
before plus both. The same from a shell, against the application's own
listener on `127.0.0.1:8102`:

```sh
curl -s http://127.0.0.1:8102/_lumenize/rust/value
seq 500 | xargs -P 8 -I{} curl -s -o /dev/null http://127.0.0.1:8102/_lumenize/rust &
seq 500 | xargs -P 8 -I{} curl -s -o /dev/null http://127.0.0.1:8102/_lumenize/php &
wait
curl -s http://127.0.0.1:8102/_lumenize/php/value   # the first value plus 1000
```

The value is kept: it survives the master's reset of the Orbitals table at a
quiet boot, and lives as long as the fleet's shared memory.

## One Orbit atomic, two languages

`/tiles`, also linked from the front page, shows a Rust handler and a Laravel
controller working on one value in shared memory, and proves that neither
loses the other's change.

The value is a counter. Each request, to Rust or to PHP, adds one to it and
takes the number it had before as the index of a tile; the page paints that
tile, warm colours for Rust and cool ones for PHP. The add is one atomic
operation on a word both languages reach directly, so no two requests can
receive the same number and no number can be skipped, whichever language asks
and however the requests interleave.

**What the page proves.** When the run ends, the evidence is two counts:

- *Duplicates*: tiles claimed more than once. If one side missed a change the
  other made, it would hand out a number already given.
- *Still empty*: tiles nobody claimed. A lost add would leave a gap.

Both at zero means the numbers handed out were exactly `0..N-1`, once each.
The evidence is then outlined in green and checked; otherwise it is outlined
in red and says which count is off. Three more counts appear only when they
are not zero: answers outside the canvas, claims that failed, and answers
from another round.

PHP can only take tile 41 after Rust took tile 40 by reading the value Rust
had just written, so every tile next to one the other language claimed is one
side continuing from the other's change.

**How the requests are divided** (under Options):

| Mode | Who gets the next request | The split shows |
| --- | --- | --- |
| Both, free (default) | The side whose request just finished | How the two request paths compare: the quicker side asks again sooner and claims more |
| Both, at random | A coin toss | Nothing; about half each |
| Both, in turn | The other side | Nothing; exactly half each |
| Rust only, PHP only | That side | — |

The evidence must hold in every mode: the split changes, the consistency does
not. The split in the free mode measures a whole request through each path (a
Rust handler, and Laravel's routing and controller), not the shared value,
whose add costs nanoseconds either way.

Each language has one flat colour. "Who served" lists the processes, Tokio
threads and PHP instances that took part, and how many tiles each claimed.

The value is the kept `i64` `tiles`, declared beside `hits` in
`src/app/services.rs`. Its high 32 bits are the round and its low 32 the next
tile, so a claim is one add of 1 and the value before it names both the tile
and the round it was taken in. Starting a round sets the next round with no
tile taken, in one compare-exchange; every answer from another round is
counted and not painted. A separate round value would not do: a claim could
read one round and add to the next.

| Path | Side | Does |
| --- | --- | --- |
| `GET /_lumenize/rust/tiles` | Rust | `fetch_add(1)` on the declared Orbital |
| `GET /_lumenize/php/tiles` | PHP | `orbit_counter_inc('tiles', 1)` |
| `DELETE /_lumenize/rust/tiles` | Rust | starts the next round; answers `{"round":N}` |
| `GET /_lumenize/rust/tiles/id` | Rust | the Orbital's id as text, such as `i64:1:16`, for PHP to open |
| `GET /_lumenize/rust/burst?n=N` | Rust | N claims in one loop, `fetch_add(1)` each |
| `GET /_lumenize/php/burst?n=N&id=ID` | PHP | N claims in one loop on `Orbital::open(ID)`, `add(1)` each |

A claim answers `{"side","round","index","pid","worker","execution"}`. The
process and worker number are read once in each process, when the Rust router
is built and on a PHP instance's first claim; `execution` is the Tokio thread
that ran the Rust handler or the PHP instance, read once per thread. The page
retries a `503`, the PHP application's admission refusing a request before it
runs, and counts it apart. From a shell:

```sh
curl -s -X DELETE http://127.0.0.1:8102/_lumenize/rust/tiles     # {"round":N}
seq 0 999 | awk '{ print ($1 % 2 ? "php" : "rust") }' \
  | xargs -P 8 -I{} curl -s http://127.0.0.1:8102/_lumenize/{}/tiles \
  | grep -o '"index":[0-9]*' | cut -d: -f2 | sort -n | uniq | wc -l   # 1000
```

The colours show who served each claim, not how requests are spread. Rust and
PHP run in the same process. With `--workers`, each worker binds the TCP
listeners with `SO_REUSEPORT`; Linux spreads connections across them, while
macOS gives every connection to the socket bound last, so there the page shows
one process.

### An orchestra with no conductor

The page sends one request per claim, so what it measures is the request path
(about 13,000 claims a second through Laravel), not the atomic. The orchestra
takes the request out of the claim: a terminal client asks Rust and PHP
workers for *bursts*, each burst is many claims in a tight loop on the server,
and millions of claims land on the same value from both languages with
nothing coordinating them.

The PHP side is this Laravel application as it stands: a route in
`routes/web.php`, `TilesController` in `app/Http/Controllers`, the framework
booted once and kept in each PHP instance. Nothing here is special to the
demo. It is one implementation on top of an Orbit atomic; the atomic is the
contract, and any process on the machine that declares it can join.

```sh
cargo run --release --example orchestra                      # 50,000,000 claims, 8 + 8 lanes
cargo run --release --example orchestra -- --claims 1000000 --plain
cargo run --release --example orchestra -- --help
```

`--url` names the application (default `http://127.0.0.1:8102`), `--claims`
the target (at most 4,294,967,295: a round's index is 32 bits and one more
claim reads the value), `--burst` the claims per burst (default 10,000, at
most 1,000,000), `--rust` and `--php` the lanes of each kind (default 8 and
8). Keep the PHP lanes at or below the application's PHP instances
(`php.application.instances`, 8): more only wait in the admission queue.
`--plain` prints progress lines and the verdict instead of the screen, and
exits 0 only when the run is verified. The client is a Cargo example with
dev-dependencies only; the server binary does not carry it.

The run starts a round, fetches the Orbital's id, and starts its lanes. A
lane is one worker of its kind with one burst in flight; when its burst is
answered it takes the next, until the bursts handed out add up to the target.
No lane waits for another and nothing orders them. A `503` from PHP's
admission took nothing and is sent again, counted as refused; so is a
connection the listener refused before anything was sent.

**Both sides use `orbit-atomic`.** Rust declares the value and claims with the
injected handle's `fetch_add(1)`. PHP has no declaration by name yet, so Rust
hands it the id and the controller opens it with `Claviron\Runtime\Orbital::open`,
once per PHP instance: the handle is kept in the instance's statics, and only a
request naming another id opens again. A claim there is `$orbital->add(1) - 1`,
since `add` answers the value after the add. PHP integers are 64-bit, so the
round and the index unpack with shifts and masks, and a burst's index sum
stays below 1,000,000 × 2^32, under 2^52.

**A burst checks itself.** Every claim must be in the first claim's round, and
each index one worker takes must be greater than the one before: an atomic add
never gives one caller the same or a smaller index twice. Within a round that
is "greater value", so both tests fold into one condition that a correct run
never meets, beside the add. A burst answers
`{"side","round","count","first","last","sum","pid","worker","execution","fault"}`,
`sum` as a decimal string and `fault` null, or the check that failed with the
claim before and the claim that broke it. The client also checks each answer:
its side, the run's round, the count asked, and room between the first and last
index for strictly increasing indices.

**The screen.** The title, the run's numbers (target, claims so far, claims a
second overall and per side, elapsed, refusals), one row per lane with its
bursts, its claims and a mark that turns while it works, then the fireflies:
every verified burst lights its share of the field in its side's flat colour,
in a scattered order, so the field twinkles in and is fully lit when the run
ends. With more bursts than lights, the field's title says how many bursts one
light stands for, and a light takes the colour of its group's first burst, a
sample that keeps the two colours in proportion. A burst that fails any check
lights its place red with a cross, no more bursts are handed out, and the
verdict says what was wrong. The screen is drawn 30 times a second, a display
setting of the client; the lanes keep counters the drawing reads and never wait
for it. `q` or Ctrl-C leaves; after the run, any key does.

**What the evidence means.** When every lane has ended, one more claim reads the
value. Three totals, which cost the same at any size:

- *Answered*: the claims in verified bursts, which must be the target.
- *Shared value*: it must be in the run's round, at index N, the claims
  answered. An add writes the value it read plus one, so reaching N from 0 in N
  adds needs each of 1..N written once: every add read a different value, and
  the N indices handed out were 0..N-1, once each. A lost add leaves it short;
  a claim from outside the run leaves it over.
- *Index sum*: the indices the answers report must add up to N(N-1)/2, the sum
  of 0..N-1. This holds the answers to the value: an answer that reports an
  index it did not take moves the sum.

The per-burst checks catch a wrong round or order where it happens. Neither
the sum nor the per-burst checks proves exactly-once on its own (two bursts
both reporting 0 and 3 sum like 0, 1, 2, 3), which is why the shared value is
there: the value carries the proof and the sum checks what was reported.

**Measured** on Apple M3 Max (16 cores), macOS 27.2, release build, one process
(no `--workers`, 8 PHP instances), bursts of 10,000:

| Run | Wall time | Claims/s | Rust | PHP | Refused |
| --- | --- | --- | --- | --- | --- |
| 1,000,000, 8 + 8 lanes | 0.062 s | 16.3 M | 11.1 M/s (680,000) | 5.2 M/s (320,000) | 0 |
| 50,000,000, 8 + 8 lanes | 2.666 s | 18.8 M | 12.7 M/s (33,760,000) | 6.1 M/s (16,240,000) | 0 |
| 10,000,000, Rust only, 8 lanes | 0.268 s | 37.3 M | 37.3 M/s | — | 0 |
| 10,000,000, PHP only, 8 lanes | 0.539 s | 18.5 M | — | 18.5 M/s | 0 |

Each verified: answered equal to the target, the value at N in the run's round,
and the index sum at N(N-1)/2 (1,249,999,975,000,000 for 50,000,000). With one
lane and bursts of 1,000,000, Rust's loop makes 494 M claims a second, about
2 ns per add with the line in its own core's cache, and PHP's 25 M, about
39 ns per `add()` call. Eight lanes or more share one cache line, so the rates
above measure that line moving between cores more than either language.

The client has a hidden option for testing the checker itself:
`--test-checker reported` makes one answer report an index taken twice, and the
run is rejected at that burst; `--test-checker silent` lowers one answer's
index sum as if one index had been taken twice and another never, which passes
every per-burst check and is rejected by the index sum.

These are local diagnostics, not a public deployment: the demo channel permits
subscription without a user login, and `/runtime` reports PHP server data. Some
invocation examples require providers not registered in this sample; they are
not a claim that every Claviron capability is active.

## Optional shared benchmarks

Set `EXAMPLES_BENCH=1` before `scripts/run.sh --release octane-link`.
The local `..` development package supplies `/bench/plain`,
`/bench/json` and `/bench/echo` through the existing Laravel application, not a
native interception shortcut. Ordinary launches leave those routes disabled.
Keep results separate from transport or PHP lifecycle correctness checks.

The Rust package is an independent workspace with `publish = false`. The entire
`..` tree is excluded from the root Claviron crate's registry package.
