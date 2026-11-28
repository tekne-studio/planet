import { describe, expect, test } from 'bun:test';
import type { Effects, Presets } from './protocol';
import { livesOf } from './settings';
import { choices, drafted, newGpu, opens, picked, pictureOf, pressing, readGpu, type Last } from './start';

const high = {
	shadows: true,
	grass: true,
	trees: true,
	shrubs: true,
	flowers: true,
	animals: true,
	foliage_shadows: true,
	foliage_reach_m: 600,
	resolution: 1
} as Effects;
const presets: Presets = {
	light: { ...high, shadows: false, grass: false, shrubs: false, flowers: false, foliage_shadows: false, foliage_reach_m: 250, resolution: 0.5 },
	medium: { ...high, foliage_shadows: false, foliage_reach_m: 400 },
	high
};

describe('readGpu', () => {
	test('reads what was kept and drops what it does not know', () => {
		expect(readGpu('{"power":"low","backend":"webgl2"}')).toEqual({ power: 'low', backend: 'webgl2' });
		expect(readGpu('{"power":"lowest","backend":"webgl"}')).toEqual({});
		expect(readGpu(null)).toEqual({});
		expect(readGpu('{')).toEqual({});
	});
});

describe('pressing', () => {
	const quiet = { failed: false, last: 'steady' as Last, safe: false };

	test('a refused start, an unsteady one and the address press; nothing else', () => {
		expect(pressing(quiet)).toBeNull();
		expect(pressing({ ...quiet, failed: true })).toBe('failed');
		expect(pressing({ ...quiet, last: { stopped: 'drawn' } })).toBe('unsteady');
		expect(pressing({ ...quiet, safe: true })).toBe('safe');
	});

	test('the most pressing reason is the one said', () => {
		expect(pressing({ failed: true, last: { stopped: null }, safe: true })).toBe('failed');
		expect(pressing({ ...quiet, last: { stopped: null }, safe: true })).toBe('unsteady');
	});
});

describe('opens', () => {
	const quiet = { failed: false, last: 'steady' as Last, safe: false, touch: false, family: 'nvidia' as const, asked: false, keeps: true };

	test('a capable machine with nothing amiss starts without asking', () => {
		expect(opens(quiet)).toBeNull();
	});

	test('a pressing reason opens on any screen, its graphics not asked', () => {
		expect(opens({ ...quiet, failed: true, touch: true, family: null })).toBe('failed');
		expect(opens({ ...quiet, last: { stopped: 'drawn' }, touch: true })).toBe('unsteady');
		expect(opens({ ...quiet, safe: true, touch: true, asked: true, keeps: false })).toBe('safe');
	});

	test('graphics that may be light ask once for their GPU, and never on a touch screen', () => {
		expect(opens({ ...quiet, family: 'amd-integrated' })).toBe('weak');
		expect(opens({ ...quiet, family: 'amd-integrated', asked: true })).toBeNull();
		expect(opens({ ...quiet, family: 'hidden', touch: true })).toBeNull();
	});

	test('a browser that keeps nothing is not asked, or it would be at every visit', () => {
		expect(opens({ ...quiet, family: 'intel', keeps: false })).toBeNull();
	});

	test('graphics not asked ask nothing', () => {
		expect(opens({ ...quiet, family: null })).toBeNull();
	});
});

describe('picked', () => {
	test('nothing kept is High on a capable desktop, and light where the machine is light', () => {
		expect(picked(presets, { effects: null, named: null }, false)).toEqual({ base: 'high', effects: presets.high });
		expect(picked(presets, { effects: null, named: null }, true)).toEqual({ base: 'light', effects: presets.light });
	});

	test('what was kept stands, its base the preset named, else the one it matches', () => {
		const custom = { ...presets.medium, shrubs: false };
		expect(picked(presets, { effects: custom, named: 'medium' }, true)).toEqual({ base: 'medium', effects: custom });
		expect(picked(presets, { effects: presets.medium, named: null }, true).base).toBe('medium');
	});
});

describe('pictureOf', () => {
	test('a preset with what lives set over it is that preset, since the dialog asks what lives on its own', () => {
		expect(pictureOf(presets.medium, presets)).toBe('medium');
		expect(pictureOf({ ...presets.light, trees: false, animals: false }, presets)).toBe('light');
	});

	test('any other knob set off the presets is Custom, whatever preset was picked last', () => {
		expect(pictureOf({ ...presets.light, grass: true, resolution: 1 }, presets)).toBe('custom');
		expect(pictureOf({ ...presets.high, foliage_reach_m: 500 }, presets)).toBe('custom');
	});
});

