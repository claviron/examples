// A second library in the same process, in C++. An exception must not cross the
// ABI: it is caught at the edge of every call and becomes a status.
#include <claviron_module.h>

#include <cstring>
#include <stdexcept>
#include <string>

namespace {

const claviron_host *host;

claviron_slice text(const std::string &value) {
    return {reinterpret_cast<const uint8_t *>(value.data()), value.size()};
}

std::string field(const claviron_request *request, uint32_t which) {
    claviron_slice value{};
    host->request_field(request, which, 0, &value);
    return {reinterpret_cast<const char *>(value.data), value.length};
}

std::string answer(const std::string &path) {
    if (path == "/cpp/throw") throw std::runtime_error("thrown in C++");
    return "{\"application\":\"c++\",\"path\":\"" + path + "\"}\n";
}

uint32_t on_request(claviron_request *request, void *) {
    try {
        const std::string path = field(request, CLAVIRON_FIELD_PATH);
        if (path == "/cpp/bench/plain" || path == "/cpp/bench/json") {
            const bool plain = path == "/cpp/bench/plain";
            host->response_begin(request, 200);
            host->response_header(request, text("content-type"), text(plain ? "text/plain" : "application/json"));
            host->response_write(request, text(plain ? "ok\n" : "{\"message\":\"ok\"}"));
            host->response_finish(request);
            return CLAVIRON_OK;
        }
        if (path == "/cpp/bench/echo") {
            host->response_begin(request, 200);
            host->response_header(request, text("content-type"), text("application/octet-stream"));
            return host->request_read_body(request);
        }
        const std::string body = answer(field(request, CLAVIRON_FIELD_PATH));
        host->response_begin(request, 200);
        host->response_header(request, text("content-type"), text("application/json"));
        host->response_write(request, text(body));
        host->response_finish(request);
        return CLAVIRON_OK;
    } catch (const std::exception &failure) {
        host->log(CLAVIRON_LOG_ERROR, text(failure.what()));
        return CLAVIRON_ERROR;  // No reply began, so this is a 500.
    }
}

}  // namespace

/* The next chunk arrives once the reply has room for it. */
static uint32_t on_body(claviron_request *request, const uint8_t *data, uint64_t length, uint32_t end, void *) {
    if (end) return host->response_finish(request);
    return host->response_write(request, claviron_slice{data, length});
}

extern "C" uint32_t claviron_module_abi_version(void) {
    return CLAVIRON_MODULE_ABI_VERSION;
}

extern "C" uint32_t claviron_module_register(const claviron_host *h, claviron_registrar *registrar) {
    host = h;
    claviron_http_handler http{};
    http.struct_size = sizeof http;
    http.request = on_request;
    http.body = on_body;
    claviron_application application{};
    application.struct_size = sizeof application;
    application.http = &http;
    return host->register_application(registrar, text("hello-cpp"), &application);
}
