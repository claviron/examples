<?php

declare(strict_types=1);

namespace App\Http\Controllers;

use Illuminate\Contracts\View\View;
use Illuminate\Http\Response;

/**
 * PHP's side of the fleet's hit count.
 *
 * The Rust side declares the value `hits` at boot; these actions reach the
 * same value as the Orbit counter `hits`, from every PHP instance of every
 * worker.
 */
final class HitsController extends Controller
{
    private const KEY = 'hits';

    /** The page that runs both sides against the value at once. */
    public function show(): View
    {
        return view('hits', ['hits' => orbit_counter_get(self::KEY)]);
    }

    /** Add one and return the count after it. */
    public function hit(): Response
    {
        $value = orbit_counter_inc(self::KEY, 1);

        return $value === false
            ? $this->unavailable()
            : $this->hits($value);
    }

    public function value(): Response
    {
        $value = orbit_counter_get(self::KEY);

        return $value === null
            ? $this->unavailable()
            : $this->hits($value);
    }

    /** Set the count to zero and return what it reads afterwards. */
    public function reset(): Response
    {
        if (! orbit_counter_reset(self::KEY)) {
            return $this->unavailable();
        }

        return $this->value();
    }

    private function hits(int $value): Response
    {
        return response("hits={$value}\n", 200, [
            'Content-Type' => 'text/plain; charset=utf-8',
        ]);
    }

    private function unavailable(): Response
    {
        return response("the Orbit counter 'hits' is unavailable\n", 503, [
            'Content-Type' => 'text/plain; charset=utf-8',
        ]);
    }
}
