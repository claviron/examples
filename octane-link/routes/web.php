<?php

use App\Http\Controllers\BroadcastController;
use App\Http\Controllers\ClientCertificateController;
use App\Http\Controllers\DatabaseCapabilitiesController;
use App\Http\Controllers\DatabaseController;
use App\Http\Controllers\DemoEventController;
use App\Http\Controllers\HitsController;
use App\Http\Controllers\InvocationController;
use App\Http\Controllers\RuntimeController;
use App\Http\Controllers\SessionController;
use App\Http\Controllers\TilesController;
use Illuminate\Support\Facades\Route;

Route::view('/', 'welcome')->name('welcome');
Route::view('/todos', 'todos')->name('todos');

Route::get('/runtime', RuntimeController::class)->name('runtime');
Route::get('/runtime/sleep', [RuntimeController::class, 'sleep'])->name('runtime.sleep');
Route::get('/events/demo', DemoEventController::class)->name('events.demo');
Route::get('/events/shared', [DemoEventController::class, 'shared'])->name('events.shared');

// The fleet's hit count from PHP; /_lumenize/rust is the same value from Rust.
Route::get('/hits', [HitsController::class, 'show'])->name('hits');
Route::controller(HitsController::class)
    ->prefix('_lumenize/php')
    ->name('hits.php.')
    ->group(function (): void {
        Route::get('/', 'hit')->name('hit');
        Route::get('/value', 'value')->name('value');
        Route::delete('/', 'reset')->name('reset');
    });

// Tiles handed out by one shared value; /_lumenize/rust/tiles claims from the
// same value in Rust, and DELETE there starts a new round.
Route::get('/tiles', [TilesController::class, 'show'])->name('tiles');
Route::get('/_lumenize/php/tiles', [TilesController::class, 'claim'])->name('tiles.php.claim');
// Many claims in one request, on the Orbital whose id /_lumenize/rust/tiles/id
// hands out; /_lumenize/rust/burst is the same in Rust.
Route::get('/_lumenize/php/burst', [TilesController::class, 'burst'])->name('tiles.php.burst');

// What the TLS handshake verified of the client; /_lumenize/rust/client is
// the same from Rust.
Route::get('/_lumenize/php/client', ClientCertificateController::class)->name('client.php');

Route::controller(SessionController::class)
    ->prefix('session')
    ->name('session.')
    ->group(function (): void {
        Route::get('/', 'show')->name('show');
        Route::delete('/', 'destroy')->name('destroy');
    });

Route::controller(InvocationController::class)
    ->prefix('invocation')
    ->name('invocation.')
    ->group(function (): void {
        Route::get('/shout', 'shout')->name('shout');
        Route::get('/js-shout', 'jsShout')->name('js-shout');
        Route::get('/geo-ip', 'geoIp')->name('geo-ip');
        Route::get('/ping', 'ping')->name('ping');
    });

Route::controller(BroadcastController::class)
    ->prefix('broadcast')
    ->name('broadcast.')
    ->group(function (): void {
        Route::post('/message', 'message')->name('message');
        Route::post('/notification', 'notification')->name('notification');
    });

Route::controller(DatabaseController::class)
    ->prefix('database')
    ->name('database.')
    ->group(function (): void {
        Route::get('/round-trip', 'roundTrip')->name('round-trip');
        Route::get('/types', 'types')->name('types');
        Route::get('/leak-transaction', 'leakTransaction')->name('leak-transaction');
        Route::get('/capabilities', DatabaseCapabilitiesController::class)->name('capabilities');
    });

// A record on Laravel's default log channel — `claviron` inside the runtime
// (config/runtime/php.yaml sets LOG_CHANNEL), .env's outside it. Its source is
// found by the runtime, its context becomes fields.
Route::get('/log-demo', function () {
    \Illuminate\Support\Facades\Log::warning('log demo', [
        'order' => ['id' => 7, 'total' => 12.5],
        'paid' => false,
        'exception' => new RuntimeException('just a demo'),
    ]);

    return response()->json(['logged' => true]);
})->name('log-demo');
