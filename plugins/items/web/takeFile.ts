/**
 * A file given to items, dropped anywhere on the page or chosen in the
 * panel, becomes the item it is: a picture, a sound, a model of voxels from
 * a `.vox`, a 3D model from a `.glb`, or a cabinet running a cartridge, a
 * `.wasm`. It is sent to the world through the seam and placed in front of
 * the body as soon as it is ready. With a part picked in the world, a file
 * it takes goes into it instead: a picture into a picture, a cartridge
 * into a cabinet, and a still picture onto a cabinet as its skin. Pure of
 * the DOM: it reads the file alone.
 */
import { kindOf } from '$lib/files';
import { Refused } from '$lib/instance';
import type { Seam } from '$lib/plugins/plugin';
import { isPicture, isVideo, prepareImage, type Media } from './image';
import type { Picked } from './protocol';

/** What a file is to items, by its name and type: a picture, a model of voxels, a 3D model, a sound, a cartridge, or none of them. */
export type Given = 'picture' | 'model' | 'glb' | 'sound' | 'cartridge';

/** The audio kinds the world takes: what every browser plays. */
const SOUND_TYPES = ['audio/mpeg', 'audio/mp4', 'audio/x-m4a', 'audio/ogg'];

/** Why a file was not taken: the world's codes, and the browser's. */
export type NotTaken = 'kind' | 'size' | 'level' | 'key' | 'unreachable' | 'canvas';
const refusals: readonly NotTaken[] = ['kind', 'size', 'level', 'key', 'unreachable', 'canvas'];

/** Why the world refuses a cartridge: the codes of the emulator's `check`, which the page judges by too, and the world's own when its judgement runs past its deadline (`late`) or breaks (`trapped`). */
const contract = ['size', 'invalid', 'import', 'export', 'memory', 'table', 'frame', 'segment', 'instruction', 'late', 'trapped'] as const;
type Breaks = (typeof contract)[number];

/** Why a file does not fit what it was given to: a 3D model by the engine's weighing, against what a world draws of one; a cartridge by the contract, `cartridge` for a file that is no WebAssembly or one refused for no reason given; `skin` for one no cabinet wears. */
export type Unfit = 'broken' | 'triangles' | 'texels' | 'materials' | 'vertices' | 'cartridge' | `cartridge.${Breaks}` | 'skin';
const unfits: readonly Unfit[] = [
	...(['broken', 'triangles', 'texels', 'materials', 'vertices', 'cartridge', 'skin'] as const),
	...contract.map((code) => `cartridge.${code}` as const)
];

/** What a file would be, by its name and type, before it is read. Null for one items takes no interest in. */
export function givenOf(name: string, type: string): Given | null {
	if (/\.vox$/i.test(name)) return 'model';
	if (/\.glb$/i.test(name)) return 'glb';
	if (/\.wasm$/i.test(name) || type === 'application/wasm') return 'cartridge';
	if (isPicture({ type } as Blob) || /\.(png|jpe?g|webp|gif|mp4)$/i.test(name)) return 'picture';
	if (SOUND_TYPES.includes(type) || /\.(mp3|m4a|ogg|oga|opus)$/i.test(name)) return 'sound';
	return null;
}

/** What a file being dragged over the page would be, from the types a drag says before the drop: a picture, still or moving, is known by its type, a `.glb` by the type a browser may give it, a `.vox` says nothing until dropped. */
export function draggedOf(types: readonly string[]): Given | 'any' {
	if (types.some((type) => type.startsWith('image/') || type.startsWith('video/'))) return 'picture';
	if (types.includes('model/gltf-binary')) return 'glb';
	if (types.includes('application/wasm')) return 'cartridge';
	if (types.some((type) => type.startsWith('audio/'))) return 'sound';
	return 'any';
}

/** Reads the refusal code of what was thrown, in the words this panel has. */
export function notTaken(error: unknown): NotTaken {
	const code = error instanceof Error ? error.message : '';
	return refusals.find((known) => known === code) ?? 'unreachable';
}

/** Reads the refusal code of what was thrown where a model or a cabinet's file may be given: why it does not fit too. */
export function notTakenOrUnfit(error: unknown): NotTaken | Unfit {
	const code = error instanceof Error ? error.message : '';
	return unfits.find((known) => known === code) ?? notTaken(error);
}

/** A cartridge refused by the contract, by the code the page's judge or the world gave: `cartridge.<code>`, or `cartridge` with none known. */
function breaks(error: unknown): Refused {
	const code = error instanceof Error ? error.message : '';
	return new Refused(contract.some((known) => known === code) ? `cartridge.${code}` : 'cartridge');
}

