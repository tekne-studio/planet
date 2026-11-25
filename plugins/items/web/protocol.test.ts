import { describe, expect, test } from 'bun:test';
import { parsePicked, parseReset } from './protocol';

const part = { id: 7, item: 'cabinet', at: [0, 0, 0], rotation: [0, 0, 0], scale: [1, 1, 1], solid: true, shine: 0, reach_m: 0, lamp: false };

describe('a part picked', () => {
	test('says who placed it and who set it last, a visitor as one', () => {
		const picked = parsePicked({ ...part, placed_by: { name: 'ada', visitor: false }, set_by: { name: 'bea', visitor: true } });
		expect(picked?.placed_by).toEqual({ name: 'ada', visitor: false });
		expect(picked?.set_by).toEqual({ name: 'bea', visitor: true });
	});

	test('says nobody where nobody is known', () => {
		const picked = parsePicked({ ...part, placed_by: 'ada' });
		expect(picked).not.toBeNull();
		expect(picked?.placed_by).toBeUndefined();
		expect(picked?.set_by).toBeUndefined();
	});
});

describe('a reset held', () => {
	test('says how far its ring has closed and where, and nothing once it goes', () => {
		expect(parseReset({ type: 'items.reset', closed: 0.5, x: 0.5, y: 0.4 })).toEqual({ closed: 0.5, x: 0.5, y: 0.4 });
		expect(parseReset({ type: 'items.reset', closed: null, x: 0.5, y: 0.4 })).toBeNull();
	});
});
