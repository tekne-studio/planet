// What plants cost the browser's frame: `bun run webshot --flora out/flora`
// stands where `bun run shot --flora out/flora` stood, the views, hour and
// sizes its flora.json says, and measures the same way in the page: each
// draw of foliage priced by what the frame saves without it, the CPU's time
// placing the foliage in WASM, and the counts, which match the desktop's
// when both stand in the same place. Writes web.json beside flora.json and
// prints the tables. With `--software` the browser draws on SwiftShader, a
// GPU the CPU stands in for, which shades every fragment it rasterizes; a
// frame takes half a second there, so it reads the high preset at the first
// size alone, fewer times, and writes software.json.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

/** What the desktop's measure stood at, read from its flora.json. */
interface Stood {
	views: Record<string, string>;
	clock_s: number;
	sizes: [number, number][];
}

/** One view at one preset and size, measured in the page. */
interface Row {
	view: string;
	preset: string;
	size: [number, number];
	frame_ms: number;
	saved_ms: Record<string, number>;
	saved_in_pass_ms: Record<string, number>;
	cpu_ms: { gather: number; sort: number; write: number };
	counted: { timed: boolean; counted: { forms: number[] }[] };
}

/** What the page is told: where to stand, at which presets, and how many readings to take. */
interface Asked extends Stood {
	presets: string[];
	module: string;
	rounds: number;
	block: number;
}

/**
 * Runs in the page, on its own engine and canvas: the page's own UI is
 * cleared away. Self-contained, as it is sent as source.
 */
async function inPage(asked: Asked) {
	const engineModule = await import(asked.module);
	await engineModule.default();
	const presets = JSON.parse(engineModule.presets());
	document.body.replaceChildren();
	const canvas = document.createElement('canvas');
	document.body.append(canvas);
	let settled = false;
	const engine = await engineModule.Engine.create(canvas, (json: string) => {
		if (JSON.parse(json).type === 'settled') settled = true;
	});
	const frame = () => new Promise((done) => requestAnimationFrame(done));
	const frames = async (count: number) => {
		for (let at = 0; at < count; at++) await frame();
	};
	const read = () => JSON.parse(engine.measured());
	const median = (values: number[]) =>
		[...values].sort((a, b) => a - b)[Math.floor(values.length / 2)] ?? 0;
	// The page's clock is coarse, a tenth of a millisecond: a mean of many
	// readings says what one cannot.
	const mean = (values: number[]) =>
		values.reduce((sum, value) => sum + value, 0) / Math.max(values.length, 1);
	// A draw left out: of the view, its forms near to far, and the cascades.
	const priced: [string, boolean, number[], number[]][] = [
		['view', true, [], []],
		['near', false, [], [0]],
		['mid', false, [], [1]],
		['far', false, [], [2]],
		['cascade 0', false, [0], []],
		['cascade 1', false, [1], []],
		['cascade 2', false, [2], []],
		['all', true, [0, 1, 2], []]
	];
	// No world is linked to say what is on: every plugin and life, as the
	// desktop's picture stands in for one.
	engine.rehearse();
	engine.measure();
	const rows = [];
	for (const [view, pose] of Object.entries(asked.views)) {
		for (const preset of asked.presets) {
			settled = false;
			engine.command(JSON.stringify({ type: 'set_effects', effects: presets[preset] }));
			engine.command(JSON.stringify({ type: 'go_to', place: pose }));
			engine.set_clock(asked.clock_s);
			for (let waited = 0; !settled && waited < 3600; waited++) await frame();
			for (const [width, height] of asked.sizes) {
				canvas.style.cssText = `position:fixed;left:0;top:0;width:${width}px;height:${height}px`;
				await frames(30);
				// The whole frame and each draw left out, a block of readings
				// each in turn: each round prices a draw by its block against
				// the round's whole, so a drift between rounds falls on both.
				const whole: number[] = [];
				const saved: number[][] = priced.map(() => []);
				// The same, by the passes each draw is in alone: the view's,
				// for a form too, a cascade's, or all of them for all foliage.
				const inPass: number[][] = priced.map(() => []);
				const passes = ['view', 'cascade 0', 'cascade 1', 'cascade 2'];
				const passOf = (label: string) => (passes.includes(label) ? label : 'view');
				const passMs = (reading: Record<string, number>, label: string) =>
					label === 'all'
						? passes.reduce((sum, pass) => sum + (reading[pass] ?? 0), 0)
						: (reading[passOf(label)] ?? 0);
				const spent = { gather: [] as number[], sort: [] as number[], write: [] as number[] };
				for (let round = 0; round < asked.rounds; round++) {
					const blocks: number[][] = [];
					const readings: Record<string, number>[][] = [];
					for (let at = -1; at < priced.length; at++) {
						const block: number[] = [];
						blocks.push(block);
						const read_: Record<string, number>[] = [];
						readings.push(read_);
						const [, view, cascades, forms] = at < 0 ? ['', false, [], []] : priced[at];
						engine.measure_without(view, new Uint8Array(cascades), new Uint8Array(forms));
						// A reading counts once it is of a frame drawn from now on.
						const from = read().drawn;
						let [seen, drawn, got] = [from, from, 0];
						for (let guard = 0; got < asked.block && guard < asked.block * 30; guard++) {
							await frame();
							const measured = read();
							if (at < 0 && measured.drawn > drawn) {
								drawn = measured.drawn;
								spent.gather.push(measured.spent_ms.gather);
								spent.sort.push(measured.spent_ms.sort);
								spent.write.push(measured.spent_ms.write);
							}
							if (measured.read > seen && measured.passes_ms.frame !== undefined) {
								seen = measured.read;
								block.push(measured.passes_ms.frame);
								read_.push(measured.passes_ms);
								got++;
							}
						}
					}
					const [roundWhole, ...left] = blocks.map(median);
					whole.push(...blocks[0]);
					left.forEach((ms, at) => saved[at].push(roundWhole - ms));
					priced.forEach(([label], at) => {
						const of = (block: Record<string, number>[]) =>
							median(block.map((reading) => passMs(reading, label)));
						inPass[at].push(of(readings[0]) - of(readings[at + 1]));
					});
				}
				engine.measure_without(false, new Uint8Array(), new Uint8Array());
				engine.count_foliage();
				await frames(30);
				const frameMs = median(whole);
				rows.push({
					view,
					preset,
					size: [width, height],
					frame_ms: frameMs,
					saved_ms: Object.fromEntries(priced.map(([label], at) => [label, median(saved[at])])),
					saved_in_pass_ms: Object.fromEntries(
						priced.map(([label], at) => [label, median(inPass[at])])
					),
					cpu_ms: { gather: mean(spent.gather), sort: mean(spent.sort), write: mean(spent.write) },
					counted: read()
				});
			}
		}
	}
	engine.free();
	const adapter = await (navigator as any).gpu?.requestAdapter();
	const info = adapter?.info ?? {};
	return {
		adapter: `${info.vendor ?? ''} ${info.architecture ?? ''} ${info.description ?? ''}`.trim(),
		rows
	};
}

