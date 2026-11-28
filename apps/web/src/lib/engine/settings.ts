import { toneMaps, type Effects, type Loudness, type Presets } from './protocol';

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

/** The knobs of what lives, each a switch of what this machine draws. */
export const lifeKnobs = ['trees', 'shrubs', 'flowers', 'animals'] as const satisfies readonly (keyof Effects)[];
export type LifeKnob = (typeof lifeKnobs)[number];
export type Lives = Pick<Effects, LifeKnob>;

/** What of life a picture draws, alone. */
export function livesOf(effects: Lives): Lives {
	return { trees: effects.trees, shrubs: effects.shrubs, flowers: effects.flowers, animals: effects.animals };
}

/**
 * The picture this browser kept, whole: a field a record kept by an older
 * version lacks is the preset's that the browser names, else the one whose
 * fields the record matches, else the light one; never High's, the heaviest.
 * A field the engine does not know, or a value it does not read (another
 * type than the preset's, a number not finite, a tone map it does not
 * name, as an older or newer engine may have kept), is dropped, so the
 * engine is never handed a picture it refuses. Null when nothing, or
 * nothing readable, is kept.
 */
export function filled(text: string | null, presets: Presets, named: PresetName | null): Effects | null {
	let kept: unknown;
	try {
		kept = JSON.parse(text ?? 'null');
	} catch {
		return null;
	}
	if (!kept || typeof kept !== 'object' || Array.isArray(kept)) return null;
	const shape = presets.light;
	const record = Object.fromEntries(
		Object.entries(kept).filter(([key, value]) => key in shape && readable(key as keyof Effects, value, shape[key as keyof Effects]))
	) as Partial<Effects>;
	const keys = Object.keys(record) as (keyof Effects)[];
	const like = presetNames.find((name) => presets[name] && keys.every((key) => record[key] === presets[name][key]));
	return { ...presets[named ?? like ?? 'light'], ...record };
}

/** Whether the engine reads `value` in the field `key`, whose preset value is `like`. */
function readable(key: keyof Effects, value: unknown, like: unknown): boolean {
	if (typeof value !== typeof like) return false;
	if (typeof value === 'number') return Number.isFinite(value);
	return key !== 'tone_map' || toneMaps.some((name) => name === value);
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
	/** The readout over the world: what the machine says and how far its start went. */
	readout: boolean;
}

export const gameDefaults: Game = { place: true, pet: true, home: true, favourite: false, readout: false };

/**
 * Whether the readout shows: one state, the choice kept in Settings, which
 * the address's `readout` sets for the visit (`said`, null where it says
 * nothing), winning while the person has not turned it since.
 */
export function readoutOn(said: boolean | null, game: Game): boolean {
	return said ?? game.readout;
}

/**
 * A change in the panel, which shows the readout as it is this visit: the
 * game to keep, and what the address still says. Turning the readout is
 * the person speaking: kept, and the address's word let go. Any other
 * change keeps the readout as it was kept.
 */
export function gameChosen(next: Game, said: boolean | null, game: Game): { game: Game; said: boolean | null } {
	if (next.readout !== readoutOn(said, game)) return { game: next, said: null };
	return { game: { ...next, readout: game.readout }, said };
}

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
