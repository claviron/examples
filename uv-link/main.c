/* Orbit callbacks run on this application's libuv loop. Native I/O stays on Tokio. */
#define _POSIX_C_SOURCE 200809L
#include "orbit.h"
#include <uv.h>
#include <errno.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

enum { EXCHANGES = 128, WINDOW = 8192, MESSAGE_LIMIT = 65536, PATH_LIMIT = 256 };
#define TEXT(literal) (const uint8_t *)(literal), sizeof(literal) - 1

typedef struct {
    uv_loop_t loop;
    uv_async_t ready;
    uv_signal_t interrupt, terminate;
    orbit_dispatcher *dispatcher;
    uint64_t runtime, application, dispatch_budget;
    int failed;
} application;

static int path_is(const orbit_request *request, const char *path) {
    uint8_t found[PATH_LIMIT];
    uint64_t length = 0;
    uint32_t status = orbit_request_copy(orbit_request_handle(request), 3, 0, NULL, 0, &length);
    if (status != ORBIT_OK || length != strlen(path) || length > sizeof found) return 0;
    status = orbit_request_copy(orbit_request_handle(request), 3, 0, found, length, &length);
    return status == ORBIT_OK && memcmp(found, path, length) == 0;
}

static uint32_t answer(orbit_request *request, uint32_t code,
    const char *content_type, const uint8_t *body, uint64_t length) {
    uint32_t status = orbit_response_begin(request, code);
    if (status == ORBIT_OK) status = orbit_response_header(request, TEXT("content-type"),
        (const uint8_t *)content_type, strlen(content_type));
    if (status == ORBIT_OK) status = orbit_response_write(request, body, length);
    return status == ORBIT_OK ? orbit_response_finish(request) : status;
}

static uint32_t http_request(orbit_request *request, void *context) {
    (void)context;
    if (path_is(request, "/")) return answer(request, 200, "text/plain; charset=utf-8",
        TEXT("libuv application on Claviron\n"));
    if (path_is(request, "/bench/plain")) return answer(request, 200, "text/plain; charset=utf-8", TEXT("ok\n"));
    if (path_is(request, "/bench/json")) return answer(request, 200, "application/json", TEXT("{\"message\":\"ok\"}"));
    if (path_is(request, "/echo") || path_is(request, "/bench/echo")) {
        uint32_t status = orbit_response_begin(request, 200);
        return status == ORBIT_OK ? orbit_request_read_body(request) : status;
    }
    return answer(request, 404, "text/plain; charset=utf-8", TEXT("not found\n"));
}

static uint32_t http_body(orbit_request *request, const uint8_t *data,
    uint64_t length, uint32_t end, void *context) {
    (void)context;
    return end ? orbit_response_finish(request) : orbit_response_write(request, data, length);
}

static uint32_t socket_open(orbit_request *request, void *context) {
    (void)context;
    if (path_is(request, "/ws") || path_is(request, "/wt")) return orbit_session_accept(request, MESSAGE_LIMIT);
    uint32_t status = orbit_response_begin(request, 404);
    return status == ORBIT_OK ? orbit_response_finish(request) : status;
}

static uint32_t socket_message(orbit_request *request, uint32_t kind, uint32_t code,
    const uint8_t *data, uint64_t length, void *context) {
    (void)context;
    if (kind == ORBIT_MESSAGE_END || kind == ORBIT_MESSAGE_CLOSE) return ORBIT_OK;
    return orbit_session_send(request, kind, code, data, length);
}

static void close_handle(uv_handle_t *handle, void *context) {
    (void)context;
    if (!uv_is_closing(handle)) uv_close(handle, NULL);
}

static void report(application *app, const char *operation, uint32_t status) {
    if (status != ORBIT_OK) {
        fprintf(stderr, "%s: Orbit status %u\n", operation, status);
        app->failed = 1;
    }
}

/* Called from native I/O threads. Only signal libuv; dispatch happens later. */
static void wake(void *context) {
    (void)uv_async_send(context);
}

static void dispatch(uv_async_t *ready) {
    application *app = ready->data;
    uint64_t processed;
    uint32_t closed;
    uint32_t status = orbit_dispatcher_poll(app->dispatcher, app->dispatch_budget, &processed, &closed);
    report(app, "dispatch", status);
    if (closed) {
        report(app, "dispatcher release", orbit_dispatcher_release(app->dispatcher));
        app->dispatcher = NULL;
        /* Terminal dispatch detached wake before any libuv handle is closed. */
        uv_walk(&app->loop, close_handle, NULL);
    } else if (status != ORBIT_OK) {
        report(app, "cancel", orbit_cancel(app->application));
    }
}

static void stop(uv_signal_t *signal, int number) {
    (void)number;
    application *app = signal->data;
    report(app, "withdraw", orbit_withdraw(app->application));
}

static int positive(const char *text, uint32_t *value) {
    char *end;
    errno = 0;
    unsigned long count = strtoul(text, &end, 10);
    if (errno || !*text || *text == '-' || *end || count == 0 || count > UINT32_MAX) return 0;
    *value = (uint32_t)count;
    return 1;
}

