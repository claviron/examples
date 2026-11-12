package example.javalink;

import dev.claviron.orbit.ApplicationOptions;
import dev.claviron.orbit.NativeLibrary;
import dev.claviron.orbit.OrbitApplication;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.Executors;

public final class Main {
    private Main() {}

    public static void main(String[] args) throws Exception {
        for (String argument : args) {
            if (argument.equals("--help") || argument.equals("-h")) {
                System.out.println("Usage: java-link [--config FILE] [--library FILE] [--name NAME] [--fleet NAME]");
                return;
            }
        }
        Options options = Options.parse(args);
        var stop = new CompletableFuture<Void>();
        var closed = new CompletableFuture<Void>();
        Thread shutdown = new Thread(() -> {
            stop.complete(null);
            closed.join();
        }, "java-link-shutdown");
        Runtime.getRuntime().addShutdownHook(shutdown);
        try {
            try (NativeLibrary library = options.library() == null ? NativeLibrary.load() : NativeLibrary.load(options.library());
                 var runtime = library.openRuntime(options.operationBudget(), options.bufferBytes(), options.completionThreads());
                 var handlers = Executors.newVirtualThreadPerTaskExecutor()) {
                App app = App.boot(handlers, options.bodyChunkBytes());
                try (OrbitApplication application = runtime.join(new ApplicationOptions(options.fleet(), options.name(),
                        System.currentTimeMillis(), options.maxInFlight(), options.maxQueued(), options.eventCapacity(), options.controlCapacity()))) {
                    application.serve(app, handlers).whenComplete((ignored, failure) -> {
                        if (failure == null) stop.complete(null);
                        else stop.completeExceptionally(failure);
                    });
                    System.out.printf("Java Link ready: application=%s fleet=%s pid=%d%n",
                            options.name(), options.fleet(), ProcessHandle.current().pid());
                    stop.join();
                }
            }
        } finally {
            closed.complete(null);
            try {
                Runtime.getRuntime().removeShutdownHook(shutdown);
            } catch (IllegalStateException shuttingDown) {
                // The shutdown hook is waiting for owned application work to drain.
            }
        }
    }
}
