import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { Fingers } from './fingers';

// What lies under a finger, as `document.elementFromPoint` finds it: the
// key of a button that rolls, by where the finger is.
let under: Record<string, string> = {};
const page = globalThis as unknown as { window: EventTarget; document: unknown };
beforeEach(() => {
	page.window = new EventTarget();
	page.document = {
		elementFromPoint: (x: number, y: number) => {
			const key = under[`${x},${y}`];
			return key ? { closest: () => ({ dataset: { key } }) } : null;
		}
	};
});
afterEach(() => {
	under = {};
});

function pad() {
	const pressed: string[] = [];
	const fingers = new Fingers((code, down) => pressed.push(`${code} ${down ? 'down' : 'up'}`));
	return { fingers, pressed, stop: fingers.listen() };
}

/** A finger landing on the button `attrs` makes, by its pointer. */
function land(attrs: ReturnType<Fingers['hold']>, finger: number) {
	const target = { setPointerCapture() {} };
	attrs.onpointerdown!({ pointerId: finger, preventDefault() {}, currentTarget: target } as never);
}

function slide(attrs: ReturnType<Fingers['hold']>, finger: number, x: number, y: number) {
	attrs.onpointermove!({ pointerId: finger, clientX: x, clientY: y } as never);
}

/** A finger lifting, wherever it is: the page hears it. */
function lift(finger: number) {
	page.window.dispatchEvent(Object.assign(new Event('pointerup'), { pointerId: finger }));
}

describe('fingers on buttons that stand in for keys', () => {
	test('two fingers hold two keys at once, each until it lifts', () => {
		const { fingers, pressed } = pad();
		land(fingers.hold('KeyZ', true), 1);
		land(fingers.hold('KeyX', true), 2);
		expect(fingers.holds('KeyZ') && fingers.holds('KeyX')).toBe(true);
		lift(1);
		expect(pressed).toEqual(['KeyZ down', 'KeyX down', 'KeyZ up']);
		expect(fingers.holds('KeyX')).toBe(true);
	});

	test('a key two fingers hold goes up with the last', () => {
		const { fingers, pressed } = pad();
		const a = fingers.hold('KeyZ', true);
		land(a, 1);
		land(a, 2);
		lift(1);
		expect(pressed).toEqual(['KeyZ down']);
		lift(2);
		expect(pressed).toEqual(['KeyZ down', 'KeyZ up']);
	});

	test('a thumb rolls from B to A: A goes down before B goes up', () => {
		const { fingers, pressed } = pad();
		const b = fingers.hold('KeyX', true);
		land(b, 1);
		under['300,700'] = 'KeyZ';
		slide(b, 1, 300, 700);
		expect(pressed).toEqual(['KeyX down', 'KeyZ down', 'KeyX up']);
		// Off every button it keeps what it held.
		slide(b, 1, 10, 10);
		lift(1);
		expect(pressed).toEqual(['KeyX down', 'KeyZ down', 'KeyX up', 'KeyZ up']);
	});

	test('a button that does not roll keeps its finger wherever it slides', () => {
		const { fingers, pressed } = pad();
		const start = fingers.hold('Enter');
		land(start, 1);
		under['300,700'] = 'KeyZ';
		slide(start, 1, 300, 700);
		expect(pressed).toEqual(['Enter down']);
	});

	test('what is held when the buttons leave is let go, and no lift comes after', () => {
		const { fingers, pressed, stop } = pad();
		land(fingers.hold('Space'), 1);
		land(fingers.hold('KeyC'), 2);
		stop();
		lift(1);
		expect(pressed).toEqual(['Space down', 'KeyC down', 'Space up', 'KeyC up']);
		expect(fingers.holds('Space')).toBe(false);
	});

	test('a finger on the cross presses what is under it as it moves, two at a corner', () => {
		const { fingers, pressed } = pad();
		// A cross 90 across whose middle is at 100, 100: right of it is
		// right, over it is up, and the finger holds the keys it says.
		const box = { left: 55, top: 55, width: 90, height: 90 };
		const target = { setPointerCapture() {}, getBoundingClientRect: () => box };
		const ways = (x: number, y: number) =>
			Math.abs(x) < 10 && Math.abs(y) < 10
				? []
				: [...(x > 10 ? ['ArrowRight'] : []), ...(y < -10 ? ['ArrowUp'] : [])];
		const cross = fingers.steer(ways);
		const at = (x: number, y: number) =>
			({
				pointerId: 1,
				clientX: x,
				clientY: y,
				preventDefault() {},
				currentTarget: target
			}) as never;
		cross.onpointerdown!(at(140, 100));
		expect(fingers.holds('ArrowRight')).toBe(true);
		cross.onpointermove!(at(140, 60));
		expect(fingers.holds('ArrowRight') && fingers.holds('ArrowUp')).toBe(true);
		cross.onpointermove!(at(100, 60));
		// Back to the middle, nothing; off the cross, what is under it.
		cross.onpointermove!(at(100, 100));
		cross.onpointermove!(at(300, 100));
		lift(1);
		expect(pressed).toEqual([
			'ArrowRight down',
			'ArrowUp down',
			'ArrowRight up',
			'ArrowUp up',
			'ArrowRight down',
			'ArrowRight up'
		]);
	});

	test('a key the cross and a button both hold goes up with the last finger', () => {
		const { fingers, pressed } = pad();
		const target = {
			setPointerCapture() {},
			getBoundingClientRect: () => ({ left: 0, top: 0, width: 90, height: 90 })
		};
		fingers.steer(() => ['ArrowLeft', 'ArrowDown']).onpointerdown!({
			pointerId: 1,
			clientX: 0,
			clientY: 90,
			preventDefault() {},
			currentTarget: target
		} as never);
		land(fingers.hold('ArrowLeft'), 2);
		lift(1);
		expect(pressed).toEqual(['ArrowLeft down', 'ArrowDown down', 'ArrowDown up']);
		lift(2);
		expect(pressed.at(-1)).toBe('ArrowLeft up');
	});

	test('a tap presses once', () => {
		const { fingers, pressed } = pad();
		fingers.tap('Escape').onclick!({} as never);
		expect(pressed).toEqual(['Escape down', 'Escape up']);
	});
});
