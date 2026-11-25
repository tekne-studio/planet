// The ROMs this version ships: each a game's built cartridge, kept by its
// sha256 (`built.ts`) beside `arcade/roms/roms.toml`, which lists them in
// the order a cabinet's panel offers them. The engine builds none: a game's
// maker puts its ROM there with `bun run rom` (`rom.ts`). Shipped, they are
// written into the asset set, `assets/carts/<sha256>.wasm`, with
// `assets/carts/games.json`, `[{ name, title, file }]` in order. A game's
// title is its cover's, read from its ROM by the pinned emulator at every
// ship, so no list keeps a copy of it and a file the emulator refuses never
// ships. `bun run wasm` ships them, with the extra carts `bun run dev
// --cart` is given; the image's web stage ships them alone:
//   bun scripts/roms.ts
import { mkdirSync, readFileSync, rmSync } from 'node:fs';
import { basename } from 'node:path';
import { kept, named, sha256 } from './built';
import { hostedEmulator } from './emulator';
import { ROOT } from './lib';

/** Where the ROMs are kept, beside their list. */
export const ROMS = `${ROOT}/arcade/roms`;
/** The list's name in `ROMS`. */
export const LIST = 'roms.toml';

const SERVED = `${ROOT}/assets/carts`;

/** A ROM as the list says it: its game, its file, and what it ships under, one of three. */
export type Listed = {
	/** How a command and the dev seam name its game: its built file's name (`nameOf`). */
	name: string;
	/** Its file's, `<sha256>.wasm` beside the list. */
	sha256: string;
} & (
	| {
			/** Its licence as an SPDX expression, its crate's `license`. */
			licence: string;
	  }
	| {
			/** Or its own licence's text, beside the list: its crate's `license-file`, as `<name>.LICENSE`. */
			licenceFile: string;
	  }
	| {
			/**
			 * Or no licence at all: every right in it is its maker's, and it ships
			 * under none, not even the engine's. Its crate says so as
			 * `[package.metadata.rom] rights = "reserved"`.
			 */
			rights: 'reserved';
	  }
);

/** The keys that say what a ROM ships under: it says one. */
const TERMS = ['licence', 'licence-file', 'rights'];

/**
 * A game's name: its built file's, `<name>.wasm`, the crate's with `-` as
 * `_` as cargo writes a cdylib, so the list and `bun run dev --cart` name a
 * game alike.
 */
const NAME = /^[a-z][a-z0-9_]*$/;

/** The name of the game built into `file`, a path to `<name>.wasm`. */
export const nameOf = (file: string): string => basename(file, '.wasm');

/** Whether `name` names a game as the list does. */
export const isName = (name: string): boolean => NAME.test(name);

/** A LEB128 number at `at` in `bytes`, and where it ends. */
function leb128(bytes: Uint8Array, at: number): [number, number] {
	let value = 0;
	for (let shift = 0; shift < 35; shift += 7) {
		if (at >= bytes.length) break;
		const byte = bytes[at++];
		value += (byte & 0x7f) * 2 ** shift;
		if (!(byte & 0x80)) return [value, at];
	}
	throw new Error("a section's size is cut short");
}

/**
 * A built cartridge as a ROM keeps it: without its `name` section, the
 * names a build gives its functions. Each carries a hash of the folder its
 * crate was built in, so the same source built in another checkout names
 * its functions otherwise, and no run reads them: a ROM is the same bytes
 * from any checkout. Every other section is kept as it is.
 */
export function withoutNames(file: Uint8Array): Uint8Array {
	const MAGIC = [0x00, 0x61, 0x73, 0x6d];
	if (MAGIC.some((byte, at) => file[at] !== byte)) throw new Error('not a WebAssembly file');
	const kept: Uint8Array[] = [file.subarray(0, 8)];
	for (let at = 8; at < file.length; ) {
		const [size, body] = leb128(file, at + 1);
		const end = body + size;
		if (end > file.length) throw new Error('a section runs past the end of the file');
		let named = false;
		if (file[at] === 0) {
			const [length, text] = leb128(file, body);
			named = new TextDecoder().decode(file.subarray(text, text + length)) === 'name';
		}
		if (!named) kept.push(file.subarray(at, end));
		at = end;
	}
	const rom = new Uint8Array(kept.reduce((total, part) => total + part.length, 0));
	kept.reduce((at, part) => (rom.set(part, at), at + part.length), 0);
	return rom;
}

