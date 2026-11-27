import { describe, expect, test } from 'bun:test';
import { linkUp, reloadOnce } from './outdated';

function tab() {
	const kept = new Map<string, string>();
	return () => ({
		getItem: (key: string) => kept.get(key) ?? null,
		setItem: (key: string, value: string) => void kept.set(key, value),
		removeItem: (key: string) => void kept.delete(key)
	});
}

describe('reloadOnce', () => {
	test('reloads once, and a page that comes back refused stays', () => {
		const store = tab();
		expect(reloadOnce(store)).toBe(true);
		expect(reloadOnce(store)).toBe(false);
		expect(reloadOnce(store)).toBe(false);
	});

	test('a link that came up lets the next update reload again', () => {
		const store = tab();
		expect(reloadOnce(store)).toBe(true);
		linkUp(store);
		expect(reloadOnce(store)).toBe(true);
	});

	test('blocked storage never reloads, since nothing could stop a loop', () => {
		const blocked = () => {
			throw new Error('SecurityError');
		};
		expect(reloadOnce(blocked)).toBe(false);
		expect(() => linkUp(blocked)).not.toThrow();
		const full = () => ({
			...tab()(),
			setItem: () => {
				throw new Error('QuotaExceededError');
			}
		});
		expect(reloadOnce(full)).toBe(false);
	});
});