/** Measures in the page `send` talks to, writes its JSON, prints the tables. */
export async function flora(
	send: (method: string, params?: object) => Promise<any>,
	dir: string,
	software: boolean
) {
	const native = join(dir, 'flora.json');
	let stood: Stood;
	try {
		stood = JSON.parse(readFileSync(native, 'utf8')) as Stood;
	} catch {
		throw new Error(
			`${native}: run \`bun run shot --flora ${dir}\` first; the browser stands where it stood`
		);
	}
	const asked: Asked = {
		views: stood.views,
		clock_s: stood.clock_s,
		presets: software ? ['high'] : ['high', 'medium', 'light'],
		sizes: software ? stood.sizes.slice(0, 1) : stood.sizes,
		module: '/src/lib/engine/pkg/shell_web.js',
		rounds: software ? 3 : 15,
		block: software ? 3 : 6
	};
	const answer = await send('Runtime.evaluate', {
		expression: `(${inPage.toString()})(${JSON.stringify(asked)})`,
		awaitPromise: true,
		returnByValue: true
	});
	if (answer.error) throw new Error(`the page: ${JSON.stringify(answer.error)}`);
	if (answer.exceptionDetails)
		throw new Error(`in the page: ${JSON.stringify(answer.exceptionDetails)}`);
	const measured = answer.result.value as { adapter: string; rows: Row[] };
	const out = join(dir, software ? 'software.json' : 'web.json');
	await Bun.write(out, `${JSON.stringify(measured, null, 1)}\n`);
	console.log(tables(measured.adapter, measured.rows));
	console.log(`-> ${out}`);
}

function tables(adapter: string, rows: Row[]): string {
	const ms = (value: number | undefined) => (value ?? 0).toFixed(2);
	const name = (row: Row) => `${row.view} ${row.preset} ${row.size[0]}x${row.size[1]}`;
	const lines = [
		`${adapter} (browser)`,
		'',
		'GPU, ms (median): the frame, first pass to last, and what it saves without each draw of foliage',
		'',
		'| view | frame | view foliage | near | mid | far | cascade 0 | cascade 1 | cascade 2 | all foliage | share |',
		'|---|---|---|---|---|---|---|---|---|---|---|'
	];
	if (!rows.some((row) => row.counted.timed))
		lines.push('(this browser writes no timestamps: no GPU times)');
	const priced = (pick: (row: Row) => Record<string, number>) => {
		for (const row of rows) {
			const saved = pick(row);
			const share = (100 * (saved.all ?? 0)) / Math.max(row.frame_ms, 1e-9);
			lines.push(
				`| ${name(row)} | ${ms(row.frame_ms)} | ${ms(saved.view)} | ${ms(saved.near)} | ${ms(saved.mid)} | ${ms(saved.far)} | ${ms(saved['cascade 0'])} | ${ms(saved['cascade 1'])} | ${ms(saved['cascade 2'])} | ${ms(saved.all)} | ${share.toFixed(0)}% |`
			);
		}
	};
	priced((row) => row.saved_ms);
	lines.push(
		'',
		'The same, each draw priced by its own pass alone: the view, for a form too, a cascade, or those four for all foliage',
		'',
		'| view | frame | view foliage | near | mid | far | cascade 0 | cascade 1 | cascade 2 | all foliage | share |',
		'|---|---|---|---|---|---|---|---|---|---|---|'
	);
	priced((row) => row.saved_in_pass_ms);
	lines.push(
		'',
		'WASM placing the foliage, ms (mean), and the instances in view by form, to match the desktop',
		'',
		'| view | gather | sort | write | all | instances |',
		'|---|---|---|---|---|---|'
	);
	for (const row of rows.filter((row) => row.size.join() === rows[0].size.join())) {
		const { gather, sort, write } = row.cpu_ms;
		const forms = row.counted.counted[0]?.forms ?? [];
		lines.push(
			`| ${row.view} ${row.preset} | ${gather.toFixed(3)} | ${sort.toFixed(3)} | ${write.toFixed(3)} | ${(gather + sort + write).toFixed(3)} | ${forms.join(', ')} |`
		);
	}
	return lines.join('\n');
}
