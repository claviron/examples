# Go Link

A standard Go `net/http` application reached through Edge over Orbit Link.
It opens no HTTP listener. The application name is `go-link`, fleet `go`;
the example Edge serves it as `go.test`.

## Prepare

Go 1.23+ with cgo and a C compiler. The Go module is the published
[`github.com/claviron/go`](https://github.com/claviron/go) binding, pinned in
`go.mod`. cgo links a native library that `go get` does not fetch; each
[release](https://github.com/claviron/go/releases) carries one for Linux and
FreeBSD on x86-64 and arm64 and for macOS on arm64. Take the one that matches
the module version and your platform, then build. From `examples/`:

```sh
mkdir -p go-link/lib
curl -L https://github.com/claviron/go/releases/download/v0.1.3/claviron-go-0.1.3-macos-aarch64.tar.gz \
  | tar -xz -C go-link/lib
ORBIT_GO_LIBRARY_DIR="$(pwd)/go-link/lib" go-link/prepare.sh
```

`lib/` is not committed. The release's `SHA256SUMS` lists each archive's
checksum. Preparation builds the binary; the launcher only runs it. The
binding and the Edge must speak the same Link: this module version is built
for Claviron 0.2.2.

The application defaults to 128 running exchanges and zero queued exchanges.
Set `--max-in-flight` and `--max-queued` to tune these limits.

## Run

From `examples/`, check Edge before starting anything:

```sh
scripts/run.sh --status edge-server
scripts/run.sh --start go-link
curl --resolve go.test:8080:127.0.0.1 http://go.test:8080/
curl --resolve go.test:8080:127.0.0.1 http://go.test:8080/bench/json
curl --resolve go.test:8080:127.0.0.1 --data-binary 'hello' http://go.test:8080/bench/echo
scripts/run.sh --stop go-link
```

Use the existing Edge when it is running. Start it separately when needed.

## Routes

| Method | Path | Response |
| --- | --- | --- |
| GET | `/` | `Go application on Claviron` |
| GET | `/bench/plain` | `ok` |
| GET | `/bench/json` | `{"message":"ok"}` |
| POST | `/bench/echo` | Streaming request body |
| GET, POST | `/_bench/rust` | Same workload URL as Axum; identifies `from: go`, accepts `min`, `max` in milliseconds and `payload` bytes |
| GET | `/_bench/stats` | Go handler in-flight/total counters; sessions are unsupported |

No delay is added unless the request declares it. Synthetic payloads are
limited to one MiB, matching Axum. This is a workload size, not an upload limit.
SIGTERM withdraws admission and drains accepted exchanges.
