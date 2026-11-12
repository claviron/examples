/* A C application reached through Edge over Orbit Link. It opens no listener. */
#define _POSIX_C_SOURCE 200809L
#include "orbit.h"
#include <errno.h>
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* The application declares its budgets; the library invents none. */
enum { EXCHANGES = 128, QUEUED = 64, WINDOW = 8192, MESSAGE_LIMIT = 65536, NATIVE_THREADS = 1, PATH_LIMIT = 256 };

#define TEXT(literal) (const uint8_t *)(literal), sizeof(literal) - 1

static uint32_t answer(orbit_request *request, uint32_t code, const uint8_t *body, uint64_t length) {
    uint32_t status = orbit_response_begin(request, code);
    if (status == ORBIT_OK)
        status = orbit_response_header(request, TEXT("content-type"), TEXT("text/plain; charset=utf-8"));
    if (status == ORBIT_OK) status = orbit_response_write(request, body, length);
    return status == ORBIT_OK ? orbit_response_finish(request) : status;
}

/* Field 3 is the raw path. A longer path than this application knows is no route. */
static int is_path(const orbit_request *request, const char *path) {
    uint8_t found[PATH_LIMIT];
    uint64_t length = 0;
    uint32_t status = orbit_request_copy(orbit_request_handle(request), 3, 0, NULL, 0, &length);
    if (status != ORBIT_OK || length != strlen(path) || length > sizeof found) return 0;
    status = orbit_request_copy(orbit_request_handle(request), 3, 0, found, length, &length);
    return status == ORBIT_OK && memcmp(found, path, length) == 0;
}

static uint32_t http_request(orbit_request *request, void *context) {
    (void)context;
    if (is_path(request, "/")) return answer(request, 200, TEXT("C application on Claviron\n"));
    if (is_path(request, "/bench/plain")) return answer(request, 200, TEXT("ok\n"));
    if (is_path(request, "/bench/json")) {
        uint32_t status = orbit_response_begin(request, 200);
        if (status == ORBIT_OK)
            status = orbit_response_header(request, TEXT("content-type"), TEXT("application/json"));
        if (status == ORBIT_OK) status = orbit_response_write(request, TEXT("{\"message\":\"ok\"}"));
        return status == ORBIT_OK ? orbit_response_finish(request) : status;
    }
    if (is_path(request, "/bench/echo")) {
        uint32_t status = orbit_response_begin(request, 200);
        if (status == ORBIT_OK)
            status = orbit_response_header(request, TEXT("content-type"), TEXT("application/octet-stream"));
        return status == ORBIT_OK ? orbit_request_read_body(request) : status;
    }
    if (is_path(request, "/echo")) {
        uint32_t status = orbit_response_begin(request, 200);
        return status == ORBIT_OK ? orbit_request_read_body(request) : status;
    }
    return answer(request, 404, TEXT("not found\n"));
}

/* The next chunk arrives once this one's output has drained. */
static uint32_t http_body(orbit_request *request, const uint8_t *data, uint64_t length, uint32_t end, void *context) {
    (void)context;
    return end ? orbit_response_finish(request) : orbit_response_write(request, data, length);
}

static uint32_t socket_open(orbit_request *request, void *context) {
    (void)context;
    if (is_path(request, "/ws")) return orbit_session_accept(request, MESSAGE_LIMIT);
    uint32_t status = orbit_response_begin(request, 404);
    return status == ORBIT_OK ? orbit_response_finish(request) : status;
}

static uint32_t socket_message(orbit_request *request, uint32_t kind, uint32_t code,
    const uint8_t *data, uint64_t length, void *context) {
    (void)context;
    /* Native I/O answers a peer's close; an application does not send it twice. */
    if (kind == ORBIT_MESSAGE_END || kind == ORBIT_MESSAGE_CLOSE) return ORBIT_OK;
    return orbit_session_send(request, kind, code, data, length);
}

/* A stop signal drains: admission ends and accepted exchanges finish. */
static void *signals(void *value) {
    sigset_t set;
    sigemptyset(&set); sigaddset(&set, SIGINT); sigaddset(&set, SIGTERM); sigaddset(&set, SIGUSR1);
    int received;
    if (sigwait(&set, &received) == 0 && received != SIGUSR1)
        orbit_withdraw(*(const uint64_t *)value);
    return NULL;
}

