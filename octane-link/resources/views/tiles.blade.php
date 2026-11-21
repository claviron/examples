<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Orbit atomic</title>
    <style>
        :root {
            color-scheme: light dark;
            --bg: #f6f5f2;
            --panel: #ffffff;
            --text: #1c1c1a;
            --muted: #686862;
            --line: #e0dfd9;
            --accent: #1f5fbf;
            --empty: #e9e8e3;
            --good: #1d7a3a;
            --bad: #c0281f;
            /* Hue and lightness of the two families; the canvas reads these. */
            --rust-hue: 22;
            --php-hue: 236;
            --tile-light: 36;
        }
        @media (prefers-color-scheme: dark) {
            :root {
                --bg: #151514;
                --panel: #1e1e1c;
                --text: #ecebe7;
                --muted: #9b9a93;
                --line: #33332f;
                --accent: #7fa9ee;
                --empty: #2a2a27;
                --good: #6cc68a;
                --bad: #f0645a;
                --tile-light: 48;
            }
        }
        * { box-sizing: border-box; }
        body {
            margin: 0;
            background: var(--bg);
            color: var(--text);
            font: 14px/1.4 system-ui, -apple-system, "Segoe UI", sans-serif;
        }
        /* One screen: the page is as tall as the window and nothing scrolls. */
        main {
            max-width: 64rem; margin: 0 auto; padding: .8rem 1rem .6rem;
            display: grid; grid-template-rows: auto minmax(0, 1fr) auto; gap: .6rem;
            height: 100dvh;
        }
        header { display: flex; flex-wrap: wrap; align-items: baseline; gap: .2rem .9rem; }
        h1 { font-size: 1.25rem; margin: 0; letter-spacing: -.01em; }
        header p { margin: 0; color: var(--muted); }
        h2 {
            font-size: .7rem; font-weight: 600; letter-spacing: .06em; text-transform: uppercase;
            color: var(--muted); margin: 0 0 .3rem;
        }
        p { margin: 0 0 .5rem; }
        code { font: .88em ui-monospace, SFMono-Regular, Menlo, monospace; }
        a { color: var(--accent); }
        .muted { color: var(--muted); }
        .layout { display: grid; gap: .7rem; grid-template-columns: minmax(0, 1fr) 21rem; min-height: 0; }
        .panel {
            background: var(--panel);
            border: 1px solid var(--line);
            border-radius: .6rem;
            padding: .7rem .8rem;
        }
        .canvas-panel { display: grid; grid-template-rows: minmax(0, 1fr) auto; justify-items: center; align-items: center; gap: .4rem; min-height: 0; }
        canvas {
            display: block;
            width: min(100%, calc(100dvh - 8.6rem));
            aspect-ratio: 1;
            border-radius: .25rem;
        }
        .progress { width: 100%; display: flex; justify-content: space-between; gap: 1rem; font-size: .85rem; font-variant-numeric: tabular-nums; }
        .side { display: grid; gap: .6rem; align-content: start; min-height: 0; overflow: auto; }
        form { display: grid; grid-template-columns: minmax(0, 1fr) auto auto; gap: .45rem; align-items: end; }
        .more { grid-column: 1 / -1; font-size: .8rem; color: var(--muted); }
        .more summary { cursor: pointer; }
        .options { display: grid; grid-template-columns: 1fr 1fr; gap: .45rem; margin-top: .35rem; }
        .sides { display: grid; grid-template-columns: 1fr 1fr; gap: .5rem; margin-bottom: .35rem; }
        .count { display: grid; padding: .35rem .6rem; border-radius: .45rem; background: var(--bg); border-left: .3rem solid; }
        .count span { font: 600 1.5rem/1.15 ui-monospace, SFMono-Regular, Menlo, monospace; font-variant-numeric: tabular-nums; }
        .count small { font-size: .75rem; color: var(--muted); }
        .count.rust { border-color: hsl(var(--rust-hue) 70% 50%); }
        .count.php { border-color: hsl(var(--php-hue) 62% 56%); }
        [hidden] { display: none !important; }
        label { display: grid; gap: .15rem; font-size: .75rem; color: var(--muted); }
        select {
            font: inherit; font-size: .9rem; color: var(--text);
            background: var(--bg); border: 1px solid var(--line); border-radius: .4rem;
            padding: .25rem .35rem;
        }
        button {
            padding: .32rem 1rem; font: inherit; font-weight: 600;
            border-radius: .4rem; border: 1px solid var(--accent);
            background: var(--accent); color: var(--panel); cursor: pointer;
        }
        button.secondary { background: transparent; color: var(--accent); }
        button:disabled { opacity: .45; cursor: default; }
        button.small { margin-top: .45rem; padding: .2rem .6rem; font-size: .8rem; font-weight: 500; }
        dl { display: grid; grid-template-columns: 1fr auto; margin: 0; }
        dt, dd { padding: .16rem 0; border-bottom: 1px solid var(--line); }
        dd { margin: 0; text-align: right; font: .9rem ui-monospace, SFMono-Regular, Menlo, monospace; font-variant-numeric: tabular-nums; }
        dl > :nth-last-child(-n + 2), dl > :not([hidden]):has(+ [hidden]), dl > dd:not([hidden]):has(+ dt[hidden]) { border-bottom: 0; }
        dd.bad { color: var(--bad); font-weight: 600; }
        dd.good { color: var(--good); font-weight: 600; }
        /* The counts the verdict rests on, outlined once the run has ended. */
        .evidence { margin-top: .55rem; padding: .1rem .6rem .2rem; border: 2px solid var(--line); border-radius: .5rem; }
        .evidence h3 { margin: .3rem 0 .05rem; font-size: .72rem; font-weight: 600; letter-spacing: .05em; text-transform: uppercase; color: var(--muted); }
        .evidence.good { border-color: var(--good); }
        .evidence.good h3 { color: var(--good); }
        .evidence.bad { border-color: var(--bad); }
        .evidence.bad h3 { color: var(--bad); }
        .verdict { margin: .5rem 0 0; padding: .4rem .6rem; border-radius: .4rem; background: var(--bg); font-size: .9rem; min-height: 2.4rem; }
        .verdict.good { color: var(--good); font-weight: 600; }
        .verdict.bad { color: var(--bad); font-weight: 600; }
        .legend { display: flex; flex-wrap: wrap; gap: .25rem .8rem; font-size: .78rem; color: var(--muted); }
        .key { display: flex; align-items: center; gap: .35rem; }
        .swatch { width: .8rem; height: .8rem; border-radius: .2rem; flex: none; }
        .ramp { width: 2.2rem; }
        .ramp.rust { background: linear-gradient(90deg, hsl(calc(var(--rust-hue) - 10) 70% 40%), hsl(calc(var(--rust-hue) + 26) 70% 62%)); }
        .ramp.php { background: linear-gradient(90deg, hsl(calc(var(--php-hue) - 10) 62% 40%), hsl(calc(var(--php-hue) + 26) 62% 66%)); }
        .swatch.empty { background: var(--empty); outline: 1px solid var(--line); }
        .swatch.bad { background: var(--bad); }
        .swatch.missing { background: var(--empty); box-shadow: inset 0 0 0 2px var(--bad); }
        .servers { display: grid; gap: .35rem; margin-top: .45rem; }
        .servers:empty { display: none; }
        .process { display: grid; gap: .2rem; }
        .process-name { font-size: .78rem; font-weight: 600; }
        .chips { display: flex; flex-wrap: wrap; gap: .2rem; }
        .chip {
            display: inline-flex; align-items: center; gap: .25rem; padding: 0 .35rem;
            border: 1px solid var(--line); border-radius: 1rem;
            font: .7rem ui-monospace, SFMono-Regular, Menlo, monospace; font-variant-numeric: tabular-nums;
        }
        .chips-label { flex-basis: 100%; font-size: .72rem; color: var(--muted); }
        .chip .swatch { width: .55rem; height: .55rem; border-radius: 50%; }
        footer { font-size: .8rem; }
        footer summary { cursor: pointer; display: inline; }
        footer details[open] { max-width: 52rem; padding-bottom: .5rem; }
        footer details p { margin: .4rem 0 0; }
        /* A narrow window stacks the two columns and scrolls like any page. */
        @media (max-width: 52rem) {
            main { height: auto; }
            .layout { grid-template-columns: minmax(0, 1fr); }
            canvas { width: 100%; }
            .side { overflow: visible; }
        }
    </style>
