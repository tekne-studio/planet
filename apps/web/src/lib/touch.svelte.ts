/**
 * Whether this is a touch screen: a phone or a tablet, held and touched.
 * Its primary pointer is coarse or cannot hover, and the screen takes
 * fingers; a laptop with a touch screen keeps its mouse and is not one.
 *
 * A touch screen walks a world: a stick, a drag that looks, buttons to jump
 * and fly. It offers no founding and no building, and its layout is the
 * design system's touch tokens (`tokens.css`).
 */
import { browser } from '$app/environment';
import { on } from 'svelte/events';
import { MediaQuery, createSubscriber } from 'svelte/reactivity';

const coarse = new MediaQuery('pointer: coarse');
const hoverless = new MediaQuery('hover: none');

export const touch = {
	get current(): boolean {
		return browser && navigator.maxTouchPoints > 0 && (coarse.current || hoverless.current);
	}
};

const viewport = browser ? window.visualViewport : null;
const viewed = createSubscriber((update) => {
	if (!viewport) return;
	const stops = [on(viewport, 'resize', update), on(viewport, 'scroll', update)];
	return () => stops.forEach((stop) => stop());
});

/**
 * The part of a touch screen in view over its keyboard, in CSS pixels from
 * the top of the page. The phone's keyboard shrinks the visual viewport and
 * the browser pans it to the field typed in, which would carry the bar and
 * the thumbs off the screen: the stage keeps to this box instead, so every
 * control stays in view above the keys. Null while the whole page is in
 * view, and on a page zoomed in, which is the person's own look.
 */
export const visible = {
	get current(): { top: number; height: number } | null {
		viewed();
		if (!touch.current || !viewport || viewport.scale !== 1) return null;
		const { offsetTop: top, height } = viewport;
		return top === 0 && height >= innerHeight ? null : { top, height };
	}
};