static int thread_count(const char *text, uint32_t *value) {
    char *end;
    errno = 0;
    unsigned long count = strtoul(text, &end, 10);
    if (errno || !*text || *text == '-' || *end || count == 0 || count > UINT32_MAX) return 0;
    *value = (uint32_t)count;
    return 1;
}

static void usage(const char *program) {
    printf("Usage: %s [--fleet FLEET] [--name APPLICATION] "
        "[--execution multi|current] [--threads N]\n"
        "Defaults: fleet=c, name=c-link, execution=multi, threads=1.\n"
        "Current execution requires exactly one I/O thread. No supervisor is started.\n", program);
}

int main(int argc, char **argv) {
    const char *fleet = "c", *name = "c-link";
    orbit_runtime_options execution = {.abi_version = ORBIT_ABI_VERSION,
        .execution = ORBIT_MULTI_THREAD, .native_threads = NATIVE_THREADS};
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--help") == 0) { usage(argv[0]); return 0; }
        else if (strcmp(argv[i], "--fleet") == 0 && i + 1 < argc) fleet = argv[++i];
        else if (strcmp(argv[i], "--name") == 0 && i + 1 < argc) name = argv[++i];
        else if (strcmp(argv[i], "--execution") == 0 && i + 1 < argc) {
            const char *mode = argv[++i];
            if (strcmp(mode, "multi") == 0) execution.execution = ORBIT_MULTI_THREAD;
            else if (strcmp(mode, "current") == 0) execution.execution = ORBIT_CURRENT_THREAD;
            else { usage(argv[0]); return 2; }
        } else if (strcmp(argv[i], "--threads") == 0 && i + 1 < argc) {
            if (!thread_count(argv[++i], &execution.native_threads)) { usage(argv[0]); return 2; }
        } else { usage(argv[0]); return 2; }
    }
    if (execution.execution == ORBIT_CURRENT_THREAD && execution.native_threads != 1) {
        fprintf(stderr, "Current execution requires --threads 1\n");
        return 2;
    }
    sigset_t set;
    sigemptyset(&set); sigaddset(&set, SIGINT); sigaddset(&set, SIGTERM); sigaddset(&set, SIGUSR1);
    if (pthread_sigmask(SIG_BLOCK, &set, NULL) != 0) return 1;

    orbit_options options = {.abi_version = ORBIT_ABI_VERSION, .incarnation = 1,
        .max_in_flight = EXCHANGES, .max_queued = QUEUED, .io_buffer_bytes = WINDOW};
    uint64_t runtime = 0, app = 0;
    uint32_t status = orbit_runtime_open(&execution, &runtime);
    if (status != ORBIT_OK) { fprintf(stderr, "runtime: status %u\n", status); return 1; }
    status = orbit_open(runtime, (const uint8_t *)fleet, strlen(fleet),
        (const uint8_t *)name, strlen(name), &options, &app);
    if (status != ORBIT_OK) {
        fprintf(stderr, "open: status %u\n", status);
        orbit_runtime_release(runtime);
        return 1;
    }
    pthread_t waiter;
    if (pthread_create(&waiter, NULL, signals, &app) != 0) {
        orbit_cancel(app);
        orbit_event event;
        do { status = orbit_wait(app, &event); } while (status == ORBIT_OK && event.kind != ORBIT_APPLICATION_CLOSED);
        orbit_release(app);
        orbit_runtime_release(runtime);
        return 1;
    }
    orbit_http_handler http = {.request = http_request, .body = http_body};
    orbit_session_handler socket = {.open = socket_open, .message = socket_message};
    printf("%s joined fleet %s (execution=%s, I/O threads=%u)\n", name, fleet,
        execution.execution == ORBIT_CURRENT_THREAD ? "current" : "multi", execution.native_threads);
    fflush(stdout);
    status = orbit_serve(app, &http, &socket, NULL);
    pthread_kill(waiter, SIGUSR1);
    pthread_join(waiter, NULL);
    int failed = status != ORBIT_OK;
    if (orbit_release(app) != ORBIT_OK) failed = 1;
    if (orbit_runtime_release(runtime) != ORBIT_OK) failed = 1;
    return failed;
}
