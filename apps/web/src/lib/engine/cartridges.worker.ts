/**
 * The worker that runs a page's cartridges off its frame: the emulator the
 * engine pins, its runner on the browser's own WebAssembly by its host
 * (`emulator.ts`). The engine (`crates/shell-web/src/carts.rs`) creates it
 * the first time a cabinet in view has a game, so a page that never comes
 * near one never loads it. It asks it to open a cartridge from its URL
 * under a name and a seed, to show a cartridge's cover, still, from its
 * URL, to show one whose file never came as its stop screen, to close one,
 * to hold buttons on one, the contract's bits, from its next update on, and
 * to hold a game where it is, its player gone, and to run it on from there.
 * It tells it each frame the page drew while a game runs or a screen is
 * still to be said, and the worker ticks its games then, by the emulator's
 * clock: it runs no faster than the page draws, and not at all in a hidden
 * tab. It answers with the size of a game's screen, then each screen that
 * changed, handed over, never copied, what each game played since the last
 * tick, as the chip's processor takes it, and a game that stopped for good.
 * Asked to read a game, it answers with its palette and its pixels, what an
 * agent reads. When its emulator breaks, it says why once and stops.
 */
import { hostEmulator, type Emulator } from './emulator';

/** What the engine asks. */
type Asked =
	| { open: number; url: string; seed: number }
	| { cover: number; url: string; name: string }
	| { fail: number }
	| { close: number }
	| { press: number; buttons: number }
	| { hold: number }
	| { resume: number }
	| { read: number }
	| { tick: true };
/** What the worker answers. */
type Told =
	| { opened: number; width: number; height: number }
	| { failed: number; message: string }
	| { frame: number; rgba: ArrayBuffer }
	| { stopped: number }
	| { sound: number; batch: ArrayBuffer }
	| { read: number; palette: ArrayBuffer; pixels: ArrayBuffer }
	| { dead: string };

/** The worker's own scope, as much of it as this uses. */
interface Scope {
	postMessage(message: Told, transfer?: Transferable[]): void;
	onmessage: ((event: MessageEvent<Asked>) => void) | null;
	close(): void;
}
const scope = self as unknown as Scope;

/** The emulator, once hosted: a tick before then has nothing to tick. */
let hosted: Emulator | undefined;
const ready = hostEmulator().then((emulator) => (hosted = emulator));
// Seen once it is awaited; caught here so parts never copied are no unhandled rejection.
ready.catch(() => {});

/** Opens asked whose files are on their way: a close that comes meanwhile wins. */
const opening = new Set<number>();
/** The games opened to run and not held, whose sound is taken each tick. */
const running = new Set<number>();
/** The games held, their player gone. */
const held = new Set<number>();
/**
 * When the games were last ticked: a game is held as it was then, so no
 * update falls due between that tick and the hold to run while it is held.
 */
let ticked = 0;
let dead = false;

const why = (error: unknown) => (error instanceof Error ? error.message : String(error));

/**
 * Runs a call into the emulator. One that throws is its runner broken: the
 * emulator is left unusable, so the worker says why and stops.
 */
function run(work: () => void) {
	if (dead) return;
	try {
		work();
	} catch (error) {
		dead = true;
		scope.postMessage({ dead: why(error) });
		scope.close();
	}
}

function tick(emulator: Emulator) {
	run(() => {
		ticked = performance.now();
		for (const id of emulator.tick(ticked)) {
			const rgba = emulator.rgba(id);
			if (!rgba) continue;
			scope.postMessage({ frame: id, rgba: rgba.buffer }, [rgba.buffer]);
			// Said as its stop screen changes, once.
			if (emulator.stopped(id)) scope.postMessage({ stopped: id });
		}
		for (const id of running) {
			const batch = emulator.sound(id);
			if (batch.length > 0) scope.postMessage({ sound: id, batch: batch.buffer }, [batch.buffer]);
		}
	});
}

async function fetched(url: string): Promise<Uint8Array> {
	const response = await fetch(url);
	if (!response.ok) throw new Error(`${response.status} for ${url}`);
	return new Uint8Array(await response.arrayBuffer());
}

/**
 * Puts `id` on the shelf once the emulator is hosted and its file, if it
 * has one, came: answered with the size of its screen, or why it did not.
 */
async function shelve(id: number, url: string | null, put: (emulator: Emulator, file: Uint8Array) => void) {
	opening.add(id);
	let found: [Emulator, Uint8Array];
	try {
		found = await Promise.all([ready, url === null ? new Uint8Array() : fetched(url)]);
	} catch (error) {
		if (opening.delete(id)) scope.postMessage({ failed: id, message: why(error) });
		return;
	}
	if (!opening.delete(id)) return;
	const [emulator, file] = found;
	run(() => {
		put(emulator, file);
		scope.postMessage({ opened: id, width: emulator.width, height: emulator.height });
	});
}

scope.onmessage = ({ data }) => {
	if ('tick' in data) {
		if (hosted) tick(hosted);
	} else if ('open' in data) {
		const { open: id, url, seed } = data;
		void shelve(id, url, (emulator, file) => {
			emulator.open(id, file, seed, performance.now());
			running.add(id);
		});
	} else if ('cover' in data) {
		const { cover: id, url, name } = data;
		void shelve(id, url, (emulator, file) => emulator.still(id, file, name, performance.now()));
	} else if ('fail' in data) {
		const id = data.fail;
		void shelve(id, null, (emulator) => emulator.fail(id, performance.now()));
	} else if ('press' in data) {
		const emulator = hosted;
		if (emulator) run(() => emulator.press(data.press, data.buttons));
	} else if ('hold' in data) {
		// Its clock holds where its last tick left it, a share of the next
		// update with it: run on, it is as if it never held.
		const emulator = hosted;
		const id = data.hold;
		if (emulator && running.delete(id)) {
			held.add(id);
			run(() => emulator.pace(id, 0, ticked));
		}
	} else if ('resume' in data) {
		const emulator = hosted;
		const id = data.resume;
		if (emulator && held.delete(id)) {
			running.add(id);
			run(() => emulator.pace(id, 1, performance.now()));
		}
	} else if ('read' in data) {
		const emulator = hosted;
		if (emulator)
			run(() => {
				const read = emulator.read(data.read);
				if (!read) return;
				// Four bytes a colour, then a byte a pixel.
				const at = read.length - emulator.width * emulator.height;
				const palette = read.slice(0, at).buffer;
				const pixels = read.slice(at).buffer;
				scope.postMessage({ read: data.read, palette, pixels }, [palette, pixels]);
			});
	} else {
		const id = data.close;
		opening.delete(id);
		running.delete(id);
		held.delete(id);
		ready.then((emulator) => run(() => emulator.close(id))).catch(() => {});
	}
};
