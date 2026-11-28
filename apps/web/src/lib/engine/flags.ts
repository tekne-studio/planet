/**
 * What an address may say of the start, read before the engine starts: its
 * flags, each on bare or as `on`, off as `off`; any other value is no flag.
 *
 * | Parameter | What it does                                                                     |
 * | --------- | -------------------------------------------------------------------------------- |
 * | `safe`    | opens the graphics dialog before the engine starts; gone from the address once answered |
 * | `readout` | shows on the page what the machine says and how far the start went, or hides it, for the visit |
 */
const names = ['safe', 'readout'] as const;
type Name = (typeof names)[number];

export interface Flags {
	/** Each flag read, as the address wrote it: what the readout lists. */
	said: string[];
	safe: boolean;
	/** Null where the address says nothing: the readout is then the choice kept in Settings. */
	readout: boolean | null;
}

export function readFlags(search: string): Flags {
	const flags: Flags = { said: [], safe: false, readout: null };
	for (const [name, value] of new URLSearchParams(search)) {
		const on = onOff(value);
		if (!isName(name) || on === null) continue;
		flags[name] = on;
		flags.said.push(value === '' ? name : `${name}=${value}`);
	}
	return flags;
}

/** The query once the dialog `safe` opened is answered: the rest stays, `readout` among it. */
export function answered(search: string): string {
	// Piece by piece, so what stays is as the address wrote it.
	const rest = search
		.replace(/^\?/, '')
		.split('&')
		.filter((piece) => piece && new URLSearchParams(piece).keys().next().value !== 'safe');
	return rest.length ? `?${rest.join('&')}` : '';
}

/** The flags of an address, as written: what another address of this page carries. */
export function flagsIn(search: string): URLSearchParams {
	const kept = new URLSearchParams();
	for (const [name, value] of new URLSearchParams(search)) {
		if (isName(name) && onOff(value) !== null) kept.append(name, value);
	}
	return kept;
}

function isName(name: string): name is Name {
	return (names as readonly string[]).includes(name);
}

function onOff(value: string): boolean | null {
	if (value === '' || value === 'on') return true;
	return value === 'off' ? false : null;
}
