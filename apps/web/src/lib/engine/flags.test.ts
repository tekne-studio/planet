import { describe, expect, test } from 'bun:test';
import { answered, readFlags, flagsIn } from './flags';

describe('readFlags', () => {
	test('an address without flags sets none', () => {
		expect(readFlags('?lang=pt')).toEqual({ said: [], safe: false, readout: null });
	});

	test('a flag is on bare or as on, and off as off', () => {
		expect(readFlags('?safe').safe).toBe(true);
		expect(readFlags('?safe=on').safe).toBe(true);
		expect(readFlags('?safe=off').safe).toBe(false);
		expect(readFlags('?readout=on').readout).toBe(true);
		expect(readFlags('?readout=off').readout).toBe(false);
	});

	test('any other value is no flag, and is not listed', () => {
		const flags = readFlags('?safe=false&readout=1');
		expect(flags).toEqual({ said: [], safe: false, readout: null });
	});

	test('each flag read is listed as the address wrote it', () => {
		expect(readFlags('?readout=on&lang=pt&safe').said).toEqual(['readout=on', 'safe']);
		expect(readFlags('?safe=off').said).toEqual(['safe=off']);
	});

	test('a parameter the start does not read is no flag', () => {
		expect(readFlags('?plants=off&keep&power=low').said).toEqual([]);
	});
});

describe('answered', () => {
	test('drops safe and keeps the rest, readout among it', () => {
		expect(answered('?safe&lang=pt&readout')).toBe('?lang=pt&readout');
		expect(answered('?safe=on&seed=a%20b')).toBe('?seed=a%20b');
		expect(answered('?safe')).toBe('');
		expect(answered('')).toBe('');
	});
});

describe('flagsIn', () => {
	test('keeps the flags as written and drops the rest', () => {
		expect(flagsIn('?seed=ab&safe&readout=on&shape=earth').toString()).toBe('safe=&readout=on');
		expect(flagsIn('?seed=ab&safe=maybe').toString()).toBe('');
	});
});
