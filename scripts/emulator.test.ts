import { expect, test } from 'bun:test';
import { audit, kept } from './built';
import { EMULATOR, filesOf, hostedEmulator, MANIFEST, manifest } from './emulator';
import { listed, ROMS, titles } from './roms';

// The engine carries the emulator built: what it can hold of it is what
// its files are, never how they were built, which the console proves from
// its source.
test("is this version's: every file its manifest names is there and hashes to its name, and nothing else lies beside it", () => {
	expect(audit(EMULATOR, MANIFEST, filesOf(manifest()))).toEqual([]);
});

test('runs on this engine: its runner hosted reads every ROM this version ships', async () => {
	const emulator = await hostedEmulator();
	expect([emulator.width, emulator.height]).toEqual([manifest().width, manifest().height]);
	const title = await titles();
	for (const { name, sha256 } of listed()) expect(title(kept(ROMS, sha256, 'wasm')), name).not.toBe('');
});
