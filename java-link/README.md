# Java Link

A JDK 25 application served by Claviron over Orbit Link. `App.boot()` prepares
the application once; `App.handle()` routes each request by method and path.
The JVM owns no HTTP listener. No Servlet container or web framework is needed.

| Route | Response |
| --- | --- |
| `GET /` | Web page embedded in the application JAR. |
| `GET /api/status` | Java version, process ID and request count as JSON. |
| `POST /api/echo` | Request bytes streamed back with backpressure. |

`Main` owns the Orbit connection and shutdown. Handlers use Java virtual threads.
Closing the application drains owned exchanges before closing its executor,
native runtime and library. All resource budgets are in `java-link.properties`,
including `maxInFlight=128` and `maxQueued=0`. Build with a matching ABI 5
Orbit Java library.

## Prepare

Set `JAVA_HOME` to a JDK 25 installation. The binding is on Maven Central as
`dev.claviron:orbit:0.1.1`: an API JAR, and one native JAR per platform under
a classifier (`macos-aarch64`, `linux-x86_64`, `linux-aarch64`,
`freebsd-x86_64`, `freebsd-aarch64`). This example uses no build tool, so it
takes the two JARs directly:

```sh
mkdir -p lib
base=https://repo1.maven.org/maven2/dev/claviron/orbit/0.1.1
curl -fL -o lib/orbit-0.1.1.jar "$base/orbit-0.1.1.jar"
curl -fL -o lib/orbit-0.1.1-macos-aarch64.jar "$base/orbit-0.1.1-macos-aarch64.jar"
export ORBIT_JAVA_CLASSPATH="$PWD/lib/orbit-0.1.1.jar:$PWD/lib/orbit-0.1.1-macos-aarch64.jar"
./prepare.sh
```

Use the classifier of your OS and CPU; Maven Central publishes a checksum
beside each file. `lib/` is not committed, and the scripts do not download
the binding. A project that uses Maven or Gradle declares the same
coordinates, with the classifier for the native JAR. The binding and the Edge
must speak the same Link: `orbit` 0.1.1 is built for Claviron 0.2.0-alpha.26.

## Run

```sh
./run.sh
```

The application defaults to `java-link` in fleet `java`. `--name`, `--fleet` and
`--config` can override these settings. Ctrl+C withdraws and drains the application.

Add this host to an Edge's vhost configuration:

```yaml
java.test:
  root: .
  transport: { non_ssl: serve, protocols: [h1] }
  orbit:
    - { application: java-link, fleet: java }
```

With that Edge listening on loopback port 8080:

```sh
curl --resolve java.test:8080:127.0.0.1 http://java.test:8080/api/status
curl --resolve java.test:8080:127.0.0.1 http://java.test:8080/api/echo --data 'Hello from Java'
```

Resolve `java.test` to the Edge address for browser use. The page and diagnostics
are intended for local demonstration; they expose no authentication.
