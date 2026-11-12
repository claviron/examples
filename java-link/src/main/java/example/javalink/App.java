package example.javalink;

import dev.claviron.orbit.Header;
import dev.claviron.orbit.NativeBuffer;
import dev.claviron.orbit.OrbitApplication;
import dev.claviron.orbit.OrbitRequest;
import dev.claviron.orbit.OrbitResponse;
import java.io.IOException;
import java.lang.foreign.ValueLayout;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionStage;
import java.util.concurrent.Executor;
import java.util.concurrent.atomic.AtomicLong;

/** Boot once, then handle requests without owning an HTTP listener. */
final class App implements OrbitApplication.Handler {
    private static final List<Header> PLAIN = List.of(Header.text("content-type", "text/plain"));
    private static final List<Header> JSON = List.of(Header.text("content-type", "application/json"));
    private static final List<Header> BINARY = List.of(Header.text("content-type", "application/octet-stream"));
    private static final OrbitResponse NOT_FOUND = OrbitResponse.text(404, List.of(), "Not Found\n");
    private final OrbitResponse page;
    private final Executor executor;
    private final int chunkBytes;
    private final AtomicLong requests = new AtomicLong();

    private App(OrbitResponse page, Executor executor, int chunkBytes) {
        this.page = page;
        this.executor = executor;
        this.chunkBytes = chunkBytes;
    }

    static App boot(Executor executor, int chunkBytes) throws IOException {
        try (var input = Objects.requireNonNull(App.class.getResourceAsStream("/index.html"), "Missing index.html")) {
            var page = OrbitResponse.copyOf(200, List.of(Header.text("content-type", "text/html; charset=utf-8")),
                    input.readAllBytes());
            return new App(page, executor, chunkBytes);
        }
    }

    @Override
    public CompletionStage<OrbitResponse> handle(OrbitRequest request) {
        long count = requests.incrementAndGet();
        String path = request.uri().getRawPath();
        String method = request.method();
        OrbitResponse response = switch (path) {
            case "/" -> method.equals("GET") ? page : methodNotAllowed("GET");
            case "/bench/plain" -> OrbitResponse.text(200, PLAIN, "ok\n");
            case "/bench/json" -> OrbitResponse.text(200, JSON, "{\"message\":\"ok\"}");
            case "/bench/echo" -> method.equals("POST") ? echo(request) : methodNotAllowed("POST");
            case "/api/status" -> method.equals("GET")
                    ? OrbitResponse.text(200, JSON, """
                        {"runtime":"java","version":%d,"pid":%d,"requests":%d}
                        """.formatted(Runtime.version().feature(), ProcessHandle.current().pid(), count))
                    : methodNotAllowed("GET");
            case "/api/echo" -> method.equals("POST") ? echo(request) : methodNotAllowed("POST");
            default -> NOT_FOUND;
        };
        return CompletableFuture.completedStage(response);
    }

    private OrbitResponse echo(OrbitRequest request) {
        return OrbitResponse.streaming(200, BINARY, output -> CompletableFuture.runAsync(() -> {
            for (;;) {
                NativeBuffer chunk = request.readBody(chunkBytes).completion().toCompletableFuture().join();
                if (chunk == null) return;
                try (chunk) {
                    byte[] bytes = chunk.view().toArray(ValueLayout.JAVA_BYTE);
                    output.write(bytes).toCompletableFuture().join();
                }
            }
        }, executor));
    }

    private static OrbitResponse methodNotAllowed(String allowed) {
        return OrbitResponse.text(405, List.of(Header.text("allow", allowed)), "Method Not Allowed\n");
    }
}
