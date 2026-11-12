package example.javalink;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Properties;
import java.util.Set;

record Options(String name, String fleet, Path library, int operationBudget,
               long bufferBytes, int completionThreads, int maxInFlight, int maxQueued,
               int eventCapacity, int controlCapacity, int bodyChunkBytes) {
    static Options parse(String[] args) throws IOException {
        Path config = Path.of("java-link.properties");
        Path library = null;
        String name = null;
        String fleet = null;
        for (int i = 0; i < args.length; i++) {
            String flag = args[i];
            if (i + 1 == args.length) throw new IllegalArgumentException("Missing value for " + flag);
            String value = args[++i];
            switch (flag) {
                case "--config" -> config = Path.of(value);
                case "--library" -> library = Path.of(value);
                case "--name" -> name = value;
                case "--fleet" -> fleet = value;
                default -> throw new IllegalArgumentException("Unknown option: " + flag);
            }
        }
        Properties values = new Properties();
        try (var reader = Files.newBufferedReader(config)) {
            values.load(reader);
        }
        Set<String> keys = Set.of("name", "fleet", "operationBudget", "bufferBytes",
                "completionThreads", "maxInFlight", "maxQueued", "eventCapacity", "controlCapacity", "bodyChunkBytes");
        for (String key : values.stringPropertyNames()) {
            if (key.equals("exchangeBudget") || key.equals("maxExchanges")) {
                throw new IllegalArgumentException(key + " was replaced by maxInFlight and maxQueued");
            }
            if (key.equals("maxWaiting")) {
                throw new IllegalArgumentException("maxWaiting was replaced by maxQueued, which counts only waiting exchanges");
            }
            if (!keys.contains(key)) throw new IllegalArgumentException("Unknown setting: " + key);
        }
        return new Options(name == null ? required(values, "name") : name,
                fleet == null ? required(values, "fleet") : fleet, library,
                positiveInt(values, "operationBudget"), positive(values, "bufferBytes"),
                positiveInt(values, "completionThreads"), positiveInt(values, "maxInFlight"), nonNegativeInt(values, "maxQueued"),
                positiveInt(values, "eventCapacity"), positiveInt(values, "controlCapacity"),
                positiveInt(values, "bodyChunkBytes"));
    }

    private static String required(Properties values, String key) {
        String value = values.getProperty(key);
        if (value == null || value.isBlank()) throw new IllegalArgumentException("Missing setting: " + key);
        return value.strip();
    }

    private static long positive(Properties values, String key) {
        long value = Long.parseLong(required(values, key));
        if (value <= 0) throw new IllegalArgumentException(key + " must be positive");
        return value;
    }

    private static int nonNegativeInt(Properties values, String key) {
        int value = Integer.parseInt(required(values, key));
        if (value < 0) throw new IllegalArgumentException(key + " must be non-negative");
        return value;
    }

    private static int positiveInt(Properties values, String key) {
        return Math.toIntExact(positive(values, key));
    }
}
