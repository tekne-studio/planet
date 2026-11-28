import { describe, expect, test } from 'bun:test';
import type { Effects, Presets } from './protocol';
import { filled, gameChosen, gameDefaults, hiddenMarkers, living, loudnessDefaults, matching, presetName, readGame, readLoudness, readoutOn, shows } from './settings';

const high: Effects = {
	shadows: true,
	shadows_fade: true,
	grass: true,
	clouds: true,
	trees: true,
	shrubs: true,
	flowers: true,
	foliage_shadows: true,
	animals: true,
	resolution: 1,
	cloud_cover: 0.5,
	cloud_density: 0.1,
	wind_m_s: 14,
	cloud_change: 1,
	exposure: 1,
	bloom: 0.7,
	bloom_threshold: 0.3,
	haze: 0.8,
	water_clarity: 3,
	video_play_m: 128,
	picture_reach_m: 256,
	model_reach_m: 128,
	foliage_near_m: 90,
	foliage_mid_m: 260,
	foliage_reach_m: 600,
	undergrowth: 1,
	tone_map: 'aces'
};
const presets: Presets = {
	light: { ...high, shadows: false, grass: false, shrubs: false, flowers: false, foliage_shadows: false, resolution: 0.5, undergrowth: 0.5 },
	medium: { ...high, shadows_fade: false, foliage_shadows: false, undergrowth: 0.5 },
	high
};

describe('matching', () => {
	test('names the preset the effects are, field by field', () => {
		expect(matching(high, presets)).toBe('high');
		expect(matching({ ...high, shadows_fade: false, foliage_shadows: false, undergrowth: 0.5 }, presets)).toBe('medium');
	});

	test('a knob off every preset is custom', () => {
		expect(matching({ ...high, haze: 2 }, presets)).toBeNull();
	});
});

describe('life', () => {
	test('the world has life while what is on says gaia', () => {
		expect(living([{ name: 'chat' }, { name: 'gaia' }])).toBe(true);
		expect(living([{ name: 'chat' }])).toBe(false);
		expect(living([])).toBe(false);
	});

	test('trees, shrubs, flowers and animals show only while the world has life, every other knob always', () => {
		const keys = Object.keys(high) as (keyof Effects)[];
		expect(keys.filter((key) => !shows(key, false)).sort()).toEqual(['animals', 'flowers', 'shrubs', 'trees']);
		expect(keys.filter((key) => !shows(key, true))).toEqual([]);
	});
});

describe('presetName', () => {
	test('reads a kept preset, and nothing else', () => {
		expect(presetName('medium')).toBe('medium');
		expect(presetName('ultra')).toBeNull();
		expect(presetName(null)).toBeNull();
	});
});

describe('readGame', () => {
	test('a choice not kept is its default', () => {
		expect(readGame(null)).toEqual(gameDefaults);
		expect(readGame('{"place":false}')).toEqual({ ...gameDefaults, place: false });
	});

	test('keeps what was chosen, and drops what was kept wrong', () => {
		expect(readGame('{"place":"no","pet":false}')).toEqual({ ...gameDefaults, pet: false });
		expect(readGame('not json')).toEqual(gameDefaults);
	});
});

describe('the readout', () => {
	const off = gameDefaults;

	test('the address says it for the visit, else the choice kept', () => {
		expect(readoutOn(null, off)).toBe(false);
		expect(readoutOn(null, { ...off, readout: true })).toBe(true);
		expect(readoutOn(true, off)).toBe(true);
		expect(readoutOn(false, { ...off, readout: true })).toBe(false);
	});

	test('another choice in the panel keeps the readout as kept, the address still saying it', () => {
		const shown = { ...off, readout: true };
		expect(gameChosen({ ...shown, place: false }, true, off)).toEqual({ game: { ...off, place: false }, said: true });
	});

	test('the readout turned is kept, and the address lets go', () => {
		expect(gameChosen({ ...off, readout: false }, true, off)).toEqual({ game: off, said: null });
		expect(gameChosen({ ...off, readout: true }, false, off)).toEqual({ game: { ...off, readout: true }, said: null });
		expect(gameChosen({ ...off, readout: true }, null, off)).toEqual({ game: { ...off, readout: true }, said: null });
	});
});

