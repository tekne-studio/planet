// The four ways a touch screen's pad presses, by the stick or by the cross,
// and how a thumb on the cross picks them.

/** The four ways the stick presses, as the console knows them. */
export type Way = 'up' | 'down' | 'left' | 'right';

/**
 * The key a touch screen's cross presses for each way, by the name the web
 * gives it: the arrows, which press the four ways a keyboard's stick does.
 */
export const WAY_KEYS = {
	up: 'ArrowUp',
	down: 'ArrowDown',
	left: 'ArrowLeft',
	right: 'ArrowRight'
} as const satisfies Record<Way, string>;

/** What presses the four ways on a touch screen's pad: the stick or the cross, as the person chose. */
export type Ways = 'stick' | 'cross';

/** The cross first: the console's own shape. */
export function readWays(kept: string | null): Ways {
	return kept === 'stick' ? 'stick' : 'cross';
}

/** The ways each eighth of a turn presses, from the right turning up: an arm, then the corner beside it, which holds both. */
const EIGHTHS: readonly (readonly Way[])[] = [
	['right'],
	['right', 'up'],
	['up'],
	['up', 'left'],
	['left'],
	['left', 'down'],
	['down'],
	['down', 'right']
];

/**
 * The ways a thumb presses on a cross `size` across, `x` right and `y`
 * down of its middle: the arm it is on, or both arms beside the corner it
 * leans into, an eighth of a turn each. The cross is three cells across,
 * and within half a cell of its middle it presses nothing, as a cross
 * rocks on nothing there.
 */
export function crossed(x: number, y: number, size: number): readonly Way[] {
	if (Math.hypot(x, y) < size / 6) return [];
	const eighth = Math.round(Math.atan2(-y, x) / (Math.PI / 4));
	return EIGHTHS[(eighth + 8) % 8];
}
