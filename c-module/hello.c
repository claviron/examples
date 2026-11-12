/* A web application in C: HTTP and WebSocket. */
#include <claviron_module.h>

#include <string.h>

static const claviron_host *host;

#define TEXT(literal) ((claviron_slice){(const uint8_t *)(literal), strlen(literal)})

static void reply(claviron_request *request, uint32_t status, const char *type, const char *body) {
    host->response_begin(request, status);
    host->response_header(request, TEXT("content-type"), TEXT(type));
    host->response_write(request, TEXT(body));
    host->response_finish(request);
}

static int is(claviron_slice value, const char *literal) {
    return value.length == strlen(literal) && memcmp(value.data, literal, value.length) == 0;
}

static uint32_t on_request(claviron_request *request, void *context) {
    (void)context;
    claviron_slice path;
    host->request_field(request, CLAVIRON_FIELD_PATH, 0, &path);
    if (is(path, "/c/bench/plain")) {
        reply(request, 200, "text/plain", "ok\n");
        return CLAVIRON_OK;
    }
    if (is(path, "/c/bench/json")) {
        reply(request, 200, "application/json", "{\"message\":\"ok\"}");
        return CLAVIRON_OK;
    }
    if (is(path, "/c/bench/echo")) {
        host->response_begin(request, 200);
        host->response_header(request, TEXT("content-type"), TEXT("application/octet-stream"));
        return host->request_read_body(request);
    }
    if (is(path, "/c")) {
        reply(request, 200, "application/json", "{\"application\":\"c\",\"engine\":\"native module\"}\n");
        return CLAVIRON_OK;
    }
    if (is(path, "/c/echo")) {
        host->response_begin(request, 200);
        return host->request_read_body(request);
    }
    /* A status with no reply yet is answered 500. */
    if (is(path, "/c/fail")) return CLAVIRON_ERROR;
    reply(request, 404, "text/plain", "not found\n");
    return CLAVIRON_OK;
}

/* The next chunk arrives once the reply has room for it. */
static uint32_t on_body(claviron_request *request, const uint8_t *data, uint64_t length, uint32_t end, void *context) {
    (void)context;
    if (end) return host->response_finish(request);
    return host->response_write(request, (claviron_slice){data, length});
}

static uint32_t on_open(claviron_request *request, void *context) {
    (void)context;
    claviron_slice path;
    host->request_field(request, CLAVIRON_FIELD_PATH, 0, &path);
    if (!is(path, "/c/ws")) {
        host->response_begin(request, 404);
        return host->response_finish(request);
    }
    return host->session_accept(request, 1024 * 1024);
}

static uint32_t on_message(claviron_request *request, uint32_t kind, uint32_t code, const uint8_t *data, uint64_t length, void *context) {
    (void)context;
    if (kind == CLAVIRON_MESSAGE_CLOSE) {
        /* 1001 that the client did not send: the application is stopping. */
        if (code == 1001) host->session_send(request, CLAVIRON_MESSAGE_CLOSE, 1001, TEXT("stopping"));
        return CLAVIRON_OK;
    }
    return host->session_send(request, kind, 0, (claviron_slice){data, length});
}

uint32_t claviron_module_abi_version(void) {
    return CLAVIRON_MODULE_ABI_VERSION;
}

/* What exists, and nothing else: no port, no thread and no file here. */
uint32_t claviron_module_register(const claviron_host *h, claviron_registrar *registrar) {
    host = h;
    claviron_http_handler http = {sizeof http, on_request, on_body, NULL, NULL};
    claviron_session_handler websocket = {sizeof websocket, on_open, on_message, NULL, NULL};
    claviron_application application = {sizeof application, NULL, NULL, NULL, &http, &websocket};
    return host->register_application(registrar, TEXT("hello"), &application);
}
