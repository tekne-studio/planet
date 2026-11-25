// What a frame costs in WASM, against budgets. Fails when one is blown.
//   bun run bench
// Budgets are for the development machine (Apple silicon). A Raspberry Pi or
// a phone is several times slower, which is why they are this tight.
import { $ } from 'bun';
import { kept } from './built';
import { hostedEmulator } from './emulator';
import { listed, ROMS } from './roms';
import { ROOT } from './lib';

const PATCH_SAMPLES = 35 * 35;
const PATCH_BUDGET_MS = 1.5;
const FRAME_BUDGET_MS = 12;
// A volume patch carries 50 times the samples of the height it replaces, so it
// gets its own budget: a handful of them a frame, not one a millisecond.
const VOLUME_BUDGET_MS = 8;
// The cabinets' runner is off the frame's thread: a tick costs the frame
// nothing and the worker's core, a phone's second, what it takes. Its
// tail, not its worst, is held: a pause of the JS heap's collector lands
// on any tick. A millisecond here is a few on a phone, a slice of the
// 16.7 ms each tick has to keep pace with the page.
const RUNNER_BUDGET_MS = 1;

$.cwd(ROOT);
const OUT = `${ROOT}/target/bench-pkg`;
const BUILT = `${ROOT}/target/wasm32-unknown-unknown/release`;
// The cabinets' runner is the emulator the engine pins, on this engine's
// WebAssembly as a page's worker runs it, and the game Rocks, the ROM this
// version ships.
await $`cargo build --release --target wasm32-unknown-unknown -p bench`;
await $`wasm-bindgen --target web --out-dir ${OUT} ${BUILT}/bench.wasm`;
const rocks = listed().find(({ name }) => name === 'rocks');
if (!rocks) throw new Error('the bench plays rocks, and arcade/roms/roms.toml ships none');
const bench = await import(`${OUT}/bench.js`);
bench.initSync({ module: await Bun.file(`${OUT}/bench_bg.wasm`).bytes() });

let failed = false;
const report = (what: string, ms: number, budget: number) => {
	const over = ms > budget;
	failed ||= over;
	console.log(`${over ? 'OVER' : 'ok  '} ${what}: ${ms.toFixed(2)} ms (budget ${budget})`);
};

// A world shaped by a field is the other hot path.
bench.load_field(await Bun.file(`${ROOT}/assets/fields/earth.field`).bytes());
const runs: [number, string][] = [
	[0, 'planet'],
	[1, 'moon'],
	[2, 'planet over a field']
];

// A descent from 3 km to the ground of a body, every frame timed.
function descend(body: number, name: string, life: boolean) {
	bench.descent_start(body, life);
	let worst = 0;
	let worstAt = -1;
	const times: number[] = [];
	for (let frame = 0; frame < 600; frame++) {
		const before = performance.now();
		bench.descent_frame();
		const ms = performance.now() - before;
		times.push(ms);
		if (ms > worst) {
			worst = ms;
			worstAt = frame;
		}
	}
	report(`${name}: worst frame of a 10 s descent`, worst, FRAME_BUDGET_MS);
	const sorted = [...times].sort((a, b) => a - b);
	const over = times.filter((t) => t > FRAME_BUDGET_MS).length;
	console.log(
		`     worst at frame ${worstAt}, median ${sorted[300].toFixed(2)} ms, ` +
			`p99 ${sorted[594].toFixed(2)} ms, ${over} frames over budget`
	);
}

for (const [body, name] of runs) {
	const rounds = 40;
	bench.samples(body, PATCH_SAMPLES * rounds);
	const start = performance.now();
	bench.samples(body, PATCH_SAMPLES * rounds);
	report(`${name}: samples of one patch`, (performance.now() - start) / rounds, PATCH_BUDGET_MS);
	descend(body, name, false);
}
// The planet once more with its life on: its plants grown at the start and
// placed in every patch built.
descend(0, 'planet with life', true);

// What one body's collision costs, apart from the frame it lands in: a
// footing is what holds it up, and a step asks for up to four of them.
// A world full of agents is what the budget leaves room for: at this one,
// twenty of them walking cost a millisecond between them.
const FOOTING_BUDGET_MS = 0.05;
{
	const rounds = 2000;
	bench.footings(rounds);
	const start = performance.now();
	bench.footings(rounds);
	const ms = (performance.now() - start) / rounds;
	report('planet: four footings, one body one step', ms * 4, FOOTING_BUDGET_MS);
	console.log(`     ${(ms * 1000).toFixed(1)} us a footing`);
}

// A body running across cave country, where every frame reads the density
// field for what holds it up and for what stops it. Terrain keeps streaming
// under it, so this is the whole frame and not only the collision in it.
{
	bench.walk_start();
	let worst = 0;
	for (let frame = 0; frame < 600; frame++) {
		const before = performance.now();
		bench.walk_frame();
		worst = Math.max(worst, performance.now() - before);
	}
	report('planet: worst frame of a 10 s walk in cave country', worst, FRAME_BUDGET_MS);
}

