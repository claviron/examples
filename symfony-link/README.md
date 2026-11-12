# Symfony

A persistent Symfony application hosted by Skene. It connects to the separate
Claviron Edge through Orbit as application `symfony-link` in fleet `symfony`.
Edge serves it at `symfony.test`; this process opens no HTTP listener.

| Route | Response |
| --- | --- |
| `GET /` | JSON containing the method, path and per-instance request count. |
| `POST /echo` | The same JSON with the request body. |

The PHP classes live in `app/` under the `App` namespace. `application.php`
passes the kernel to `claviron/symfony`; the adapter owns request reception,
Symfony termination and kernel shutdown. Each serving process has eight PHP
instances. Each keeps its kernel and controller between requests.

## Prepare

With the ZTS embed PHP toolchain selected through `PHP` and `PHP_CONFIG`, run
from this directory:

```sh
./prepare.sh
cargo build --release
```

Preparation installs the locked Composer dependencies, builds the authoritative
classmap and warms Symfony's production container/router cache. It is separate
from the shared launcher. `config/php.d` enables OPcache and disables timestamp
validation; restart the application after PHP source changes.

## Run

From the repository root, with Edge already running:

```sh
scripts/run.sh --release symfony-link
```

The application reads `skene.yaml`. Its supervisor starts six serving
processes. Each process has one receiver lane with a dedicated current-thread
executor. Each PHP instance has its own thread.
Edge's `symfony.test` host targets the application in
its Link fleet. Use the Edge listener's address with that Host header:

```sh
curl -H 'Host: symfony.test' http://127.0.0.1:8080/
curl -H 'Host: symfony.test' --data 'Hello from Symfony' http://127.0.0.1:8080/echo
```