describe('drafted', () => {
	const now = { ...presets.high, animals: false };

	test('opens on what starts, and the GPU kept', () => {
		const lives = { trees: true, shrubs: true, flowers: true, animals: false };
		expect(drafted('weak', now, presets, { backend: 'webgl2' })).toEqual({ picture: 'high', ...lives, gpu: { backend: 'webgl2' } });
		expect(drafted(null, now, presets, {}).picture).toBe('high');
	});

	test('a picture set off the presets opens as Custom, never as a preset it is not', () => {
		const grassy = { ...presets.light, grass: true, resolution: 1 };
		expect(drafted('weak', grassy, presets, {}).picture).toBe('custom');
		expect(drafted(null, grassy, presets, {}).picture).toBe('custom');
	});

	test('after a start that never ran steady, or on safe, opens on the lightest picture, its trees kept', () => {
		const lightest = { picture: 'light' as const, trees: true, shrubs: false, flowers: false, animals: false, gpu: {} };
		expect(drafted('unsteady', now, presets, {})).toEqual(lightest);
		expect(drafted('safe', now, presets, {})).toEqual(lightest);
	});
});

describe('choices', () => {
	const now = { ...presets.medium, foliage_reach_m: 500 };
	const draft = { picture: 'custom' as const, trees: true, shrubs: true, flowers: true, animals: true, gpu: {} };

	test('nothing moved is no choice, and a picture off the presets stays', () => {
		expect(choices(now, draft, presets, 'kept')).toEqual([]);
		expect(choices(presets.medium, { ...draft, picture: 'medium' }, presets, 'kept')).toEqual([]);
	});

	test('a preset shown is what starts: answered over a kept picture that is not that preset, it is picked', () => {
		const grassy = { ...presets.light, grass: true, resolution: 1 };
		expect(choices(grassy, drafted('safe', grassy, presets, {}), presets, 'kept')).toEqual([
			{ pick: 'light' },
			{ choose: { ...presets.light, animals: false } }
		]);
		expect(choices(grassy, { ...drafted('weak', grassy, presets, {}), picture: 'light' }, presets, 'kept')).toEqual([{ pick: 'light' }]);
	});

	test('with no picture kept, the one it starts with is picked, so it is kept', () => {
		expect(choices(presets.light, { ...draft, ...livesOf(presets.light), picture: 'light' }, presets, 'none')).toEqual([{ pick: 'light' }]);
	});

	test('after the engine refused the start, the picture is written anew, so one it could not read is not kept', () => {
		expect(choices(presets.medium, { ...draft, picture: 'medium' }, presets, 'refused')).toEqual([{ pick: 'medium' }]);
		expect(choices(now, draft, presets, 'refused')).toEqual([{ choose: now }]);
	});

	test('a preset moved is picked, as the panel picks it', () => {
		expect(choices(now, { ...draft, ...livesOf(presets.light), picture: 'light' }, presets, 'kept')).toEqual([{ pick: 'light' }]);
	});

	test('what lives, set, goes over the preset picked', () => {
		expect(choices(now, { ...draft, picture: 'light', trees: false, shrubs: false, flowers: false }, presets, 'kept')).toEqual([
			{ pick: 'light' },
			{ choose: { ...presets.light, trees: false } }
		]);
		expect(choices(now, { ...draft, picture: 'light', shrubs: false, flowers: false }, presets, 'kept')).toEqual([{ pick: 'light' }]);
	});

	test('what lives, set alone, goes over the picture as it is', () => {
		expect(choices(now, { ...draft, flowers: false }, presets, 'kept')).toEqual([{ choose: { ...now, flowers: false } }]);
		const sparse = { ...presets.medium, shrubs: false };
		expect(choices(sparse, { ...draft, picture: 'medium', shrubs: false }, presets, 'kept')).toEqual([]);
		expect(choices(sparse, { ...draft, picture: 'medium', shrubs: false, animals: false }, presets, 'kept')).toEqual([
			{ choose: { ...sparse, animals: false } }
		]);
	});
});

describe('newGpu', () => {
	test('a field left out is the default, so asking for it is no change', () => {
		expect(newGpu({}, { power: 'high', backend: 'webgpu' })).toBe(false);
		expect(newGpu({}, { backend: 'webgl2' })).toBe(true);
		expect(newGpu({ power: 'low' }, {})).toBe(true);
	});
});