</head>
<body>
<main>
    <header>
        <h1>One Orbit atomic, two languages</h1>
        <p>Rust and PHP take tiles from one shared value. Every tile exactly once is the proof.</p>
    </header>

    <div class="layout">
        <section class="panel canvas-panel" aria-label="Tiles">
            <canvas id="canvas" role="img" aria-label="The tiles of the current round"></canvas>
            <div class="progress muted"><span id="progress">No run yet.</span><span id="rate"></span></div>
        </section>

        <div class="side">
            <section class="panel">
                <form id="form">
                    <label>Tiles
                        <select id="size">
                            <option value="256">256</option>
                            <option value="1024" selected>1,024</option>
                            <option value="4096">4,096</option>
                        </select>
                    </label>
                    <button id="run" type="submit">Run</button>
                    <button id="stop" class="secondary" type="button" disabled>Stop</button>
                    <details class="more">
                        <summary>Options</summary>
                        <div class="options">
                            <label>In flight
                                <select id="concurrency">
                                    <option>4</option>
                                    <option>8</option>
                                    <option selected>16</option>
                                    <option>32</option>
                                    <option>64</option>
                                </select>
                            </label>
                            <label>Routes
                                <select id="sides">
                                    <option value="free" selected>Both, free</option>
                                    <option value="random">Both, at random</option>
                                    <option value="both">Both, in turn</option>
                                    <option value="rust">Rust only</option>
                                    <option value="php">PHP only</option>
                                </select>
                            </label>
                            <label>Pace
                                <select id="pace">
                                    <option value="full" selected>Full speed</option>
                                    <option value="steps">In steps</option>
                                </select>
                            </label>
                        </div>
                        <button id="fault" class="secondary small" type="button" disabled>Fake a mistake</button>
                    </details>
                </form>
            </section>

            <section class="panel">
                <div class="sides">
                    <div class="count rust"><span id="rust">–</span><small>Rust</small></div>
                    <div class="count php"><span id="php">–</span><small>PHP</small></div>
                </div>
                <dl>
                    <dt hidden>Refused, retried</dt><dd id="refused" hidden>–</dd>
                </dl>
                <div id="evidence" class="evidence">
                    <h3 id="evidence-title">Evidence</h3>
                    <dl>
                        <dt>Duplicates</dt><dd id="duplicates">–</dd>
                        <dt>Still empty</dt><dd id="empty">–</dd>
                        <dt hidden>Outside the range</dt><dd id="outside" hidden>–</dd>
                        <dt hidden>Failed</dt><dd id="failed" hidden>–</dd>
                        <dt hidden>From another round</dt><dd id="stale" hidden>–</dd>
                    </dl>
                </div>
                <p id="verdict" class="verdict muted" role="status">Run claims every tile once and checks the picture.</p>
            </section>

            <section class="panel">
                <details class="more">
                    <summary id="served">Who served</summary>
                    <div id="servers" class="servers"></div>
                </details>
            </section>
        </div>
    </div>

    <footer class="muted">
        <details>
            <summary>What this shows</summary>
            <p>
                Each tile is one claim. A request to Rust (<code>/_lumenize/rust/tiles</code>) or to Laravel
                (<code>/_lumenize/php/tiles</code>) takes the next index from the Orbit atomic <code>tiles</code> with
                one atomic add, and its answer paints that tile. Because the add is atomic across threads and
                processes, no tile is painted twice and none is left empty.
            </p>
            <p>
                PHP can only take tile 41 after Rust took tile 40 by reading the value Rust had just written. A
                change one side missed would show as a duplicate.
            </p>
            <p>
