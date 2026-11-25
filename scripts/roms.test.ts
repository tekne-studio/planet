import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { audit, keep, kept, named, prune, sha256 } from './built';
import { ROOT } from './lib';
import { filesOf, LIST, type Listed, listed, ROMS, withoutNames, written } from './roms';

/** A folder of ROMs in a repository of its own, made afresh for each test. */
let folder: string;
beforeEach(() => {
	// Real, as cargo says a package's folder: macOS keeps its temporary ones behind a link.
	folder = realpathSync(mkdtempSync(`${tmpdir()}/roms-`));
	Bun.spawnSync(['git', 'init', '-q'], { cwd: folder });
});
afterEach(() => rmSync(folder, { recursive: true, force: true }));

const ROCK = new Uint8Array([0, 97, 115, 109, 1]);
const KING = new Uint8Array([0, 97, 115, 109, 2]);
const MINE = new Uint8Array([0, 97, 115, 109, 3]);

/** The list `roms` in `folder`, with their files and licences beside it. */
function shipped(roms: Listed[], files = true): Listed[] {
	writeFileSync(`${folder}/${LIST}`, written(roms));
	if (files) for (const rom of roms) if (rom.licenceFile !== undefined) writeFileSync(`${folder}/${rom.licenceFile}`, 'a licence of its own');
	return roms;
}

describe('the list of ROMs', () => {
	test('reads back what it wrote, in order: a licence, one of its own beside it, or rights reserved and no file', () => {
		const roms = shipped([
			{ name: 'rocks', sha256: keep(folder, ROCK, 'wasm').slice(0, 64), licence: 'MIT' },
			{ name: 'mine', sha256: keep(folder, MINE, 'wasm').slice(0, 64), licenceFile: 'mine.LICENSE' },
			{ name: 'kings', sha256: keep(folder, KING, 'wasm').slice(0, 64), rights: 'reserved' }
		]);
		expect(listed(folder)).toEqual(roms);
		expect(written(listed(folder))).toContain('name = "kings"\nsha256 = "' + sha256(KING) + '"\nrights = "reserved"\n');
		expect(audit(folder, LIST, filesOf(listed(folder)))).toEqual([]);
	});

	test('refuses a ROM that says none of a licence, a licence-file or its rights reserved, or two, or an empty one', () => {
		const rom = (terms: string) => writeFileSync(`${folder}/${LIST}`, `[[rom]]\nname = "rocks"\nsha256 = "${sha256(ROCK)}"\n${terms}`);
		for (const terms of ['', 'licence = "MIT"\nlicence-file = "rocks.LICENSE"\n', 'licence = "MIT"\nrights = "reserved"\n', 'licence = ""\n', 'rights = true\n', 'licence = "MIT"\nrights = ""\n']) {
			rom(terms);
			expect(() => listed(folder), terms).toThrow('a licence, a licence-file or rights = "reserved", one of the three');
		}
	});

	test('refuses rights of any kind but reserved', () => {
		writeFileSync(`${folder}/${LIST}`, `[[rom]]\nname = "kings"\nsha256 = "${sha256(KING)}"\nrights = "shared"\n`);
		expect(() => listed(folder)).toThrow('rights = "reserved", the one value it takes');
	});

	test('refuses a licence of its own under another name, a key it does not know, and a name twice', () => {
		writeFileSync(`${folder}/${LIST}`, `[[rom]]\nname = "rocks"\nsha256 = "${sha256(ROCK)}"\nlicence-file = "LICENSE"\n`);
		expect(() => listed(folder)).toThrow('its licence-file is rocks.LICENSE');
		writeFileSync(`${folder}/${LIST}`, `[[rom]]\nname = "rocks"\nsha256 = "${sha256(ROCK)}"\nlicence = "MIT"\ntitle = "ROCKS"\n`);
		expect(() => listed(folder)).toThrow('no title in a ROM');
		const rocks: Listed = { name: 'rocks', sha256: sha256(ROCK), licence: 'MIT' };
		writeFileSync(`${folder}/${LIST}`, written([rocks, rocks]));
		expect(() => listed(folder)).toThrow('rocks listed twice');
	});

	test("has NOTICE name each ROM whose rights are reserved, so none reads as under the engine's licence", () => {
		const notice = readFileSync(`${ROOT}/NOTICE`, 'utf8');
		for (const rom of listed()) if (rom.rights !== undefined) expect(notice, rom.name).toContain(`\`${rom.name}\``);
	});

	test("is this version's: every file it names is there and hashes to its name", () => {
		expect(audit(ROMS, LIST, filesOf(listed()))).toEqual([]);
	});

	test('keeps each ROM as `bun run rom` writes it, with no function names', () => {
		for (const { name, sha256 } of listed()) {
			const rom = kept(ROMS, sha256, 'wasm');
			expect([name, withoutNames(rom).length]).toEqual([name, rom.length]);
		}
	});
});