/** How long an audio file plays, in milliseconds, off an audio element loaded from it; 0 when the browser cannot say. */
async function playsFor(file: Blob): Promise<number> {
	const url = URL.createObjectURL(file);
	const audio = new Audio();
	audio.preload = 'metadata';
	try {
		await new Promise<void>((loaded, failed) => {
			audio.onloadedmetadata = () => loaded();
			audio.onerror = () => failed(new Error('kind'));
			audio.src = url;
		});
		return Number.isFinite(audio.duration) ? Math.round(audio.duration * 1000) : 0;
	} finally {
		audio.removeAttribute('src');
		audio.load();
		URL.revokeObjectURL(url);
	}
}

/** What taking a file came to: the item it is, and a picture's record. */
export interface Taken {
	given: Given;
	media?: Media;
}

/**
 * Takes a file: sends it to the world and places the item it is in front
 * of the body, as soon as it is ready, or puts it into the picture picked.
 * Throws an `Error` whose message is a `NotTaken` code, or an `Unfit` one
 * for a 3D model or a cartridge.
 */
export async function takeFile(seam: Seam, file: File, picked?: Picked | null): Promise<Taken> {
	const given = givenOf(file.name, file.type);
	if (given === null) throw new Refused('kind');
	if (given === 'model') {
		const head = new Uint8Array(await file.slice(0, 16).arrayBuffer());
		if (kindOf(head) !== 'vox') throw new Refused('kind');
		const asset = await seam.upload(file);
		seam.command('take', { item: 'model', asset, ahead: true });
		return { given };
	}
	if (given === 'glb') {
		const head = new Uint8Array(await file.slice(0, 16).arrayBuffer());
		if (kindOf(head) !== 'glb') throw new Refused('kind');
		// Weighed before it leaves: a model over the budget is never sent.
		await seam.weigh(file);
		const asset = await seam.upload(file);
		seam.command('take', { item: 'glb', asset, ahead: true });
		return { given };
	}
	if (given === 'cartridge') {
		const head = new Uint8Array(await file.slice(0, 16).arrayBuffer());
		if (kindOf(head) !== 'wasm') throw new Refused('cartridge');
		// Held to the contract here as the world will hold it, so a refusal
		// is told before the file leaves, and why. The world prepares it as
		// an engine will before it keeps it: what it refuses of the file
		// itself is a module no cabinet runs.
		await seam.weigh(file).catch((error: unknown) => {
			throw breaks(error);
		});
		const cart = await seam.upload(file).catch((error: unknown) => {
			const code = error instanceof Error ? error.message : '';
			throw ['level', 'key', 'unreachable'].includes(code) ? error : breaks(error);
		});
		if (picked?.item === 'cabinet') seam.command('set', { cart });
		else seam.command('take', { item: 'cabinet', cart, ahead: true });
		return { given };
	}
	if (given === 'sound') {
		const head = new Uint8Array(await file.slice(0, 16).arrayBuffer());
		if (!['mp3', 'm4a', 'ogg'].includes(kindOf(head) ?? '')) throw new Refused('kind');
		const [asset, millis] = await Promise.all([seam.upload(file), playsFor(file)]);
		seam.command('take', { item: 'sound', clip: { file: asset, millis }, ahead: true });
		return { given };
	}
	// A picture onto a cabinet picked is its skin.
	if (picked?.item === 'cabinet') {
		await takeSkin(seam, file, picked);
		return { given };
	}
	const media = await prepareImage(file, (blob) => seam.upload(blob));
	// A picture picked takes either kind of file in its place: a still and a
	// video are the same part, told apart by their file.
	if (picked?.item === 'picture' || picked?.item === 'video') {
		seam.command('set', { media });
	} else {
		seam.command('take', { item: isVideo(file) ? 'video' : 'picture', media, ahead: true });
	}
	return { given, media };
}

/**
 * Takes a still picture as a cabinet's skin, painted by the cabinet's
 * layout: onto the cabinet picked, else into the hand for the next cabinet
 * placed. It goes up as a picture does, its three files, and a cabinet
 * shows its thumb or its low version as near as it stands. Throws an
 * `Error` whose message is a `NotTaken` or an `Unfit` code.
 */
export async function takeSkin(seam: Seam, file: File, picked?: Picked | null): Promise<void> {
	const head = new Uint8Array(await file.slice(0, 16).arrayBuffer());
	if (!['png', 'jpg', 'webp'].includes(kindOf(head) ?? '')) throw new Refused('skin');
	const media = await prepareImage(file, (blob) => seam.upload(blob));
	// A skin never plays: an animated WebP is worn as its first frame, the
	// frame its versions are drawn from.
	const skin = { ...media, millis: 0 };
	if (picked?.item === 'cabinet') seam.command('set', { skin });
	else seam.command('take', { item: 'cabinet', skin });
}
