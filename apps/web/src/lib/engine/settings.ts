import type { Effects, Loudness, Presets } from './protocol';

/** The presets, lightest first: the order the panel offers them in. */
export const presetNames = ['light', 'medium', 'high'] as const satisfies readonly (keyof Presets)[];
export type PresetName = (typeof presetNames)[number];

/** What the picture is: one of the presets, or knobs set off them all. */
export type Picture = PresetName | 'custom';

/** The preset the effects in force are, field by field, if they are one. */
export function matching(effects: Effects, presets: Presets): PresetName | null {
	return presetNames.find((name) => presets[name] && same(effects, presets[name])) ?? null;
}

export function same(a: Effects, b: Effects): boolean {
	return (Object.keys(b) as (keyof Effects)[]).every((key) => a[key] === b[key]);
}

/** A preset's name as this browser kept it, if it is one. */
export function presetName(text: string | null): PresetName | null {
	return presetNames.find((known) => known === text) ?? null;
}

/**
 * What a person chose of the game, as against the picture: what floats over
 * the world. Kept in this browser, as the picture is.
 */
export interface Game {
	/** The line at the top right that says where the body is. */
	place: boolean;
	/** The marker over the body's pet. */
	pet: boolean;
	/** The marker over the body's home. */
	home: boolean;
	/** A marker over each of the body's favourite places. */
	favourite: boolean;
}

export const gameDefaults: Game = { place: true, pet: true, home: true, favourite: false };

/** The kinds of marker a game hides, as `hide_markers` names them. */
export const markerKinds = ['pet', 'home', 'favourite'] as const satisfies readonly (keyof Game)[];

export function hiddenMarkers(game: Game): string[] {
	return markerKinds.filter((kind) => !game[kind]);
}

/** The game's choices as this browser kept them: a choice it lacks, or kept wrong, is its default. */
export function readGame(text: string | null): Game {
	return readKept(text, gameDefaults);
}

/** How loud each group of what is heard is, until a person sets it: everything. */
export const loudnessDefaults: Loudness = { master: 1, muted: false, world: 1, steps: 1, calls: 1, games: 1 };

/** The loudness as this browser kept it: a level it lacks, or kept outside 0 to 1, is its default. */
export function readLoudness(text: string | null): Loudness {
	return readKept(text, loudnessDefaults, (value) => typeof value !== 'number' || (value >= 0 && value <= 1));
}

/**
 * Choices kept as JSON, read against their defaults: each one missing, of
 * another type, or refused by `fits` is its default.
 */
function readKept<T extends object>(text: string | null, defaults: T, fits: (value: unknown) => boolean = () => true): T {
	let kept: unknown;
	try {
		kept = JSON.parse(text ?? 'null');
	} catch {
		kept = null;
	}
	const read = { ...defaults };
	if (kept && typeof kept === 'object') {
		for (const key of Object.keys(defaults) as (keyof T)[]) {
			const value = (kept as Record<keyof T, unknown>)[key];
			if (typeof value === typeof defaults[key] && fits(value)) read[key] = value as T[keyof T];
		}
	}
	return read;
}