describe('a ROM', () => {
	/** A WebAssembly file of `sections`, each `[id, bytes]`. */
	const wasm = (...sections: [number, number[]][]) =>
		new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0, ...sections.flatMap(([id, bytes]) => [id, bytes.length, ...bytes])]);
	const custom = (name: string, ...bytes: number[]): [number, number[]] => [0, [name.length, ...new TextEncoder().encode(name), ...bytes]];

	test('is its built file without the names a build gives its functions, every other section as it was', () => {
		const types: [number, number[]] = [1, [1, 0x60, 0, 0]];
		const cover = custom('cartridge', 1, 2, 3);
		expect(withoutNames(wasm(types, custom('name', 9, 9), cover, custom('producers', 4)))).toEqual(wasm(types, cover, custom('producers', 4)));
		expect(withoutNames(wasm(types, cover))).toEqual(wasm(types, cover));
	});

	test('reads a size of several bytes, and refuses what is no WebAssembly or is cut short', () => {
		// 205 bytes, the name's five and 200 more: 0xcd 0x01 as LEB128.
		const names = [4, ...new TextEncoder().encode('name'), ...Array.from({ length: 200 }, (_, at) => at)];
		const file = new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0, 0, 0xcd, 0x01, ...names, 1, 0]);
		expect(withoutNames(file)).toEqual(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0, 1, 0]));
		expect(() => withoutNames(new Uint8Array([1, 2, 3, 4, 1, 0, 0, 0]))).toThrow('not a WebAssembly file');
		expect(() => withoutNames(wasm([1, [1, 2, 3]]).subarray(0, 11))).toThrow('runs past the end');
	});
});

describe('the audit of a folder of built files', () => {
	test('names a listed file missing, one changed since it was named, and one nobody lists', () => {
		const rocks = keep(folder, ROCK, 'wasm');
		const kings = named(sha256(KING), 'wasm');
		writeFileSync(`${folder}/${rocks}`, KING);
		writeFileSync(`${folder}/stray.wasm`, ROCK);
		shipped([
			{ name: 'rocks', sha256: rocks.slice(0, 64), licence: 'MIT' },
			{ name: 'kings', sha256: kings.slice(0, 64), licence: 'MIT' }
		]);
		const at = folder.split('/').pop();
		expect(audit(folder, LIST, filesOf(listed(folder)))).toEqual([
			`${at}/${rocks}: its content hashes otherwise than its name`,
			`${at}/${kings}: ${LIST} names it, and it is not there`,
			`${at}/stray.wasm: ${LIST} does not name it`
		]);
	});

	test('reads the folder as git sees it: a file git ignores is no part of it', () => {
		const rocks = keep(folder, ROCK, 'wasm');
		shipped([{ name: 'rocks', sha256: rocks.slice(0, 64), licence: 'MIT' }]);
		writeFileSync(`${folder}/.git/info/exclude`, '.DS_Store\n');
		writeFileSync(`${folder}/.DS_Store`, '');
		writeFileSync(`${folder}/stray.wasm`, KING);
		const at = folder.split('/').pop();
		expect(audit(folder, LIST, [rocks])).toEqual([`${at}/stray.wasm: ${LIST} does not name it`]);
		prune(folder, LIST, [rocks]);
		expect(audit(folder, LIST, [rocks])).toEqual([]);
	});

	test('reads a kept file only when it hashes to its name', () => {
		const rocks = keep(folder, ROCK, 'wasm');
		expect(kept(folder, sha256(ROCK), 'wasm')).toEqual(ROCK);
		writeFileSync(`${folder}/${rocks}`, KING);
		expect(() => kept(folder, sha256(ROCK), 'wasm')).toThrow(`hashes to ${sha256(KING)}, not to its name`);
		expect(() => kept(folder, sha256(KING), 'wasm')).toThrow('not there');
	});

	test('prunes what a new build replaced, and keeps the manifest and what it names', () => {
		const rocks = keep(folder, ROCK, 'wasm');
		const kings = keep(folder, KING, 'wasm');
		writeFileSync(`${folder}/${LIST}`, '');
		prune(folder, LIST, [kings]);
		expect(audit(folder, LIST, [kings])).toEqual([]);
		expect(() => kept(folder, rocks.slice(0, 64), 'wasm')).toThrow('not there');
	});
});
