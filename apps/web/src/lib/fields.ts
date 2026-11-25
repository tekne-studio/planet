import { FIELD_PATH, fieldUrl, type Baked, type Field } from '$lib/world';

// Baked fields are served under `/assets/fields`, beside the rest of the
// asset set. The sidecar beside each field names the ground, so a page builds
// a whole recipe without downloading a texel.

/** Where a field file keeps its id: the 32 bytes after its 8 of magic. */
const ID = { start: 8, end: 40 };

/**
 * A field's content id, as hex: the sha256 of its file with the id's own bytes
 * zeroed. It covers the magic, so a format that reads the same planes by
 * another rule names other ground. `bun run field` writes it into the file and
 * the sidecar; `fetchField` checks fetched bytes by it.
 */
export async function contentId(file: Uint8Array): Promise<string> {
	const blank = file.slice();
	blank.fill(0, ID.start, ID.end);
	const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', blank));
	return Array.from(digest, (byte) => byte.toString(16).padStart(2, '0')).join('');
}

/** The bytes fetched for a field are not the ground its id names. */
export class DamagedField extends Error {}

/**
 * The bytes of the field a recipe names, checked against its id where they
 * arrive: the engine reads the id a file carries and never recomputes it.
 * A file that is not that ground throws `DamagedField`.
 */
export async function fetchField(url: string, id: string): Promise<Uint8Array> {
	const response = await fetch(url);
	if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
	const bytes = new Uint8Array(await response.arrayBuffer());
	if ((await contentId(bytes)) !== id) throw new DamagedField(url);
	return bytes;
}

/** The sidecar of a field is the `.json` beside its `.field`. */
function sidecarPath(shape: Baked): string {
	return FIELD_PATH[shape].replace(/\.field$/, '.json');
}

/** Null when this version has no such field, or its sidecar cannot be read. */
export async function readFieldSidecar(shape: Baked): Promise<Field | null> {
	try {
		const response = await fetch(sidecarPath(shape));
		if (!response.ok) return null;
		const field = (await response.json()) as Field;
		return field.id ? field : null;
	} catch {
		// Unreadable: the page offers the generated shape.
		return null;
	}
}

/**
 * The URL the field a recipe names is fetched from, or null when this version
 * does not have that ground. Looked up by content id, never by name: a world
 * is only ever the world its field was baked for.
 */
export async function servedField(id: string | null): Promise<string | null> {
	if (!id) return null;
	for (const shape of Object.keys(FIELD_PATH) as Baked[]) {
		const field = await readFieldSidecar(shape);
		if (field?.id === id) return fieldUrl(shape, id);
	}
	return null;
}
