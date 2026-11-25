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

	test('a tap presses once', () => {
		const { fingers, pressed } = pad();
		fingers.tap('Escape').onclick!({} as never);
		expect(pressed).toEqual(['Escape down', 'Escape up']);
	});
});