// A body walking among two thousand cubes placed around it: every one is
// drawn each frame, and the footing of each step asks the ones near. The
// frame is the whole frame, the ground settled under it first: its arriving
// is the descents' to time, and what the walk streams is its own. The
// footing is apart from it, as the ground's.
{
	const count = 2000;
	bench.parts_start(count);
	let worst = 0;
	for (let frame = 0; frame < 300; frame++) {
		const before = performance.now();
		bench.parts_frame();
		worst = Math.max(worst, performance.now() - before);
	}
	report(`planet: worst frame of a 5 s walk among ${bench.parts_count()} parts`, worst, FRAME_BUDGET_MS);
	const rounds = 200;
	bench.parts_footings(rounds);
	const start = performance.now();
	bench.parts_footings(rounds);
	const ms = (performance.now() - start) / rounds;
	report(`planet: four footings among ${count} parts, one body one step`, ms * 4, FOOTING_BUDGET_MS);
	console.log(`     ${(ms * 1000).toFixed(1)} us a footing`);
}

// A platform of 64 laid on a solid base over the steepest plot near the test
// world: the frame the command lands in, and every frame after it until each
// chunk it changed is drawn. The worst of them is what a hand feels.
{
	bench.lay_start();
	const before = performance.now();
	bench.lay_platform();
	const command = performance.now() - before;
	let worst = 0;
	let after = 0;
	let frames = 0;
	for (; frames < 240; frames++) {
		const start = performance.now();
		const settled = bench.lay_frame();
		const ms = performance.now() - start;
		after = Math.max(after, ms);
		worst = Math.max(worst, frames === 0 ? command + ms : ms);
		if (settled) break;
	}
	report('planet: worst frame while a platform of 64 is laid', worst, FRAME_BUDGET_MS);
	console.log(
		`     the command ${command.toFixed(2)} ms, the worst frame after it ` +
			`${after.toFixed(2)} ms, laid and drawn whole after ${frames + 1} frames`
	);
}

// One patch of the finest quadtree level, as a volume instead of a height:
// 32 x 32 columns and a 32 m window of cells up each. What this costs against
// `samples` is what says whether the volume layer can be meshed in a frame.
const VOLUME_COLUMNS = 32 * 32;
const VOLUME_CELLS = 64;
{
	const rounds = 4;
	bench.volume(rounds, VOLUME_COLUMNS, VOLUME_CELLS);
	const start = performance.now();
	bench.volume(rounds, VOLUME_COLUMNS, VOLUME_CELLS);
	const ms = (performance.now() - start) / rounds;
	report('planet: one patch as a volume', ms, VOLUME_BUDGET_MS);
	console.log(
		`     ${(VOLUME_COLUMNS * VOLUME_CELLS).toLocaleString()} cells, ` +
			`${((ms * 1e6) / (VOLUME_COLUMNS * VOLUME_CELLS)).toFixed(0)} ns a cell`
	);
}

// A voice is rendered once a seed, in the frame it is first struck, and kept.
// The long case is four layers for a second at 48 kHz, longer than the
// whistle.
const VOICE_BUDGET_MS = 3;
{
	const rounds = 50;
	bench.voice_render(rounds);
	const start = performance.now();
	bench.voice_render(rounds);
	const ms = (performance.now() - start) / rounds;
	report('a voice of four layers, one second, rendered', ms, VOICE_BUDGET_MS);
	console.log(`     ${((ms * 1e6) / (4 * 48_000)).toFixed(1)} ns a sample of a layer`);
}

// A stage is a world as a person has it, every plugin and life on, a body
// in herd country. Its frames are timed as the descents' are.
function stage(what: string, frames: number, budget: number, said = () => '') {
	const times: number[] = [];
	for (let frame = 0; frame < frames; frame++) {
		const before = performance.now();
		bench.stage_frame();
		times.push(performance.now() - before);
	}
	const sorted = [...times].sort((a, b) => a - b);
	report(what, sorted[frames - 1], budget);
	const over = times.filter((t) => t > budget).length;
	console.log(
		`     median ${sorted[frames >> 1].toFixed(2)} ms, p99 ${sorted[Math.floor(frames * 0.99)].toFixed(2)} ms, ` +
			`${over} frames over budget${said()}`
	);
}
const drawn = () => `, ${bench.stage_animals()} animals drawn at the end`;

// A body running through herd country with life on: herds placed as their
// regions come within reach, the 24 nearest posed and drawn, the regions
// behind let go. The ground settled first: the descents time its arriving.
bench.herds_start();
stage('planet: worst frame of a 10 s run through herd country', 600, FRAME_BUDGET_MS, drawn);

