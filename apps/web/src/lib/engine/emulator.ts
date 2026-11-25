/**
 * The arcade's emulator as the page runs it: the parts the engine pins,
 * copied by `bun run wasm` into `./pkg/emulator` (gitignored, may be
 * absent), each by its role. The runner (`runner.wasm`) on the browser's
 * own WebAssembly, by its host (`host.js`), which gives a page the
 * runner's calls as plain JavaScript; the chip (`chip.wasm`) and the
 * processor (`processor.js`) a running game sounds through. Reading a cover
 * or checking a file runs no game and may run on the page; a game runs in
 * the cartridges' worker (`cartridges.worker.ts`).
 */

/** What a cartridge's cover says of it, when its file has one. */
export interface Said {
	title: string;
	maker: string;
	year: number;
}

/** The emulator `host.js` gives, as much of it as the page and its worker use. */
export interface Emulator {
	/** A screen's pixels across and down. */
	width: number;
	height: number;
	check(file: Uint8Array): string;
	cover(file: Uint8Array, name: string): { said: Said | null; rgba: Uint8Array<ArrayBuffer> };
	open(id: number, file: Uint8Array, seed: number, now: number): void;
	still(id: number, file: Uint8Array, name: string, now: number): void;
	fail(id: number, now: number): void;
	close(id: number): void;
	press(id: number, buttons: number): void;
	/** Runs its clock at `pace` times the contract's from `now`; 0 holds it. */
	pace(id: number, pace: number, now: number): void;
	tick(now: number): Uint32Array;
	idle(): boolean;
	stopped(id: number): boolean;
	rgba(id: number): Uint8Array<ArrayBuffer> | null;
	/** Its palette, four bytes a colour, then its pixels. */
	read(id: number): Uint8Array<ArrayBuffer> | null;
	/** What it played since last asked, as the chip's processor takes it. */
	sound(id: number): Uint32Array<ArrayBuffer>;
}

interface Host {
	hosted(runner: WebAssembly.Module): Promise<Emulator>;
}

// A glob resolves to nothing when the parts were never copied: the page
// type checks, builds and runs without them.
const hosts = import.meta.glob<Host>('./pkg/emulator/host.js');
const parts = import.meta.glob<string>(
	['./pkg/emulator/runner.wasm', './pkg/emulator/chip.wasm', './pkg/emulator/processor.js'],
	{ query: '?url', import: 'default' }
);

/** Where the part named `file` is served; throws when it was never copied. */
export async function partUrl(file: 'runner.wasm' | 'chip.wasm' | 'processor.js'): Promise<string> {
	const url = parts[`./pkg/emulator/${file}`];
	if (!url) throw new Error(`the emulator was not copied: run \`bun run wasm\``);
	return url();
}

/** The pinned emulator, its runner hosted on this page's or worker's WebAssembly. */
export async function hostEmulator(): Promise<Emulator> {
	const host = hosts['./pkg/emulator/host.js'];
	if (!host) throw new Error('the emulator was not copied: run `bun run wasm`');
	const [{ hosted }, runner] = await Promise.all([
		host(),
		partUrl('runner.wasm').then((url) => WebAssembly.compileStreaming(fetch(url)))
	]);
	return hosted(runner);
}
