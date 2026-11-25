// A built file the engine ships and never builds: committed in the engine,
// in a folder of its kind beside the manifest that names it, and named by
// the hash of its content as every heavy file is, `<sha256>.<ext>`. Its
// maker's command writes it and the manifest, and its commit is the only
// way it changes. The ROMs (`arcade/roms/`, `roms.toml`) are read through
// here by `bun run wasm`, the bench and the image alike, and their test
// audits their folder (roms.test.ts), as the emulator's does its. A folder
// is what git sees in it, what a commit carries: a file git ignores, a
// `.DS_Store`, is no part of it.
import { existsSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { basename } from 'node:path';

/** The sha256 of `bytes`, as the hex a built file is named by. */
export const sha256 = (bytes: Uint8Array): string => new Bun.CryptoHasher('sha256').update(bytes).digest('hex');

/** The name of a built file of content `sha` and extension `ext`. */
export const named = (sha: string, ext: string): string => `${sha}.${ext}`;

/** A built file's name: a sha256 and an extension. */
const BUILT = /^([0-9a-f]{64})\.([a-z0-9]+)$/;

/** The bytes of the built file `<sha>.<ext>` in `folder`; throws when it is missing or its content hashes otherwise. */
export function kept(folder: string, sha: string, ext: string): Uint8Array {
	const path = `${folder}/${named(sha, ext)}`;
	if (!existsSync(path)) throw new Error(`${path}: not there`);
	const bytes = new Uint8Array(readFileSync(path));
	const found = sha256(bytes);
	if (found !== sha) throw new Error(`${path}: its content hashes to ${found}, not to its name`);
	return bytes;
}

/** Writes `bytes` into `folder` as a built file of extension `ext`, and gives its name. */
export function keep(folder: string, bytes: Uint8Array, ext: string): string {
	const name = named(sha256(bytes), ext);
	writeFileSync(`${folder}/${name}`, bytes);
	return name;
}

/**
 * The files right in `folder` as git sees them: tracked, or new and not
 * ignored, and there on disk. Empty for a folder that is not there.
 */
export function seen(folder: string): string[] {
	if (!existsSync(folder)) return [];
	const run = Bun.spawnSync(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard', '--', '.'], { cwd: folder });
	if (run.exitCode !== 0) throw new Error(`${folder}: git ls-files: ${run.stderr.toString().trim()}`);
	const files = run.stdout.toString().split('\0').filter((file) => file && !file.includes('/'));
	return [...new Set(files)].filter((file) => existsSync(`${folder}/${file}`));
}

/** Removes from `folder` every file but its manifest and `names`: what a new build replaced. */
export function prune(folder: string, manifest: string, names: Iterable<string>) {
	const keep = new Set([manifest, ...names]);
	for (const file of seen(folder)) if (!keep.has(file)) rmSync(`${folder}/${file}`);
}

/**
 * What is wrong in `folder` against its manifest, which names `names`: a
 * named file missing, a built file whose content hashes otherwise than its
 * name, a file the manifest does not name. Empty when it holds.
 */
export function audit(folder: string, manifest: string, names: string[]): string[] {
	const wrong: string[] = [];
	const at = basename(folder);
	const files = new Set(seen(folder));
	for (const name of names) {
		if (!files.has(name)) {
			wrong.push(`${at}/${name}: ${manifest} names it, and it is not there`);
			continue;
		}
		const sha = name.match(BUILT)?.[1];
		if (sha && sha256(new Uint8Array(readFileSync(`${folder}/${name}`))) !== sha) {
			wrong.push(`${at}/${name}: its content hashes otherwise than its name`);
		}
	}
	for (const file of files) {
		if (file !== manifest && !names.includes(file)) wrong.push(`${at}/${file}: ${manifest} does not name it`);
	}
	return wrong;
}