describe('hiddenMarkers', () => {
	test('a fresh person hides the favourites alone', () => {
		expect(hiddenMarkers(gameDefaults)).toEqual(['favourite']);
	});

	test('names every kind switched off, and none when all are on', () => {
		expect(hiddenMarkers({ ...gameDefaults, pet: false, home: false })).toEqual(['pet', 'home', 'favourite']);
		expect(hiddenMarkers({ ...gameDefaults, favourite: true })).toEqual([]);
	});
});

describe('readLoudness', () => {
	test('a level not kept is its default', () => {
		expect(readLoudness(null)).toEqual(loudnessDefaults);
	});

	test('keeps a level from 0 to 1, and drops one outside or of another type', () => {
		const kept = readLoudness('{"master":0.5,"muted":true,"steps":0,"calls":1.5,"games":"loud"}');
		expect(kept).toEqual({ ...loudnessDefaults, master: 0.5, muted: true, steps: 0 });
	});
});

describe('filled', () => {
	/** A record kept before a field was: the preset's whole, less the switches added since. */
	const older = (effects: Effects) => {
		const { shadows_fade, trees, shrubs, flowers, foliage_shadows, animals, ...rest } = effects;
		void [shadows_fade, trees, shrubs, flowers, foliage_shadows, animals];
		return JSON.stringify(rest);
	};

	test("Light as an older version kept it, none of its undergrowth drawn, draws as Light: the trees alone", () => {
		const kept = filled(older({ ...presets.light, undergrowth: 0 }), presets, 'light');
		expect(kept).toEqual({ ...presets.light, undergrowth: 0 });
		expect(kept && [kept.trees, kept.shrubs, kept.flowers]).toEqual([true, false, false]);
	});

	test("a field a kept record lacks is the named preset's", () => {
		const kept = filled(older(presets.light), presets, 'light');
		expect(kept).toEqual(presets.light);
		expect(kept?.foliage_shadows).toBe(false);
	});

	test('with no preset named, the one the record matches fills it', () => {
		const kept = filled(older(presets.medium), presets, null);
		expect(kept).toEqual(presets.medium);
		expect(kept?.shadows_fade).toBe(false);
	});

	test('a record off every preset is filled from the light one, never High', () => {
		const custom = { ...high, exposure: 2 };
		const kept = filled(older(custom), presets, null);
		expect(kept?.foliage_shadows).toBe(presets.light.foliage_shadows);
		expect(kept?.exposure).toBe(2);
	});

	test('what the record holds stands over the preset', () => {
		const kept = filled(JSON.stringify({ ...presets.high, trees: false }), presets, 'high');
		expect(kept?.trees).toBe(false);
	});

	test('a field the engine does not know, or of another type, is dropped', () => {
		const kept = filled(JSON.stringify({ ...presets.medium, trees: 'no', retired: 1 }), presets, 'medium');
		expect(kept).toEqual(presets.medium);
		expect(kept && 'retired' in kept).toBe(false);
	});

	test('a value the engine does not read is dropped: a tone map it does not name, a number not finite', () => {
		// JSON has no Infinity: 1e999 parses to it, and goes back out as null.
		const kept = filled('{"tone_map":"filmic","exposure":1e999,"bloom":0.25}', presets, 'medium');
		expect(kept).toEqual({ ...presets.medium, bloom: 0.25 });
		expect(filled('{"tone_map":"agx"}', presets, 'medium')?.tone_map).toBe('agx');
	});

	test('nothing kept, or kept wrong, is nothing', () => {
		expect(filled(null, presets, 'high')).toBeNull();
		expect(filled('{', presets, 'high')).toBeNull();
		expect(filled('[1]', presets, 'high')).toBeNull();
	});
});