/** The ROMs `folder`'s list ships, in order; throws naming what is wrong with it. */
export function listed(folder = ROMS): Listed[] {
	const parsed = Bun.TOML.parse(readFileSync(`${folder}/${LIST}`, 'utf8')) as { rom?: Record<string, unknown>[] };
	const roms = (parsed.rom ?? []).map((entry, at): Listed => {
		const said = (key: string) => (typeof entry[key] === 'string' ? (entry[key] as string) : undefined);
		const [name, sha] = [said('name'), said('sha256')];
		const which = `${LIST}, ROM ${at + 1}`;
		if (!name || !NAME.test(name)) throw new Error(`${which}: a name, its built file's, lower case with \`_\` for \`-\``);
		if (!sha || !/^[0-9a-f]{64}$/.test(sha)) throw new Error(`${which} (${name}): a sha256, 64 hex digits`);
		// The keys it holds are counted, so one given empty beside another is refused, never read as none.
		const terms = TERMS.filter((key) => key in entry);
		const [term] = terms;
		const value = terms.length === 1 ? said(term) : undefined;
		if (!value) throw new Error(`${which} (${name}): a licence, a licence-file or rights = "reserved", one of the three`);
		if (term === 'licence-file' && value !== `${name}.LICENSE`) throw new Error(`${which} (${name}): its licence-file is ${name}.LICENSE`);
		if (term === 'rights' && value !== 'reserved') throw new Error(`${which} (${name}): rights = "reserved", the one value it takes`);
		const unknown = Object.keys(entry).filter((key) => !['name', 'sha256', ...TERMS].includes(key));
		if (unknown.length) throw new Error(`${which} (${name}): no ${unknown.join(', ')} in a ROM`);
		if (term === 'licence') return { name, sha256: sha, licence: value };
		if (term === 'licence-file') return { name, sha256: sha, licenceFile: value };
		return { name, sha256: sha, rights: 'reserved' };
	});
	const twice = roms.find((rom, at) => roms.findIndex(({ name }) => name === rom.name) !== at);
	if (twice) throw new Error(`${LIST}: ${twice.name} listed twice`);
	return roms;
}

/** The files the list names beside it: each ROM, and its licence of its own. */
export const filesOf = (roms: Listed[]): string[] => roms.flatMap((rom) => [named(rom.sha256, 'wasm'), ...('licenceFile' in rom ? [rom.licenceFile] : [])]);

/** The list as its file says it, the order kept. */
export function written(roms: Listed[]): string {
	const head = [
		'# The ROMs this version ships, in the order a cabinet offers them: each a',
		'# game built into a cartridge beside this list, named by its sha256. `bun',
		'# run rom <built file>` writes an entry and its file; a commit of this',
		'# folder is the only way a ROM changes.'
	];
	const terms = (rom: Listed) =>
		'licence' in rom ? `licence = ${JSON.stringify(rom.licence)}` : 'licenceFile' in rom ? `licence-file = ${JSON.stringify(rom.licenceFile)}` : `rights = ${JSON.stringify(rom.rights)}`;
	const entries = roms.map((rom) => ['[[rom]]', `name = ${JSON.stringify(rom.name)}`, `sha256 = ${JSON.stringify(rom.sha256)}`, terms(rom)].join('\n'));
	return `${[head.join('\n'), ...entries].join('\n\n')}\n`;
}

/** A cart's title from its cover, by the pinned emulator, hosted once; throws for a cart with none. */
export async function titles(): Promise<(file: Uint8Array) => string> {
	const emulator = await hostedEmulator();
	return (file) => {
		const title = emulator.cover(file, '').said?.title;
		if (title === undefined) throw new Error('no cover: its build script makes one with cartridge_build::cover');
		return title;
	};
}

/** A game as `games.json` offers it. */
interface Offered {
	name: string;
	title: string;
	/** `<sha256>.wasm`. */
	file: string;
}

/**
 * Writes the ROMs into the asset set, the folder whole: a game changed
 * leaves no file of the last. `extra` are built carts by path, offered
 * after them for one run, each named by its file's name; one named as a
 * ROM takes its place, so a game is heard in a cabinet before its ROM is
 * committed. Every file is read and its title checked first, so one
 * that fails leaves the folder as it was. Gives the titles, in order.
 */
export async function shipRoms(extra: string[] = []): Promise<string[]> {
	const title = await titles();
	const games: { name: string; bytes: Uint8Array; from: string }[] = listed().map(({ name, sha256 }) => ({
		name,
		bytes: kept(ROMS, sha256, 'wasm'),
		from: `arcade/roms/${named(sha256, 'wasm')}`
	}));
	for (const path of extra) {
		const name = nameOf(path);
		if (!path.endsWith('.wasm') || !NAME.test(name)) throw new Error(`${path}: a built cart, <name>.wasm`);
		const game = { name, bytes: new Uint8Array(await Bun.file(path).bytes()), from: path };
		const at = games.findIndex((shipped) => shipped.name === name);
		if (at === -1) games.push(game);
		else games[at] = game;
	}
	const offered: (Offered & { bytes: Uint8Array })[] = games.map(({ name, bytes, from }) => {
		try {
			return { name, title: title(bytes), file: named(sha256(bytes), 'wasm'), bytes };
		} catch (error) {
			throw new Error(`${from}: ${(error as Error).message}`);
		}
	});
	rmSync(SERVED, { recursive: true, force: true });
	mkdirSync(SERVED, { recursive: true });
	for (const { file, bytes } of offered) await Bun.write(`${SERVED}/${file}`, bytes);
	const json = offered.map(({ name, title, file }) => ({ name, title, file }));
	await Bun.write(`${SERVED}/games.json`, `${JSON.stringify(json, null, '\t')}\n`);
	return offered.map((game) => game.title);
}

if (import.meta.main) console.log(`${(await shipRoms()).join(', ')} -> ${SERVED}`);
