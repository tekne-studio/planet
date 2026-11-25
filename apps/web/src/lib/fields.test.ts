import { afterEach, describe, expect, test } from 'bun:test';
import { contentId, DamagedField, fetchField } from './fields';

// The Earth every version carries, as `bun run field --replace` wrote it.
const FIELDS = new URL('../../../../assets/fields/', import.meta.url);
const earth = new Uint8Array(await Bun.file(new URL('earth.field', FIELDS)).arrayBuffer());
const sidecar: { id: string } = await Bun.file(new URL('earth.json', FIELDS)).json();

const hex = (bytes: Uint8Array) => Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
/** The Earth with one byte changed. */
const patched = (at: number, value: number) => {
	const bytes = earth.slice();
	bytes[at] = value;
	return bytes;
};

describe('contentId', () => {
	test('the committed Earth, its sidecar and the check name one ground', async () => {
		const id = await contentId(earth);
		expect(id).toBe(sidecar.id);
		expect(hex(earth.subarray(8, 40))).toBe(id);
	});

	test('the magic is ground: a format of the same planes is another id', async () => {
		expect(await contentId(patched(7, '3'.charCodeAt(0)))).not.toBe(await contentId(earth));
	});

	test('every plane is ground, and the id is not', async () => {
		const id = await contentId(earth);
		expect(await contentId(patched(earth.length - 1, earth[earth.length - 1] ^ 1))).not.toBe(id);
		expect(await contentId(patched(8, earth[8] ^ 1))).toBe(id);
	});
});

describe('fetchField', () => {
	const real = globalThis.fetch;
	afterEach(() => {
		globalThis.fetch = real;
	});
	const serving = (bytes: Uint8Array<ArrayBuffer>) => {
		globalThis.fetch = (async () => new Response(bytes)) as unknown as typeof fetch;
	};

	test('hands over the bytes of the ground the recipe names', async () => {
		serving(earth);
		expect(await fetchField('/assets/fields/earth.field', sidecar.id)).toEqual(earth);
	});

	test('refuses other bytes under that name, whatever id they carry', async () => {
		serving(patched(earth.length - 1, earth[earth.length - 1] ^ 1));
		await expect(fetchField('/assets/fields/earth.field', sidecar.id)).rejects.toBeInstanceOf(DamagedField);
	});
});
