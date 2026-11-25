import { expect, test } from 'bun:test';
import { termsGiven } from './rom';

test('takes the terms its maker gives: a licence, a file of its own, or its rights reserved', () => {
	expect(termsGiven({ licence: 'MIT' })).toEqual({ licence: 'MIT' });
	expect(termsGiven({ 'licence-file': 'LICENSE' })).toEqual({ file: 'LICENSE' });
	expect(termsGiven({ rights: 'reserved' })).toEqual({ rights: 'reserved' });
});

test('refuses none, two, one given empty beside another, and rights of another kind', () => {
	for (const options of [{}, { licence: 'MIT', rights: 'reserved' }, { licence: '', rights: 'reserved' }, { licence: '' }, { rights: 'shared' }])
		expect(() => termsGiven(options), JSON.stringify(options)).toThrow('one of --licence');
});
