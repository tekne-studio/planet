import { describe, expect, test } from 'bun:test';
import { layerProps, type Reads, type Readings, type Seam } from './plugin';

// Svelte tracks what a prop read reads: these hold that reading the seam
// reads no reading, and that a reading reads only its own, so an effect
// over the seam never reruns for a frame's speed.

const seam = {} as Seam;

/** Reads that give these readings, and note each one asked in `asked`. */
function readsOf(readings: Readings, asked: string[] = []): Reads {
	return Object.fromEntries(Object.entries(readings).map(([key, value]) => [key, () => (asked.push(key), value)])) as unknown as Reads;
}

function counted(readings: Readings) {
	const asked: string[] = [];
	return { asked, props: layerProps(seam, readsOf(readings, asked)) };
}

const readings: Readings = {
	online: true,
	me: 7,
	builds: false,
	touch: false,
	palette: ['#ffffff'],
	peers: [],
	anchors: [],
	mode: 'walk',
	lent: false,
	speed_mps: 3,
	altitude_m: 0,
	pace: null
};

describe('layerProps', () => {
	test('the seam is a value: reading it reads no reading', () => {
		const { asked, props } = counted(readings);
		expect(props.seam).toBe(seam);
		expect(asked).toEqual([]);
	});

	test('a reading reads only its own', () => {
		const { asked, props } = counted(readings);
		expect(props.mode).toBe('walk');
		expect(props.speed_mps).toBe(3);
		expect(asked).toEqual(['mode', 'speed_mps']);
	});

	test('the same object reads the reading as it is now', () => {
		let speed = 1;
		const props = layerProps(seam, { ...readsOf(readings), speed_mps: () => speed });
		speed = 2;
		expect(props.speed_mps).toBe(2);
	});

	test('every prop is there, for a spread to hand on', () => {
		const { props } = counted(readings);
		expect(Object.keys(props).sort()).toEqual(['seam', ...Object.keys(readings)].sort());
	});
});
