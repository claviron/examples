<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="csrf-token" content="{{ csrf_token() }}">
    <title>Shared hits</title>
    <style>
        :root {
            color-scheme: light dark;
            --bg: #f7f7f5;
            --panel: #ffffff;
            --text: #1d1d1b;
            --muted: #6b6b66;
            --line: #deded8;
            --accent: #1f5fbf;
            --good: #1d7a3a;
            --bad: #b3261e;
        }
        @media (prefers-color-scheme: dark) {
            :root {
                --bg: #161615;
                --panel: #1f1f1d;
                --text: #ececea;
                --muted: #9a9a94;
                --line: #34342f;
                --accent: #7aa7ec;
                --good: #6cc68a;
                --bad: #f08a82;
            }
        }
        * { box-sizing: border-box; }
        body {
            margin: 0;
            background: var(--bg);
            color: var(--text);
            font: 16px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif;
        }
        main { max-width: 44rem; margin: 0 auto; padding: 2rem 1rem 3rem; }
        h1 { font-size: 1.5rem; margin: 0 0 .5rem; }
        p { margin: 0 0 1rem; }
        code { font: .9em ui-monospace, SFMono-Regular, Menlo, monospace; }
        a { color: var(--accent); }
        .panel {
            background: var(--panel);
            border: 1px solid var(--line);
            border-radius: .5rem;
            padding: 1.25rem;
            margin-bottom: 1rem;
        }
        .current { display: flex; align-items: baseline; gap: .75rem; }
        .current output {
            font: 600 2.5rem/1 ui-monospace, SFMono-Regular, Menlo, monospace;
            font-variant-numeric: tabular-nums;
        }
        .muted { color: var(--muted); }
        form { display: flex; flex-wrap: wrap; align-items: center; gap: .75rem; }
        label { display: flex; align-items: center; gap: .5rem; }
        input[type=number] {
            width: 7rem;
            padding: .4rem .5rem;
            font: inherit;
            color: inherit;
            background: transparent;
            border: 1px solid var(--line);
            border-radius: .375rem;
        }
        button {
            padding: .45rem 1rem;
            font: inherit;
            border-radius: .375rem;
            border: 1px solid var(--accent);
            background: var(--accent);
            color: var(--panel);
            cursor: pointer;
        }
        button.secondary { background: transparent; color: var(--accent); }
        button:disabled { opacity: .5; cursor: default; }
        table { width: 100%; border-collapse: collapse; font-variant-numeric: tabular-nums; }
        th, td { padding: .4rem 0; border-bottom: 1px solid var(--line); text-align: left; }
        td { text-align: right; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
        tr:last-child th, tr:last-child td { border-bottom: 0; }
        .verdict { font-weight: 600; }
        .verdict.good { color: var(--good); }
        .verdict.bad { color: var(--bad); }
        #status { min-height: 1.5em; margin: .75rem 0 0; }
    </style>
</head>
<body>
<main>
    <h1>One counter, two languages</h1>
    <p>
        This page proves that Rust and PHP add to one shared value without losing each other's updates:
        a run sends the same number of requests to <code>/_lumenize/rust</code>, where Rust adds one with
        <code>fetch_add</code> on the declared Orbital <code>hits</code>, and to <code>/_lumenize/php</code>,
        where Laravel adds one with <code>orbit_counter_inc('hits', 1)</code>, all at once.
    </p>

    <section class="panel">
        <div class="current">
            <output id="current">{{ $hits ?? 'unavailable' }}</output>
            <span class="muted">current value of <code>hits</code></span>
        </div>
    </section>

    <section class="panel">
        <form id="run-form">
            <label>Requests to each side <input id="count" type="number" min="1" step="1" value="200" required></label>
            <button id="run" type="submit">Run</button>
            <button id="reset" class="secondary" type="button">Reset</button>
        </form>
        <p id="status" class="muted" role="status"></p>
    </section>

    <section class="panel">
        <table>
            <tbody>
            <tr><th>Sent to Rust</th><td id="sent-rust">–</td></tr>
            <tr><th>Sent to PHP</th><td id="sent-php">–</td></tr>
            <tr><th>Answered by Rust</th><td id="ok-rust">–</td></tr>
            <tr><th>Answered by PHP</th><td id="ok-php">–</td></tr>
            <tr><th>Value before</th><td id="before">–</td></tr>
            <tr><th>Value after, read by Rust</th><td id="after-rust">–</td></tr>
            <tr><th>Value after, read by PHP</th><td id="after-php">–</td></tr>
            <tr><th>Expected (before + both)</th><td id="expected">–</td></tr>
            <tr><th>Distinct values returned</th><td id="distinct">–</td></tr>
            <tr><th>Match</th><td id="match" class="verdict">–</td></tr>
            </tbody>
        </table>
    </section>

    <p class="muted">
        Expected counts only answered requests. Another client adding to <code>hits</code> during a run
        makes the values differ without any update being lost. <a href="{{ route('welcome') }}">Back</a>
    </p>
</main>

<script>
(() => {
    const rust = '/_lumenize/rust';
    const php = '/_lumenize/php';
    const token = document.querySelector('meta[name="csrf-token"]').content;
    const $ = (id) => document.getElementById(id);
    const buttons = [$('run'), $('reset')];

    // Both sides answer "hits=<value>".
    async function request(url, init) {
        const response = await fetch(url, { cache: 'no-store', ...init });
        const body = await response.text();
        const match = /^hits=(-?\d+)$/m.exec(body);
        if (!response.ok || match === null) {
            throw new Error(`${url}: ${response.status} ${body.trim()}`);
        }
        return Number(match[1]);
    }

    function show(value) {
        $('current').textContent = String(value);
    }

    function fill(values) {
        for (const [id, value] of Object.entries(values)) {
            $(id).textContent = String(value);
        }
    }

    function busy(on, text) {
        buttons.forEach((button) => { button.disabled = on; });
        $('status').textContent = text;
    }

    async function run(count) {
        busy(true, `Sending ${count} requests to each side…`);
        const verdict = $('match');
        verdict.className = 'verdict';
        fill({
            'sent-rust': count, 'sent-php': count, 'ok-rust': '…', 'ok-php': '…',
            before: '…', 'after-rust': '…', 'after-php': '…', expected: '…', distinct: '…', match: '…',
        });

        const before = await request(`${rust}/value`);
        fill({ before });
        show(before);

        // Interleaved so neither side starts ahead of the other.
        let highest = before;
        const sides = [];
        for (let i = 0; i < count; i++) {
            sides.push(rust, php);
        }
        const results = await Promise.allSettled(sides.map((url) => request(url).then((value) => {
            if (value > highest) {
                highest = value;
                show(value);
            }
            return value;
        })));

        const answered = { [rust]: 0, [php]: 0 };
        const returned = new Set();
        let firstError = null;
        results.forEach((result, i) => {
            if (result.status === 'fulfilled') {
                answered[sides[i]] += 1;
                returned.add(result.value);
            } else if (firstError === null) {
                firstError = result.reason;
            }
        });

        const [afterRust, afterPhp] = await Promise.all([request(`${rust}/value`), request(`${php}/value`)]);
        show(afterRust);
        const expected = before + answered[rust] + answered[php];
        const ok = afterRust === expected && afterPhp === expected;
        fill({
            'ok-rust': answered[rust], 'ok-php': answered[php],
            'after-rust': afterRust, 'after-php': afterPhp, expected,
            distinct: `${returned.size} of ${answered[rust] + answered[php]}`,
            match: ok ? 'yes' : 'no',
        });
        verdict.className = `verdict ${ok ? 'good' : 'bad'}`;
        busy(false, firstError === null ? 'Done.' : `Done, with failures. First: ${firstError.message}`);
    }

    $('run-form').addEventListener('submit', (event) => {
        event.preventDefault();
        const count = Number($('count').value);
        if (!Number.isInteger(count) || count < 1) {
            $('status').textContent = 'Enter a whole number of at least 1.';
            return;
        }
        run(count).catch((error) => busy(false, error.message));
    });

    $('reset').addEventListener('click', () => {
        busy(true, 'Resetting…');
        request(php, { method: 'DELETE', headers: { 'X-CSRF-TOKEN': token } })
            .then((value) => { show(value); busy(false, `Reset; hits=${value}.`); })
            .catch((error) => busy(false, error.message));
    });
})();
</script>
</body>
</html>
