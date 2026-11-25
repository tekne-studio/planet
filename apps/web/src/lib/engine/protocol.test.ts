import { describe, expect, test } from 'bun:test';
import { isPluginEvent, parseChoices, parseEvent, parsePet, type Effects, type Traits } from './protocol';

describe('parseEvent', () => {
	test('parses the known events', () => {
		expect(parseEvent('{"type":"ready","generator_version":1}')).toEqual({ type: 'ready', generator_version: 1 });
		expect(parseEvent('{"type":"mode_changed","mode":"fly"}')).toEqual({ type: 'mode_changed', mode: 'fly' });
		// The shell's own, as `touch.rs` writes it.
		expect(parseEvent('{"type":"stick","stick":{"center":[100.0,600.0],"at":[100.0,576.0],"reach":48}}')).toEqual({
			type: 'stick',
			stick: { center: [100, 600], at: [100, 576], reach: 48 }
		});
		expect(parseEvent('{"type":"stick","stick":null}')).toEqual({ type: 'stick', stick: null });
		expect(parseEvent('{"type":"home","place":"4-K7M42Q"}')).toEqual({ type: 'home', place: '4-K7M42Q' });
		expect(parseEvent('{"type":"choices","home":7,"favourites":[5]}')).toEqual({ type: 'choices', home: 7, favourites: [5] });
		const effects: Effects = {
			shadows: true,
			grass: true,
			clouds: false,
			resolution: 1,
			cloud_cover: 0.5,
			cloud_density: 1,
			wind_m_s: 14,
			cloud_change: 1,
			exposure: 1,
			bloom: 0.5,
			bloom_threshold: 1.1,
			haze: 1,
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
		expect(parseEvent(JSON.stringify({ type: 'effects_changed', effects }))).toEqual({ type: 'effects_changed', effects });
		expect(parseEvent('{"type":"session","status":"online","session":3,"level":"builder"}')).toEqual({
			type: 'session',
			status: 'online',
			session: 3,
			level: 'builder'
		});
		expect(parseEvent('{"type":"peers","peers":[{"session":2,"name":"Ada","visitor":false}]}')).toEqual({
			type: 'peers',
			peers: [{ session: 2, name: 'Ada', visitor: false }]
		});
		expect(parseEvent('{"type":"statement","plugins":[{"name":"chat","version":1}]}')).toEqual({
			type: 'statement',
			plugins: [{ name: 'chat', version: 1 }]
		});
		expect(parseEvent('{"type":"anchors","anchors":[{"session":2,"x":0.5,"y":0.4,"distance_m":3}]}')).toEqual({
			type: 'anchors',
			anchors: [{ session: 2, x: 0.5, y: 0.4, distance_m: 3 }]
		});
		expect(parseEvent('{"type":"palette","colors":["#f2f0eb"]}')).toEqual({ type: 'palette', colors: ['#f2f0eb'] });
		// What building says is its plugin's, and the core no longer knows it.
		expect(parseEvent('{"type":"tool_changed","tool":"paint","paint":4,"platform":16}')).toBeNull();
	});

	test('passes a plugin event on as the plugin wrote it', () => {
		const said = parseEvent('{"type":"chat.said","session":2,"scope":"near","text":"hi","place":"4-K7M42Q"}');
		expect(said).toEqual({ type: 'chat.said', session: 2, scope: 'near', text: 'hi', place: '4-K7M42Q' });
		expect(said && isPluginEvent(said)).toBe(true);
		const ready = parseEvent('{"type":"ready","generator_version":1}');
		expect(ready && isPluginEvent(ready)).toBe(false);
		expect(parseEvent('{"type":"Chat.Said"}')).toBeNull();
		expect(parseEvent('{"type":"chat."}')).toBeNull();
	});

	test('ignores unknown types and malformed payloads', () => {
		expect(parseEvent('{"type":"from_the_future"}')).toBeNull();
		expect(parseEvent('{"no_type":true}')).toBeNull();
		expect(parseEvent('null')).toBeNull();
		expect(parseEvent('not json')).toBeNull();
	});
});

describe('parseChoices', () => {
	test('reads what a browser kept of a visitor\'s choices', () => {
		expect(parseChoices('{"home":7,"favourites":[5,9]}')).toEqual({ home: 7, favourites: [5, 9] });
		expect(parseChoices('{"home":null,"favourites":[]}')).toEqual({ home: null, favourites: [] });
	});

	test('nothing kept, or what is no choices, is null', () => {
		expect(parseChoices(null)).toBeNull();
		expect(parseChoices('not json')).toBeNull();
		expect(parseChoices('{"home":"7","favourites":[]}')).toBeNull();
		expect(parseChoices('{"home":null,"favourites":[0]}')).toBeNull();
		expect(parseChoices('{"home":null}')).toBeNull();
	});
});

describe('parsePet', () => {
	const spot = { sector: 4, u: 10.5, v: 20.25, heading: 1, at_ms: 1_800_000_000_000 };

	test('reads what a browser kept of a visitor\'s pet', () => {
		const kept = JSON.stringify({ number: 35184372088839, name: 'Dapple', spot, owner: 'Ana', mine: true, session: 3 });
		expect(parsePet(kept)).toEqual({ number: 35184372088839, name: 'Dapple', spot, owner: '', mine: true });
		expect(parsePet(JSON.stringify({ number: 7, name: '', spot, from: spot }))?.from).toEqual(spot);
		expect(parsePet(JSON.stringify({ number: 7, name: '', spot, from: spot, from_mps: 6.5 }))?.from_mps).toBe(6.5);
		expect(parsePet(JSON.stringify({ number: 7, name: '', spot, from: spot, from_mps: 'fast' }))?.from_mps).toBeUndefined();
		// Wings are the traits' own: a field beside them is left out.
		expect(parsePet(JSON.stringify({ number: 7, name: '', spot, wings: 'black' }))).not.toHaveProperty('wings');
	});

	test('keeps the traits it was born with, as the engine wrote them', () => {
		const white: [number, number, number] = [0.74, 0.72, 0.67];
		const traits: Traits = {
			coat: 'white',
			hair: white,
			mane: white,
			points: white,
			muzzle: white,
			hoof: white,
			face: '',
			socks: [0, 0, 0, 0],
			blots: [{ y: 1.2, z: -0.4, radius_m: 0.3 }],
			size: 0.7016232909552723,
			build: 1.1388859953865407,
			mane_m: 0.13359243397203852,
			mane_falls: true,
			tail_m: 0.584879338200169,
			feathers: false,
			wings: true,
			voice_cents: -42.5
		};
		const pet = { number: 7, name: '', spot };
		expect(parsePet(JSON.stringify({ ...pet, traits }))?.traits).toEqual(traits);
		// Kept before a voice was drawn: still read, the world fills it.
		const { voice_cents, ...unvoiced } = traits;
		expect(voice_cents).toBe(-42.5);
		expect(parsePet(JSON.stringify({ ...pet, traits: unvoiced }))?.traits).toEqual(unvoiced);
		// What the engine could not read is left for the world to draw again.
		const { blots, ...unblotted } = traits;
		const patches = { ...unblotted, patches: blots };
		for (const broken of [{ ...traits, socks: [0, 1] }, { ...traits, size: 'big' }, { ...traits, wings: undefined }, { ...traits, voice_cents: 'low' }, patches, 'bay']) {
			const read = parsePet(JSON.stringify({ ...pet, traits: broken }));
			expect(read?.number).toBe(7);
			expect(read).not.toHaveProperty('traits');
		}
	});

	test('nothing kept, or what is no pet, is null', () => {
		expect(parsePet(null)).toBeNull();
		expect(parsePet('null')).toBeNull();
		expect(parsePet('not json')).toBeNull();
		expect(parsePet(JSON.stringify({ number: 0, name: '', spot }))).toBeNull();
		expect(parsePet(JSON.stringify({ number: 7, name: '', spot: { ...spot, u: 'x' } }))).toBeNull();
		expect(parsePet(JSON.stringify({ number: 7, name: '', spot, from: {} }))).toBeNull();
	});
});
