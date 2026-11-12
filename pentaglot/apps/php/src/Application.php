<?php
declare(strict_types=1);

namespace Pentaglot\Php;

final class Application
{
    private int $requests = 0;

    public function run(): void
    {
        while (($request = claviron_application_receive()) !== null) {
            claviron_application_respond($this->handle($request));
            unset($request);
        }
    }

    public function handle(array $request): array
    {
        $path = parse_url($request['uri'], PHP_URL_PATH) ?: '/';
        $method = $request['method'];
        if ($path === '/php/bench/plain') {
            return ['status' => 200, 'headers' => ['content-type' => ['text/plain']], 'body' => "ok\n"];
        }
        if ($path === '/php/bench/json') {
            return [
                'status' => 200,
                'headers' => ['content-type' => ['application/json']],
                'body' => json_encode(['message' => 'ok'], JSON_THROW_ON_ERROR),
            ];
        }
        if ($path === '/php/bench/echo' && $method === 'POST') {
            return ['status' => 200, 'headers' => ['content-type' => ['application/octet-stream']], 'body' => $request['body']];
        }
        $allowed = match ($path) {
            '/php' => 'GET',
            '/php/echo' => 'POST',
            default => null,
        };
        $status = $allowed === null ? 404 : ($method === $allowed ? 200 : 405);
        $headers = ['content-type' => ['application/json']];
        if ($status === 405) {
            $headers['allow'] = [$allowed];
        }

        return [
            'status' => $status,
            'headers' => $headers,
            'body' => json_encode([
                'runtime' => 'php',
                'engine' => 'PHP / Claviron PHP embed',
                'method' => $method,
                'path' => $path,
                'body' => $request['body'],
                'requests' => ++$this->requests,
            ], JSON_THROW_ON_ERROR),
        ];
    }
}
