<?php

declare(strict_types=1);

namespace App\Http\Controllers;

use Claviron\Runtime\Orbital;
use Illuminate\Contracts\View\View;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

/**
 * PHP's side of the tiles page and of the orchestra.
 *
 * The Rust side declares the value `tiles` at boot: the round in its high 32
 * bits, the next tile in its low 32. A claim here is one
 * `orbit_counter_inc('tiles', 1)`, the same add of one that
 * `/_lumenize/rust/tiles` makes; the tile and its round are the value before
 * it. The reset is Rust's, `DELETE /_lumenize/rust/tiles`, because it needs a
 * compare-exchange the counter functions do not offer.
 *
 * A burst is many claims in one request, on the same value opened as an
 * Orbital by the id Rust hands out at `/_lumenize/rust/tiles/id`.
 */
final class TilesController extends Controller
{
    private const string KEY = 'tiles';

    /**
     * The most claims one burst performs; Rust's burst has the same bound. A
     * burst holds its PHP instance until it is done. The sum of a burst's
     * indices is below BURST_MAX * 2^32, under 2^52, far inside PHP's 64-bit
     * integers (any burst below 2^31 claims would still fit).
     */
    private const int BURST_MAX = 1_000_000;

    /**
     * The value this instance claims from in bursts, opened once by the id a
     * client handed over, and that id. Kept in the instance's statics for as
     * long as it lives; a request naming the same id reuses the handle, and
     * only one naming another id opens again.
     */
    private static ?Orbital $orbital = null;

    private static ?string $orbitalId = null;

    /**
     * Who serves the claims of this PHP instance. Fixed for the instance:
     * read on its first claim and kept, since each instance's thread keeps
     * its own statics for as long as it lives.
     *
     * @var array{pid: int, worker: int|null, execution: string|null}|null
     */
    private static ?array $identity = null;

    /** The page that paints the tiles. */
    public function show(): View
    {
        return view('tiles');
    }

    /** Take the next tile. */
    public function claim(): JsonResponse
    {
        $after = orbit_counter_inc(self::KEY, 1);
        if ($after === false) {
            return response()->json(['error' => "the Orbit counter 'tiles' is unavailable"], 503);
        }
        $taken = $after - 1;

        return response()->json([
            'side' => 'php',
            'round' => $taken >> 32,
            'index' => $taken & 0xFFFF_FFFF,
            ...(self::$identity ??= self::identity()),
        ], 200, ['Cache-Control' => 'no-store']);
    }

    /**
     * Perform `n` claims in a plain loop, one Orbital add each, and answer
     * what they took, in the shape of `/_lumenize/rust/burst`.
     *
     * The loop checks as it goes that every claim is in the first claim's
     * round and takes a greater index than the one before; within a round
     * that is a greater value. The check is one condition that a correct run
     * never meets, beside the add, which is a method call into the engine.
     */
    public function burst(Request $request): JsonResponse
    {
        $n = filter_var($request->query('n'), FILTER_VALIDATE_INT, [
            'options' => ['min_range' => 1, 'max_range' => self::BURST_MAX],
        ]);
        if ($n === false) {
            return response()->json(['error' => 'n must be a whole number from 1 to '.self::BURST_MAX], 400);
        }
        $id = $request->query('id');
        if (! is_string($id) || $id === '') {
            return response()->json(['error' => 'id names the Orbital to claim from'], 400);
        }
        if ($id !== self::$orbitalId) {
            try {
                self::$orbital = Orbital::open($id);
            } catch (\Exception $exception) {
                return response()->json(['error' => $exception->getMessage()], 400);
            }
            self::$orbitalId = $id;
        }
        $orbital = self::$orbital;

        // `add` answers the value after the add; the claim is the one before.
        $first = $orbital->add(1) - 1;
        $round = $first >> 32;
        $previous = $first;
        $sum = $first & 0xFFFF_FFFF;
        $done = 1;
        $fault = null;
        while ($done < $n) {
            $taken = $orbital->add(1) - 1;
            $sum += $taken & 0xFFFF_FFFF;
            $done++;
            if ($taken <= $previous || ($taken >> 32) !== $round) {
                $fault = [
                    'check' => ($taken >> 32) === $round ? 'order' : 'round',
                    'previous' => ['round' => $previous >> 32, 'index' => $previous & 0xFFFF_FFFF],
                    'taken' => ['round' => $taken >> 32, 'index' => $taken & 0xFFFF_FFFF],
                ];
                $previous = $taken;
                break;
            }
            $previous = $taken;
        }

        return response()->json([
            'side' => 'php',
            'round' => $round,
            'count' => $done,
            'first' => $first & 0xFFFF_FFFF,
            'last' => $previous & 0xFFFF_FFFF,
            'sum' => (string) $sum,
            ...(self::$identity ??= self::identity()),
            'fault' => $fault,
        ], 200, ['Cache-Control' => 'no-store']);
    }

    /**
     * What the runtime reports about this instance: the process, its
     * supervisor worker number (0 when standalone), and the PHP instance.
     *
     * @return array{pid: int, worker: int|null, execution: string|null}
     */
    private static function identity(): array
    {
        $worker = (int) ($_SERVER['CLAVIRON_WORKER_ID'] ?? 0);
        $instance = $_SERVER['CLAVIRON_PHP_INSTANCE'] ?? null;

        return [
            'pid' => (int) ($_SERVER['CLAVIRON_PROCESS_ID'] ?? getmypid()),
            'worker' => $worker > 0 ? $worker : null,
            'execution' => $instance === null ? null : "instance {$instance}",
        ];
    }
}
