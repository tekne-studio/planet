import { describe, expect, test } from 'bun:test';
import { crossed, readWays } from './ways';

describe('a thumb on the cross', () => {
	// A cross 120 across: its cells are 40, its middle rocks within 20.
	const size = 120;

	test('presses the arm it is on', () => {
		expect(crossed(50, 0, size)).toEqual(['right']);
		expect(crossed(0, -50, size)).toEqual(['up']);
		expect(crossed(-50, 5, size)).toEqual(['left']);
		expect(crossed(5, 50, size)).toEqual(['down']);
	});

	test('holds both arms beside the corner it leans into', () => {
		expect(crossed(40, -40, size)).toEqual(['right', 'up']);
		expect(crossed(-40, -40, size)).toEqual(['up', 'left']);
		expect(crossed(-40, 40, size)).toEqual(['left', 'down']);
		expect(crossed(40, 40, size)).toEqual(['down', 'right']);
	});

	test('turns from an arm to its corner an eighth of a turn round', () => {
		const at = (degrees: number) => {
			const turn = (degrees * Math.PI) / 180;
			return crossed(50 * Math.cos(turn), -50 * Math.sin(turn), size);
		};
		expect(at(22)).toEqual(['right']);
		expect(at(23)).toEqual(['right', 'up']);
		expect(at(67)).toEqual(['right', 'up']);
		expect(at(68)).toEqual(['up']);
		expect(at(-23)).toEqual(['down', 'right']);
	});

	test('presses nothing within half a cell of the middle, and anything past its edge', () => {
		expect(crossed(0, 0, size)).toEqual([]);
		expect(crossed(14, -14, size)).toEqual([]);
		expect(crossed(21, 0, size)).toEqual(['right']);
		expect(crossed(400, 10, size)).toEqual(['right']);
	});
});

test('a new person meets the cross, and the stick once chosen', () => {
	expect(readWays(null)).toBe('cross');
	expect(readWays('nonsense')).toBe('cross');
	expect(readWays('stick')).toBe('stick');
});
