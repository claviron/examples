# C module

Two web applications, one in C and one in C++, compiled into libraries that Pentaglot's Edge
loads into its process. Neither links against Claviron and nobody writes Rust: each exports one
function and registers what it is.

| Library | Routes |
| --- | --- |
| `hello.c` | `/c`, `POST /c/echo`, `/c/fail` and a WebSocket at `/c/ws` |
| `hello_cpp.cpp` | `/cpp`, and `/cpp/throw`, catching the exception at the ABI |

`pentaglot/config/modules.yaml` lists the libraries and declares how their applications run. Its
routes select them by name. From outside they are ordinary applications, listed and managed like
any other.

`include/claviron_module.h` is a copy of the contract, the one file a library needs from Claviron.
A library built against another ABI version is refused at boot.

## Run

Build the libraries first: Pentaglot does not boot while `modules.yaml` names a library that is
missing. From `examples/`, with Pentaglot prepared as its README says:

```sh
cmake -S c-module -B c-module/build
cmake --build c-module/build
scripts/run.sh --start pentaglot
curl http://127.0.0.1:8080/c
curl --data-binary @c-module/README.md http://127.0.0.1:8080/c/echo
curl http://127.0.0.1:8080/c/fail       # 500
curl http://127.0.0.1:8080/cpp/throw    # 500, not a crash
scripts/run.sh --stop pentaglot
```

A memory fault in a library takes its whole process with it, and every other application in it.
Where that isolation matters, a C application runs in its own process over Orbit: see `c-link`.
