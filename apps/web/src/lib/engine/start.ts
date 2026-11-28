import type { Effects, Presets } from './protocol';
import { weak, type Family } from './machine';
import { lifeKnobs, livesOf, matching, presetNames, same, type Lives, type Picture, type PresetName } from './settings';

/** The adapter asked for, on a machine with two: the fastest, the engine's default, or the one that spares power. */
export const powers = ['high', 'low'] as const;
export type Power = (typeof powers)[number];

/** The road to the GPU: WebGPU where the browser has it, the engine's default, or WebGL2 even where it does. */
export const roads = ['webgpu', 'webgl2'] as const;
export type Road = (typeof roads)[number];

/** How the engine is asked for a GPU, as this browser keeps it: a field left out is the engine's default. */
export interface Gpu {
	power?: Power;
	backend?: Road;
}

/** What `Engine.create` starts with, as the engine reads it (`shell-web`'s `start.rs`). */
export interface Start extends Gpu {
	/** The page is given a paint after each step of the start, so one that stops shows where. */
	paint?: boolean;
	/** What the first frame is drawn with. */
	effects?: Effects;
}

/** How the engine is asked for a GPU, as this browser kept it: anything else is the engine's default. */
export function readGpu(text: string | null): Gpu {
	const kept = parsed(text);
	const gpu: Gpu = {};
	if (kept && typeof kept === 'object') {
		const { power, backend } = kept as Record<string, unknown>;
		if (powers.some((known) => known === power)) gpu.power = power as Power;
		if (roads.some((known) => known === backend)) gpu.backend = backend as Road;
	}
	return gpu;
}

/** The steps of a start, in the order they are reached: the page's and the engine's between `module` and `world`. */
export const steps = ['module', 'adapter', 'device', 'pipelines', 'frame', 'drawn', 'world'] as const;
export type Step = (typeof steps)[number];

/**
 * How the last start in this browser ended: none yet, steady, refused by
 * the engine, or unsteady, with the last step it reached (`watch.ts`).
 */
export type Last = 'none' | 'steady' | 'failed' | { stopped: Step | null };

/** Why the graphics dialog opens before the engine starts, the most pressing first. */
export type Why = 'failed' | 'unsteady' | 'safe' | 'weak';

/**
 * Why the graphics dialog opens whatever the machine is: the engine
 * refused what was kept, the last start never was steady, or the address
 * asked. It opens at once, before the browser is asked of the graphics.
 */
export function pressing(said: { failed: boolean; last: Last; safe: boolean }): Why | null {
	if (said.failed) return 'failed';
	if (typeof said.last === 'object') return 'unsteady';
	return said.safe ? 'safe' : null;
}

/**
 * Whether the graphics dialog opens before the engine starts, and why: for
 * a pressing reason; else on a machine whose graphics may not hold the
 * world, once for its GPU. Never on a touch screen, which starts light,
 * nor where the browser keeps no answer (`keeps`), which would ask at
 * every visit; a family not asked (null) asks nothing.
 */
export function opens(
	said: { failed: boolean; last: Last; safe: boolean; touch: boolean; family: Family | null; asked: boolean; keeps: boolean }
): Why | null {
	const why = pressing(said);
	if (why || said.touch || said.asked || !said.keeps || said.family === null) return why;
	return weak(said.family) ? 'weak' : null;
}

/** A picture as the panel keeps it: the preset last picked, which gives each knob its default, and the knobs. */
export interface Picked {
	base: PresetName;
	effects: Effects;
}

/**
 * The picture a start asks for: what this browser kept, else the light one
 * where the screen is touched or the graphics may not hold more, else High.
 */
export function picked(presets: Presets, kept: { effects: Effects | null; named: PresetName | null }, light: boolean): Picked {
	const fallback: PresetName = light ? 'light' : 'high';
	const effects = kept.effects ?? presets[fallback];
	return { base: kept.named ?? matching(effects, presets) ?? fallback, effects };
}

/**
 * What the dialog asks: a preset, or Custom where what starts is set off
 * them all; the trees, shrubs, flowers and animals; and the GPU.
 */
export interface Draft extends Lives {
	picture: Picture;
	gpu: Gpu;
}

/**
 * What the dialog shows a picture as: the preset it is, what lives aside,
 * since the dialog asks that on its own; else Custom. Never the preset
 * picked last, which gives the panel's knobs their defaults: what the
 * dialog shows chosen is what starts.
 */
export function pictureOf(effects: Effects, presets: Presets): Picture {
	return presetNames.find((name) => same({ ...effects, ...livesOf(presets[name]) }, presets[name])) ?? 'custom';
}

/**
 * What the dialog shows chosen as it opens: what starts, the GPU kept, and
 * after a start that never ran steady, or on `safe`, the lightest picture
 * there is: Light with the trees alone, which stop the body, so nothing it
 * hides is a wall.
 */
export function drafted(why: Why | null, now: Effects, presets: Presets, gpu: Gpu): Draft {
	if (why === 'unsteady' || why === 'safe') return { picture: 'light', trees: true, shrubs: false, flowers: false, animals: false, gpu };
	return { picture: pictureOf(now, presets), ...livesOf(now), gpu };
}

/** A choice the panel makes, and the dialog makes the same way: a preset picked, or the knobs set. */
export type Choice = { pick: PresetName } | { choose: Effects };

/**
 * The dialog's answer as the panel's choices, so what starts is what it
 * showed, kept as the panel keeps it: a preset picked when what starts is
 * not that preset, when no picture was kept, so what it starts with is
 * kept, or when the engine refused the start, so a kept picture it could
 * not read is written anew; then the trees, shrubs, flowers and animals
 * set over it when they differ. Custom is what starts, and stays;
 * refused, it is written anew.
 */
export function choices(now: Effects, draft: Draft, presets: Presets, keptAs: 'kept' | 'none' | 'refused'): Choice[] {
	const out: Choice[] = [];
	let effects = now;
	const { picture } = draft;
	if (picture !== 'custom' && (picture !== pictureOf(now, presets) || keptAs !== 'kept')) {
		out.push({ pick: picture });
		effects = presets[picture];
	}
	const set = { ...effects, ...livesOf(draft) };
	if (!same(set, effects) || (picture === 'custom' && keptAs !== 'kept')) out.push({ choose: set });
	return out;
}

/** Whether the GPU asked differs: the engine takes it only as it starts. */
export function newGpu(kept: Gpu, asked: Gpu): boolean {
	return (kept.power ?? 'high') !== (asked.power ?? 'high') || (kept.backend ?? 'webgpu') !== (asked.backend ?? 'webgpu');
}

/** JSON as this browser kept it; null where it is no JSON. */
export function parsed(text: string | null): unknown {
	try {
		return JSON.parse(text ?? 'null');
	} catch {
		return null;
	}
}
