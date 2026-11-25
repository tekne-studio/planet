import { describe, expect, test } from 'bun:test';
import { parsePicked } from './protocol';

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
