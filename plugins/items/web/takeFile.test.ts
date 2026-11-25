import { describe, expect, mock, test } from 'bun:test';
import type { Seam } from '$lib/plugins/plugin';
import { kindOf } from '../../../apps/web/src/lib/files';
import { Refused } from '../../../apps/web/src/lib/instance';

// The page's own modules, as the app's aliases name them.
mock.module('$lib/files', () => ({ kindOf }));
mock.module('$lib/instance', () => ({ Refused }));
const { notTakenOrUnfit, takeFile } = await import('./takeFile');

/** A seam whose weighing and sending refuse with the codes given, or take the file. */
function seam(weighs: string, sends: string, commands: string[] = []): Seam {
	return {
		command: (kind) => void commands.push(kind),
		core: () => {},
		listen: () => () => {},
		release: () => {},
		take: () => {},
		upload: async () => {
			if (sends) throw new Refused(sends);
			return `asset://${'0'.repeat(64)}.wasm`;
		},
		weigh: async () => {
			if (weighs) throw new Refused(weighs);
		},
		cabinetTemplate: async () => null,
		cover: async () => null
	};
}

const wasm = new File([new Uint8Array([0x00, 0x61, 0x73, 0x6d, 1, 0, 0, 0])], 'rocks.wasm');
const refusal = (taking: Promise<unknown>) => taking.then(() => null, notTakenOrUnfit);

describe('a cartridge given', () => {
	test('one the page holds to the contract is refused before it is sent, and says why', async () => {
		const commands: string[] = [];
		expect(await refusal(takeFile(seam('import', '', commands), wasm))).toBe('cartridge.import');
		expect(commands).toEqual([]);
	});

	test("one the world refuses says the world's why", async () => {
		expect(await refusal(takeFile(seam('', 'frame'), wasm))).toBe('cartridge.frame');
		expect(await refusal(takeFile(seam('', 'late'), wasm))).toBe('cartridge.late');
		expect(await refusal(takeFile(seam('', 'unknown'), wasm))).toBe('cartridge');
	});

	test('who may send it, and whether the world is there, are said as they are', async () => {
		expect(await refusal(takeFile(seam('', 'level'), wasm))).toBe('level');
		expect(await refusal(takeFile(seam('', 'unreachable'), wasm))).toBe('unreachable');
	});

	test('one that is no WebAssembly is no cartridge, and one that keeps the contract is placed', async () => {
		const text = new File(['not a module'], 'rocks.wasm');
		expect(await refusal(takeFile(seam('', ''), text))).toBe('cartridge');
		const commands: string[] = [];
		await takeFile(seam('', '', commands), wasm);
		expect(commands).toEqual(['take']);
	});
});
