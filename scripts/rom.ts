// Puts a game's ROM in what this version ships, run by its maker: takes the
// cartridge the console built (`bun run cart` or `bun run workbench` in
// TKN-32 builds it) and the terms it ships under, refuses a file with no
// cover, and writes `arcade/roms/<sha256>.wasm` and its entry in
// `arcade/roms/roms.toml` under the file's name, where that name already
// stood or last. The ROM is the file without its function names
// (`withoutNames`), the same bytes from any checkout. A licence of its own
// is copied beside as `<name>.LICENSE`; a ROM whose rights are reserved
// ships under no licence, and says so.
// It commits nothing: the maker reads the diff and commits `arcade/roms/`,
// the only way a ROM changes.
//   bun run rom path/to/rocks.wasm --licence MIT
//   bun run rom path/to/my_game.wasm --licence-file path/to/LICENSE
//   bun run rom path/to/my_game.wasm --rights reserved
import { copyFileSync, existsSync, mkdirSync, renameSync, writeFileSync } from 'node:fs';
import { relative } from 'node:path';
import { parseArgs } from 'node:util';
import { keep, prune } from './built';
import { filesOf, isName, LIST, type Listed, listed, nameOf, ROMS, titles, withoutNames, written } from './roms';

/** What a ROM ships under: a licence, an SPDX expression or a file of its own, or none, its rights reserved. */
export type Terms = { licence: string } | { file: string } | { rights: 'reserved' };

/** The options `bun run rom` takes after the file, one of them given. */
export type Options = { licence?: string; 'licence-file'?: string; rights?: string };

/**
 * The terms its maker gives `bun run rom`: one option, not empty. The options given are
 * counted, so one given empty beside another is refused, never read as none.
 */
export function termsGiven(options: Options): Terms {
	const given = (['licence', 'licence-file', 'rights'] as const).filter((key) => options[key] !== undefined);
	const [key] = given;
	const value = given.length === 1 ? options[key] : undefined;
	if (!value || (key === 'rights' && value !== 'reserved')) throw new Error('one of --licence <SPDX expression>, --licence-file <file> or --rights reserved');
	return key === 'licence' ? { licence: value } : key === 'licence-file' ? { file: value } : { rights: 'reserved' };
}

/** Puts the cartridge in `file` in the list as its ROM; gives its entry and title. */
export async function shipRom(file: string, said: Terms): Promise<{ rom: Listed; title: string }> {
	const name = nameOf(file);
	if (!file.endsWith('.wasm') || !isName(name)) throw new Error(`${file}: a built cart, <name>.wasm, its name lower case with \`_\` for \`-\``);
	// The list is read before anything is written: one that does not parse
	// leaves the folder as it was.
	const roms = existsSync(`${ROMS}/${LIST}`) ? listed() : [];
	const bytes = withoutNames(new Uint8Array(await Bun.file(file).bytes()));
	let title: string;
	try {
		title = (await titles())(bytes);
	} catch (error) {
		throw new Error(`${name}: ${(error as Error).message}`);
	}
	mkdirSync(ROMS, { recursive: true });
	const sha256 = keep(ROMS, bytes, 'wasm').slice(0, -'.wasm'.length);
	const rom: Listed = 'licence' in said ? { name, sha256, licence: said.licence } : 'file' in said ? { name, sha256, licenceFile: `${name}.LICENSE` } : { name, sha256, rights: said.rights };
	if ('file' in said) copyFileSync(said.file, `${ROMS}/${name}.LICENSE`);
	const at = roms.findIndex((entry) => entry.name === name);
	if (at === -1) roms.push(rom);
	else roms[at] = rom;
	// Written whole beside it, then put in its place: the list is never half written.
	const next = `${ROMS}/${LIST}.next`;
	writeFileSync(next, written(roms));
	renameSync(next, `${ROMS}/${LIST}`);
	prune(ROMS, LIST, filesOf(roms));
	return { rom, title };
}

/** Says where the ROM went. */
export function shipped({ rom, title }: { rom: Listed; title: string }) {
	const bytes = Bun.file(`${ROMS}/${rom.sha256}.wasm`).size;
	console.log(`${title} (${rom.name}, ${bytes} bytes) -> ${relative(process.cwd(), ROMS)}/${rom.sha256}.wasm`);
	console.log(`commit ${relative(process.cwd(), ROMS)}/ to ship it`);
}

if (import.meta.main) {
	const usage = 'bun run rom <file.wasm> (--licence <SPDX expression> | --licence-file <file> | --rights reserved)';
	const { values, positionals } = parseArgs({ allowPositionals: true, options: { licence: { type: 'string' }, 'licence-file': { type: 'string' }, rights: { type: 'string' } } });
	if (positionals.length !== 1) throw new Error(usage);
	shipped(await shipRom(positionals[0], termsGiven(values)));
}
