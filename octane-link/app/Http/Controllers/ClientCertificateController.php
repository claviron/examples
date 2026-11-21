<?php

declare(strict_types=1);

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use Illuminate\Http\Response;

/**
 * What the connection's TLS handshake verified of the client, as PHP sees it:
 * mod_ssl's server variables, which the runner sets from the connection.
 *
 * Without TLS none of them is set. With TLS, SSL_CLIENT_VERIFY is NONE when
 * the client presented no certificate and SUCCESS when the handshake verified
 * one, whose subject and issuer then follow.
 */
final class ClientCertificateController extends Controller
{
    public function __invoke(Request $request): Response
    {
        $lines = [
            'verify='.$request->server('SSL_CLIENT_VERIFY', ''),
            'subject='.$request->server('SSL_CLIENT_S_DN', ''),
            'issuer='.$request->server('SSL_CLIENT_I_DN', ''),
        ];

        return response(implode("\n", $lines)."\n", 200, ['Content-Type' => 'text/plain; charset=UTF-8']);
    }
}
