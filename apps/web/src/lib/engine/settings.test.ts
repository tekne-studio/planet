import { describe, expect, test } from 'bun:test';
import type { Effects, Presets } from './protocol';
import { gameDefaults, hiddenMarkers, loudnessDefaults, matching, presetName, readGame, readLoudness } from './settings';

const high: Effects = {
	shadows: true,
	grass: true,
	clouds: true,
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
	light: { ...high, shadows: false, resolution: 0.5 },
	medium: { ...high, undergrowth: 0.5 },
	high
};

describe('matching', () => {
	test('names the preset the effects are, field by field', () => {
		expect(matching(high, presets)).toBe('high');
		expect(matching({ ...high, undergrowth: 0.5 }, presets)).toBe('medium');
	});

	test('a knob off every preset is custom', () => {
		expect(matching({ ...high, haze: 2 }, presets)).toBeNull();
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