A standalone launch is one process, and with <code>--workers</code> macOS gives every connection on one of this application's
                TCP listeners to the same worker. A refused claim (503) took nothing and is sent again; a failed one
                is counted and not sent again, since it may have taken a tile.
            </p>
        </details>
        · <a href="https://github.com/claviron/examples/tree/main/octane-link#one-orbit-atomic-two-languages">Source and how it works</a>
        · <a href="{{ route('hits') }}">One counter, two languages</a> · <a href="{{ route('welcome') }}">Back</a>
    </footer>
</main>

<script>
(() => {
    const routes = { rust: '/_lumenize/rust/tiles', php: '/_lumenize/php/tiles' };
    const $ = (id) => document.getElementById(id);
    const canvas = $('canvas');
    const context = canvas.getContext('2d');
    const format = new Intl.NumberFormat('en');
    const dark = matchMedia('(prefers-color-scheme: dark)');

    let run = null;

    function blankRun(size) {
        return {
            size,
            side: Math.round(Math.sqrt(size)),
            round: null,
            // Per tile: the index of the execution that claimed it, or -1.
            owner: new Int32Array(size).fill(-1),
            duplicate: new Uint8Array(size),
            executions: [],          // { side, pid, worker, execution, count, process }
            executionIndex: new Map(),
            processes: new Map(),    // pid -> { ordinal, worker }
            claimed: { rust: 0, php: 0 },
            painted: 0, duplicates: 0, outside: 0, refused: 0, failed: 0, stale: 0,
            inFlight: 0, settled: 0, sent: 0, flying: { rust: 0, php: 0 },
            // Pace: the claims the current step still allows, and when the
            // next step starts.
            pace: 'full', allowance: 0, nextBeat: 0,
            started: performance.now(), ended: null, stopping: false, done: false,
        };
    }

    // One flat colour for each language. Shades per thread or instance read
    // as "painted more than once", which is the one thing that never happens.
    function colour(execution) {
        return execution.side === 'rust'
            ? `hsl(${palette.rust} 70% ${palette.light + 12}%)`
            : `hsl(${palette.php} 62% ${palette.light + 16}%)`;
    }

    // The theme's colours, read again whenever the whole canvas is repainted.
    let palette = null;
    function readPalette() {
        const style = getComputedStyle(document.documentElement);
        const read = (name) => style.getPropertyValue(name).trim();
        palette = {
            panel: read('--panel'), empty: read('--empty'), bad: read('--bad'),
            rust: Number(read('--rust-hue')), php: Number(read('--php-hue')), light: Number(read('--tile-light')),
        };
    }

    let geometry = null;

    function layout() {
        const box = canvas.getBoundingClientRect();
        const ratio = window.devicePixelRatio || 1;
        const pixels = Math.max(1, Math.round(box.width * ratio));
        canvas.width = pixels;
        canvas.height = pixels;
        const side = run ? run.side : Math.round(Math.sqrt(Number($('size').value)));
        const cell = pixels / side;
        geometry = { side, cell, gap: cell >= 6 * ratio ? Math.max(1, Math.round(ratio)) : 0 };
        repaint();
    }

    function tileRect(index) {
        const { side, cell, gap } = geometry;
        const x = Math.round((index % side) * cell);
        const y = Math.round(Math.floor(index / side) * cell);
        const w = Math.round(((index % side) + 1) * cell) - x - gap;
        const h = Math.round((Math.floor(index / side) + 1) * cell) - y - gap;
        return [x, y, Math.max(1, w), Math.max(1, h)];
    }

    function paint(index) {
        const [x, y, w, h] = tileRect(index);
        if (run && run.duplicate[index]) {
            context.fillStyle = palette.bad;
            context.fillRect(x, y, w, h);
            return;
        }
        const owner = run ? run.owner[index] : -1;
        context.fillStyle = owner < 0 ? palette.empty : run.executions[owner].colour;
        context.fillRect(x, y, w, h);
        if (owner < 0 && run && run.done) {
            const inset = Math.max(1, Math.round(Math.min(w, h) / 5));
            context.strokeStyle = palette.bad;
            context.lineWidth = inset;
            context.strokeRect(x + inset / 2, y + inset / 2, w - inset, h - inset);
        }
    }

    function repaint() {
        if (!geometry) return;
        readPalette();
        context.fillStyle = palette.panel;
        context.fillRect(0, 0, canvas.width, canvas.height);
        const count = run ? run.size : geometry.side * geometry.side;
        if (run) run.executions.forEach((execution) => { execution.colour = colour(execution); });
        for (let i = 0; i < count; i++) paint(i);
    }

    // Stats are drawn once per frame at most, however many answers arrive.
    let scheduled = false;
    function schedule() {
        if (scheduled) return;
        scheduled = true;
        requestAnimationFrame(() => { scheduled = false; render(); });
    }

    function set(id, value, bad = false) {
        const element = $(id);
        element.textContent = typeof value === 'number' ? format.format(value) : value;
        element.classList.toggle('bad', bad);
    }

    // When the run has ended, each count the verdict rests on is checked or
    // crossed, and the group is outlined in the verdict's colour.
    const evidenceIds = ['duplicates', 'empty', 'outside', 'failed', 'stale'];

    function evidence(current) {
        const box = $('evidence');
        const title = $('evidence-title');
        if (!current.done) {
            box.className = 'evidence';
            title.textContent = 'Evidence';
            evidenceIds.forEach((id) => $(id).classList.remove('good'));
            return;
        }
        const counts = {
            duplicates: current.duplicates, empty: current.size - current.painted, outside: current.outside,
            failed: current.failed, stale: current.stale,
        };
        let verified = !current.stopping;
        for (const id of evidenceIds) {
            const element = $(id);
            const clean = counts[id] === 0;
            verified = verified && clean;
            element.textContent = `${format.format(counts[id])} ${clean ? '✓' : '✗'}`;
            element.classList.toggle('good', clean);
            element.classList.toggle('bad', !clean);
        }
        box.className = `evidence ${verified ? 'good' : 'bad'}`;
        title.textContent = verified ? 'Evidence: verified ✓' : 'Evidence: not verified ✗';
    }

    function show(id, visible) {
        const value = $(id);
        value.hidden = !visible;
        value.previousElementSibling.hidden = !visible;
    }

    function render() {
        if (!run) return;
        const empty = run.size - run.painted;
        set('rust', run.claimed.rust);
        set('php', run.claimed.php);
        set('duplicates', run.duplicates, run.duplicates > 0);
        set('outside', run.outside, run.outside > 0);
        set('empty', empty, run.done && empty > 0);
        set('refused', run.refused);
        set('failed', run.failed, run.failed > 0);
        set('stale', run.stale, run.stale > 0);
        // Rows that only matter when something went wrong stay out of sight.
        show('refused', run.refused > 0);
        show('outside', run.outside > 0);
        show('failed', run.failed > 0);
        show('stale', run.stale > 0);
        $('served').textContent = `Who served: ${plural(run.processes.size, 'process', 'processes')}, `
            + `${plural(run.executions.length, 'execution', 'executions')}`;
        evidence(run);
        const seconds = ((run.ended ?? performance.now()) - run.started) / 1000;
        $('progress').textContent = `Round ${run.round ?? '…'}: ${format.format(run.painted)} of ${format.format(run.size)} tiles`;
        $('rate').textContent = seconds > 0 ? `${format.format(Math.round(run.settled / seconds))} claims/s` : '';
        renderServers();
    }

    function renderServers() {
        const servers = $('servers');
        servers.replaceChildren();
        const byProcess = new Map();
        run.executions.forEach((execution) => {
            if (!byProcess.has(execution.pid)) byProcess.set(execution.pid, []);
            byProcess.get(execution.pid).push(execution);
        });
        [...run.processes.entries()].sort((a, b) => a[1].ordinal - b[1].ordinal).forEach(([pid, process]) => {
            const block = document.createElement('div');
            block.className = 'process';
            const name = document.createElement('div');
            name.className = 'process-name';
            name.textContent = `Process ${pid}` + (process.worker === null ? ', standalone' : `, worker ${process.worker}`);
            block.append(name);
            for (const [side, title] of [['rust', 'Rust, by Tokio thread'], ['php', 'PHP, by instance']]) {
                const executions = byProcess.get(pid).filter((e) => e.side === side).sort((a, b) => a.number - b.number);
                if (executions.length === 0) continue;
                const chips = document.createElement('div');
                chips.className = 'chips';
                const label = document.createElement('span');
                label.className = 'chips-label';
                label.textContent = title;
                chips.append(label);
                executions.forEach((execution) => {
                    const chip = document.createElement('span');
                    chip.className = 'chip';
                    chip.title = `${side} ${execution.execution ?? ''}: ${execution.count} tiles`;
                    const dot = document.createElement('span');
                    dot.className = 'swatch';
                    dot.style.background = execution.colour;
                    chip.append(dot, `${execution.number} · ${format.format(execution.count)}`);
                    chips.append(chip);
                });
                block.append(chips);
            }
            servers.append(block);
        });
    }

    function executionOf(answer) {
        const key = `${answer.side}|${answer.pid}|${answer.execution}`;
        let index = run.executionIndex.get(key);
        if (index !== undefined) return index;
        let process = run.processes.get(answer.pid);
        if (!process) {
            process = { ordinal: run.processes.size, worker: answer.worker };
            run.processes.set(answer.pid, process);
        }
        // The number the runtime gives the execution (a thread or an instance),
        // so a shade stays with it from one run to the next.
        const digits = /\d+/.exec(answer.execution ?? '');
        const execution = {
            side: answer.side, pid: answer.pid, execution: answer.execution, count: 0, process,
            number: digits ? Number(digits[0]) : run.executions.length,
        };
        execution.colour = colour(execution);
        index = run.executions.length;
        run.executions.push(execution);
        run.executionIndex.set(key, index);
        return index;
    }

    function accept(answer) {
        if (answer.round !== run.round) {
            run.stale++;
            return;
        }
        const index = answer.index;
        if (!Number.isInteger(index) || index < 0 || index >= run.size) {
            run.outside++;
            return;
        }
        const execution = executionOf(answer);
        run.executions[execution].count++;
        run.claimed[answer.side]++;
        if (run.owner[index] >= 0 || run.duplicate[index]) {
            run.duplicates++;
            run.duplicate[index] = 1;
        } else {
            run.owner[index] = execution;
            run.painted++;
        }
        paint(index);
    }

    // "At random" tosses a coin for every claim: about half each, in no order.
    // "In turn" sends one to Rust, one to PHP, so each side takes half.
    // "Free" gives each side half of the requests in flight and refills
    // whichever answered: the faster side claims more, and the split is the
    // ratio of the two request paths, not a property of the shared value.
    function nextSide(choice) {
        if (choice === 'random') return Math.random() < 0.5 ? 'rust' : 'php';
        if (choice === 'both') return run.sent % 2 === 0 ? 'rust' : 'php';
        if (choice === 'free') return run.flying.rust <= run.flying.php ? 'rust' : 'php';
        return choice;
    }

    // One claim. A 503 took nothing, so it frees its place and is sent again.
    async function claim(current, side) {
        let response;
        let answer = null;
        try {
            response = await fetch(routes[side], { cache: 'no-store' });
            if (response.ok) answer = await response.json();
        } catch (error) {
            response = null;
        }
        if (current !== run) return;
        current.inFlight--;
        current.flying[side]--;
        if (response && response.status === 503) {
            current.refused++;
        } else {
            current.settled++;
            if (answer && answer.side === side) accept(answer);
            else current.failed++;
        }
        launch();
        schedule();
    }

    // "In steps" lets one burst of claims go on each step instead of running
    // flat out, so a run can be watched (and set to music afterwards).
    const stepSeconds = 0.36;
    const clock = () => performance.now() / 1000;

    function tick(current) {
        if (current !== run || current.done) return;
        while (clock() >= current.nextBeat) {
            current.nextBeat += stepSeconds;
            current.allowance = Number($('concurrency').value);
            launch();
        }
        requestAnimationFrame(() => tick(current));
    }

    // What a mistake looks like, shown on purpose: the page takes
    // one answer it already painted and counts it again, as it would have to
    // if the shared value had handed one tile out twice. Nothing is sent.
    function fault() {
        if (!run || run.done || run.painted === 0) return;
        const index = run.owner.findIndex((owner) => owner >= 0);
        const execution = run.executions[run.owner[index]];
        run.injected = true;
        accept({ side: execution.side, round: run.round, index, pid: execution.pid, execution: execution.execution });
        schedule();
    }

    function launch() {
        const current = run;
        const limit = Number($('concurrency').value);
        const choice = $('sides').value;
        // In steps, each step lets one burst of claims go; see `tick`.
        const paced = current.pace === 'steps';
        while (!current.stopping && current.inFlight < limit && current.settled + current.inFlight < current.size
            && (!paced || current.allowance > 0)) {
            const side = nextSide(choice);
            if (paced) current.allowance--;
            current.sent++;
            current.inFlight++;
            current.flying[side]++;
            claim(current, side);
        }
        if (current.inFlight === 0 && (current.stopping || current.settled >= current.size)) finish(current);
    }

    function finish(current) {
        if (current.done) return;
        current.done = true;
        current.ended = performance.now();
        repaint();
        render();
        busy(false);
        verdict(current);
    }

    function verdict(current) {
        const element = $('verdict');
        const empty = current.size - current.painted;
        const n = format.format(current.size);
        const exact = !current.stopping && empty === 0 && current.duplicates === 0 && current.outside === 0
            && current.stale === 0 && current.failed === 0;
        if (exact) {
            element.className = 'verdict good';
            element.textContent = `All ${n} tiles claimed exactly once: `
                + `${format.format(current.claimed.rust)} by Rust, ${format.format(current.claimed.php)} by PHP.`;
            return;
        }
        const wrong = [];
        if (current.stopping) wrong.push('the run was stopped');
        if (current.duplicates) wrong.push(`${plural(current.duplicates, 'tile was', 'tiles were')} claimed more than once`
            + (current.injected ? ' (a mistake faked from this page, not one the server made)' : ''));
        if (empty) wrong.push(`${plural(empty, 'tile is', 'tiles are')} still empty`);
        if (current.outside) wrong.push(`${plural(current.outside, 'answer', 'answers')} named a tile outside 0–${current.size - 1}, so another client was claiming in this round`);
        if (current.stale) wrong.push(`${plural(current.stale, 'answer', 'answers')} came from another round, so another client started a round during this one`);
        if (current.failed) wrong.push(`${plural(current.failed, 'claim', 'claims')} failed, and each may have taken a tile`);
        element.className = 'verdict bad';
        element.textContent = `Not exact: ${wrong.join('; ')}.`;
    }

    function plural(count, one, many) {
        return `${format.format(count)} ${count === 1 ? one : many}`;
    }

    function busy(on) {
        $('run').disabled = on;
        $('stop').disabled = !on;
        $('fault').disabled = !on;
        ['size', 'sides', 'pace'].forEach((id) => { $(id).disabled = on; });
    }

    // A run is a round of its own: the reset starts the next round and says
    // which, and every claim answers with the round it was taken in.
    async function start() {
        const size = Number($('size').value);
        run = blankRun(size);
        layout();
        busy(true);
        $('verdict').className = 'verdict muted';
        $('verdict').textContent = 'Starting a new round…';
        const current = run;
        try {
            const response = await fetch(routes.rust, { method: 'DELETE', cache: 'no-store' });
            if (!response.ok) throw new Error(`the reset answered ${response.status}`);
            current.round = (await response.json()).round;
        } catch (error) {
            current.done = true;
            busy(false);
            $('verdict').className = 'verdict bad';
            $('verdict').textContent = `No round was started: ${error.message}.`;
            return;
        }
        if (current !== run) return;
        current.started = performance.now();
        $('verdict').textContent = 'Claiming…';
        current.pace = $('pace').value;
        current.nextBeat = clock() + 0.05;
        launch();
        schedule();
        if (current.pace === 'steps') requestAnimationFrame(() => tick(current));
    }

    $('form').addEventListener('submit', (event) => {
        event.preventDefault();
        start();
    });
    $('fault').addEventListener('click', fault);
    $('stop').addEventListener('click', () => {
        if (!run || run.done) return;
        run.stopping = true;
        $('verdict').textContent = 'Stopping: waiting for the claims in flight…';
        launch();
    });
    $('size').addEventListener('change', () => {
        if (run && !run.done) return;
        run = null;
        layout();
    });

    new ResizeObserver(layout).observe(canvas);
    dark.addEventListener('change', () => { repaint(); if (run) render(); });
})();
</script>
</body>
</html>