// The same country under a rider: a horse at a gallop, stepped under its
// rider on the ground and what is built; a pegasus flown low at its sprint,
// wings out, over the ground or the sea floor.
bench.gallop_start();
stage('planet: worst frame of a 10 s gallop through herd country', 600, FRAME_BUDGET_MS, drawn);
bench.flight_start();
stage('planet: worst frame of a 10 s pegasus flight low over herd country', 600, FRAME_BUDGET_MS, drawn);

// The same run in a world that tells 200 pets, each an account's, across
// the planet: those of herd country take the herd's places, and every
// frame reads every pet told.
const PETS = 200;
bench.pets_start(PETS);
stage(`planet: worst frame of a 10 s run in a world of ${PETS} pets`, 600, FRAME_BUDGET_MS, drawn);
// Their share of a frame, standing in herd country: the mean frame with
// them, less the mean frame of the same world telling none. Every client
// reads every pet a world tells, each frame: a tenth of a millisecond for
// 200 accounts is a millisecond at 2000, where a filter by region would
// be owed.
const PETS_BUDGET_MS = 0.1;
{
	const mean = (pets: number) => {
		bench.pets_start(pets);
		bench.stage_stand();
		for (let frame = 0; frame < 60; frame++) bench.stage_frame();
		const start = performance.now();
		for (let frame = 0; frame < 1200; frame++) bench.stage_frame();
		return (performance.now() - start) / 1200;
	};
	const none = mean(0);
	const share = mean(PETS) - none;
	report(`planet: ${PETS} pets told, their share of a frame`, share, PETS_BUDGET_MS);
	console.log(
		`     ${none.toFixed(3)} ms a frame standing in a world of none, ` +
			`${((share * 1000) / PETS).toFixed(2)} us a pet`
	);
}

// A body turning where it stands among 48 cabinets showing their covers,
// none played, and every horse drawn near: the prompt chooses among both
// plugins' offers each frame.
bench.offers_start(48);
stage('planet: worst frame turning among 48 cabinets and the horses near', 600, FRAME_BUDGET_MS, () => {
	return `, ${bench.stage_animals()} horses offered`;
});

// A cabinet played, running rocks, and four, the one played running and
// three showing their covers, still: an idle cabinet runs nothing.
// Played, the eye eases to the screen in a third of a second and the
// ground under it is built finer as it comes near: the opening is a camera
// move like any other, held to the house line. Once the eye rests, the
// frame's share is the client's; the runner's is the worker's, off the
// frame's thread on the browser's own WebAssembly: it ticks each game the
// client keeps running, 60 times a second, and colours each screen that
// changed, as `cartridges.worker.ts` does. Standing on settled ground a
// frame is under half a millisecond: what the cabinets add to it shows
// against 2 ms, where the house line would hide a tenfold regression.
const CABINETS_BUDGET_MS = 2;
// Ten times the ease: an opening that has not rested by then never will.
const OPENING_FRAMES = 200;
const plural = (count: number) => (count > 1 ? 's' : '');
const ROCKS = kept(ROMS, rocks.sha256, 'wasm');
for (const cabinets of [1, 4]) {
	bench.cabinets_start(cabinets);
	const opening: number[] = [];
	while (!bench.stage_eye_rests()) {
		if (opening.length === OPENING_FRAMES) throw new Error(`the eye never rests at a cabinet`);
		const before = performance.now();
		bench.stage_frame();
		opening.push(performance.now() - before);
	}
	const worst = Math.max(...opening);
	report(`planet: worst frame opening a game before ${cabinets} cabinet${plural(cabinets)}`, worst, FRAME_BUDGET_MS);
	console.log(`     the eye at the screen in ${opening.length} frames, ${opening.filter((t) => t > CABINETS_BUDGET_MS).length} over ${CABINETS_BUDGET_MS} ms`);
	const games = bench.stage_games();
	const emulator = await hostedEmulator();
	let now = 0;
	for (let game = 0; game < games; game++) emulator.open(game, ROCKS, game + 1, now);
	const ticks: number[] = [];
	stage(`planet: worst frame before ${cabinets} cabinet${plural(cabinets)}, one playing rocks`, 600, CABINETS_BUDGET_MS);
	for (let frame = 0; frame < 600; frame++) {
		now += 1000 / 60;
		const before = performance.now();
		for (const id of emulator.tick(now)) emulator.rgba(id);
		ticks.push(performance.now() - before);
	}
	ticks.sort((a, b) => a - b);
	report(`rocks: p99 tick of ${games} game${plural(games)}, the runner's share of a frame`, ticks[594], RUNNER_BUDGET_MS);
	console.log(`     median ${ticks[300].toFixed(3)} ms, worst ${ticks[599].toFixed(2)} ms`);
}

if (failed) process.exit(1);
