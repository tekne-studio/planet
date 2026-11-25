// The emulator the engine carries: the console's build, pinned in
// `arcade/emulator/` as built files named by their hash, with `emulator.json`
// naming each (scripts/built.ts). The engine never builds it: `bun run
// wasm`, the bench, the ROMs' titles, the server and the image copy the
// parts they run, each checked against its hash, and its test audits the
// folder and reads every ROM with it (emulator.test.ts). Pinning another is
// the console's command, `bun run emulator --pin <this folder>` in TKN-32,
// and a commit; that the files are the console's build is proved where its
// source is.
//
//   bun scripts/emulator.ts <folder>           copies the parts a page runs into <folder>
//   bun scripts/emulator.ts --judge <folder>   copies what the server judges by into <folder>
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { kept } from './built';
import { ROOT } from './lib';

/** The pinned emulator's folder, and its manifest's name. */
export const EMULATOR = `${ROOT}/arcade/emulator`;
export const MANIFEST = 'emulator.json';

/**
 * The files `emulator.json` names: the emulator's parts, and its cases,
 * what every host is held to.
 */
const PARTS = ['runner', 'host', 'chip', 'processor', 'cases'] as const;

/** The pinned manifest, as read. */
export function manifest(folder = EMULATOR): Record<string, unknown> {
	return JSON.parse(readFileSync(`${folder}/${MANIFEST}`, 'utf8'));
}

/** The built files `emulator.json` names. */
export function filesOf(read: Record<string, unknown>): string[] {
	return PARTS.map((part) => {
		const file = read[part];
		if (typeof file !== 'string') throw new Error(`${MANIFEST} names no ${part}`);
		return file;
	});
}

/**
 * The parts a page or a script runs, each copied by its role: the runner
 * and its host, and the chip with the processor a game sounds through.
 */
const RUN = ['runner', 'host', 'chip', 'processor'] as const;

/**
 * Copies the parts of the pinned emulator a page runs into `out`, each
 * checked against its hash and named by its role (`runner.wasm`,
 * `host.js`, `chip.wasm`, `processor.js`), so a page names what it loads.
 */
export function copyEmulator(out: string, folder = EMULATOR) {
	const read = manifest(folder);
	mkdirSync(out, { recursive: true });
	for (const part of RUN) {
		const [sha, ext] = String(read[part]).split('.');
		writeFileSync(`${out}/${part}.${ext}`, kept(folder, sha, ext));
	}
}

/**
 * Copies what the server judges a cartridge by into `out`, the folder it
 * embeds: the manifest, and the runner it names, under that name, checked
 * against its hash. Nothing else is left there; the server checks the
 * runner against its name again as it starts.
 */
export function copyJudge(out: string, folder = EMULATOR) {
	const read = manifest(folder);
	const runner = String(read.runner);
	const [sha, ext] = runner.split('.');
	rmSync(out, { recursive: true, force: true });
	mkdirSync(out, { recursive: true });
	writeFileSync(`${out}/${MANIFEST}`, readFileSync(`${folder}/${MANIFEST}`));
	writeFileSync(`${out}/${runner}`, kept(folder, sha, ext));
}

/** The host's emulator, as `host.js` gives it, enough of it for a script. */
export interface Emulator {
	width: number;
	height: number;
	open(id: number, file: Uint8Array, seed: number, now: number): void;
	tick(now: number): Uint32Array;
	rgba(id: number): Uint8Array | null;
	cover(file: Uint8Array, name: string): { said: { title: string; maker: string; year: number } | null; rgba: Uint8Array };
}

/**
 * The pinned emulator on this engine's WebAssembly, for a script: its parts
 * copied into `target/emulator`, checked, and its runner hosted.
 */
export async function hostedEmulator(): Promise<Emulator> {
	const out = `${ROOT}/target/emulator`;
	copyEmulator(out);
	const host = await import(`${out}/host.js`);
	return host.hosted(new WebAssembly.Module(readFileSync(`${out}/runner.wasm`)));
}

if (import.meta.main) {
	const args = process.argv.slice(2);
	const judge = args[0] === '--judge';
	const [out] = judge ? args.slice(1) : args;
	if (!out) throw new Error('bun scripts/emulator.ts [--judge] <folder>');
	if (judge) copyJudge(out);
	else copyEmulator(out);
}