static void usage(const char *program) {
    printf("Usage: %s [--fleet FLEET] [--name APPLICATION] "
        "[--execution current|multi] [--threads N] [--dispatch N]\n"
        "Defaults: fleet=uv, name=uv-link, execution=current, threads=1, dispatch=64.\n"
        "Callbacks run on libuv; execution selects native Tokio I/O. No supervisor is started.\n", program);
}

int main(int argc, char **argv) {
    const char *fleet = "uv", *name = "uv-link";
    uint32_t dispatch_budget = 64;
    orbit_runtime_options execution = {.abi_version = ORBIT_ABI_VERSION,
        .execution = ORBIT_CURRENT_THREAD, .native_threads = 1};
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--help") == 0) { usage(argv[0]); return 0; }
        else if (strcmp(argv[i], "--fleet") == 0 && i + 1 < argc) fleet = argv[++i];
        else if (strcmp(argv[i], "--name") == 0 && i + 1 < argc) name = argv[++i];
        else if (strcmp(argv[i], "--execution") == 0 && i + 1 < argc) {
            const char *mode = argv[++i];
            if (strcmp(mode, "current") == 0) execution.execution = ORBIT_CURRENT_THREAD;
            else if (strcmp(mode, "multi") == 0) execution.execution = ORBIT_MULTI_THREAD;
            else { usage(argv[0]); return 2; }
        } else if (strcmp(argv[i], "--threads") == 0 && i + 1 < argc) {
            if (!positive(argv[++i], &execution.native_threads)) { usage(argv[0]); return 2; }
        } else if (strcmp(argv[i], "--dispatch") == 0 && i + 1 < argc) {
            if (!positive(argv[++i], &dispatch_budget)) { usage(argv[0]); return 2; }
        } else { usage(argv[0]); return 2; }
    }
    if (execution.execution == ORBIT_CURRENT_THREAD && execution.native_threads != 1) {
        fprintf(stderr, "Current execution requires --threads 1\n");
        return 2;
    }
    application app = {.dispatch_budget = dispatch_budget};
    int error = uv_loop_init(&app.loop);
    if (error < 0) { fprintf(stderr, "libuv: %s\n", uv_strerror(error)); return 1; }
    if ((error = uv_async_init(&app.loop, &app.ready, dispatch)) < 0 ||
        (error = uv_signal_init(&app.loop, &app.interrupt)) < 0 ||
        (error = uv_signal_init(&app.loop, &app.terminate)) < 0) goto uv_error;
    app.ready.data = app.interrupt.data = app.terminate.data = &app;
    if ((error = uv_signal_start(&app.interrupt, stop, SIGINT)) < 0 ||
        (error = uv_signal_start(&app.terminate, stop, SIGTERM)) < 0) goto uv_error;

    uint32_t status = orbit_runtime_open(&execution, &app.runtime);
    report(&app, "runtime open", status);
    if (status != ORBIT_OK) goto close_loop;
    orbit_options options = {.abi_version = ORBIT_ABI_VERSION, .incarnation = 1,
        .max_in_flight = EXCHANGES, .max_waiting = EXCHANGES, .io_buffer_bytes = WINDOW};
    status = orbit_open(app.runtime, (const uint8_t *)fleet, strlen(fleet),
        (const uint8_t *)name, strlen(name), &options, &app.application);
    report(&app, "application open", status);
    if (status != ORBIT_OK) goto release_runtime;
    orbit_http_handler http = {.request = http_request, .body = http_body};
    orbit_session_handler session = {.open = socket_open, .message = socket_message};
    orbit_loop loop = {.abi_version = ORBIT_ABI_VERSION, .struct_size = sizeof loop,
        .wake = wake, .context = &app.ready};
    status = orbit_dispatcher_open(app.application, &http, &session, &session, &loop, &app.dispatcher);
    report(&app, "dispatcher open", status);
    if (status == ORBIT_OK) {
        printf("%s joined fleet %s (libuv callbacks, execution=%s, I/O threads=%u)\n", name, fleet,
            execution.execution == ORBIT_CURRENT_THREAD ? "current" : "multi", execution.native_threads);
        fflush(stdout);
        uv_run(&app.loop, UV_RUN_DEFAULT);
    } else {
        report(&app, "cancel", orbit_cancel(app.application));
        report(&app, "drain", orbit_serve(app.application, &http, &session, &session));
    }
    report(&app, "application release", orbit_release(app.application));
release_runtime:
    report(&app, "runtime release", orbit_runtime_release(app.runtime));
close_loop:
    uv_walk(&app.loop, close_handle, NULL);
    uv_run(&app.loop, UV_RUN_DEFAULT);
    error = uv_loop_close(&app.loop);
    if (error < 0) { fprintf(stderr, "libuv close: %s\n", uv_strerror(error)); app.failed = 1; }
    return app.failed;
uv_error:
    fprintf(stderr, "libuv: %s\n", uv_strerror(error));
    app.failed = 1;
    goto close_loop;
}
