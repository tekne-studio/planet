import type { HTMLAttributes, HTMLButtonAttributes } from 'svelte/elements';
import { SvelteMap } from 'svelte/reactivity';

/**
 * The keys that buttons on a touch screen press, by the name the web gives
 * each key. A finger holds the keys of the button it lands on until it
 * lifts or the page takes them: one for a button, one or two on a cross,
 * by where on it the finger is. Two fingers hold what both hold, and a key
 * two fingers hold goes up with the last. A finger that slides onto
 * another button that rolls lets the first go and holds the other, as a
 * thumb rolls across a game's A and B; off every button it keeps what it
 * held. On a cross it presses what is under it as it moves, off the cross
 * too. A press keeps the focus where it was, so the canvas holds on to the
 * finger on the stick. A finger lifts wherever it is, off its button or
 * off one gone, and what is held when the buttons leave the screen is let
 * go: no key outlives its finger or its button.
 */
export class Fingers {
	/** The keys each finger holds, by the pointer it is; read where a button shows it held. */
	readonly #held = new SvelteMap<number, readonly string[]>();
	readonly #onkey: (code: string, down: boolean) => void;

	constructor(onkey: (code: string, down: boolean) => void) {
		this.#onkey = onkey;
	}

	/** A button that holds `code` while a finger is on it; one that `rolls` takes a finger sliding over from another that rolls. */
	hold(code: string, rolls = false): HTMLButtonAttributes {
		return {
			'data-key': code,
			'data-rolls': rolls || undefined,
			onpointerdown: (event) => {
				event.preventDefault();
				event.currentTarget.setPointerCapture(event.pointerId);
				this.#press(event.pointerId, [code]);
			},
			onpointermove: (event) => {
				if (!rolls || !this.#held.has(event.pointerId)) return;
				const under = document
					.elementFromPoint(event.clientX, event.clientY)
					?.closest<HTMLElement>('[data-rolls]');
				const next = under?.dataset.key;
				if (next && !this.#held.get(event.pointerId)?.includes(next))
					this.#press(event.pointerId, [next]);
			},
			oncontextmenu: (event) => event.preventDefault()
		};
	}

	/**
	 * An element whose keys a finger picks by where it is over it: `at`
	 * reads them from how far the finger is from its middle, right and
	 * down, in CSS pixels, on an element `size` across, as it lands and
	 * each time it moves.
	 */
	steer(
		at: (x: number, y: number, size: number) => readonly string[]
	): HTMLAttributes<HTMLElement> {
		const under = (event: PointerEvent & { currentTarget: EventTarget & HTMLElement }) => {
			const box = event.currentTarget.getBoundingClientRect();
			return at(
				event.clientX - (box.left + box.width / 2),
				event.clientY - (box.top + box.height / 2),
				box.width
			);
		};
		return {
			onpointerdown: (event) => {
				event.preventDefault();
				event.currentTarget.setPointerCapture(event.pointerId);
				this.#press(event.pointerId, under(event));
			},
			onpointermove: (event) => {
				if (this.#held.has(event.pointerId)) this.#press(event.pointerId, under(event));
			},
			oncontextmenu: (event) => event.preventDefault()
		};
	}

	/** A button that presses `code` once, as a tap. */
	tap(code: string): HTMLButtonAttributes {
		return {
			onpointerdown: (event) => event.preventDefault(),
			onclick: () => {
				this.#onkey(code, true);
				this.#onkey(code, false);
			},
			oncontextmenu: (event) => event.preventDefault()
		};
	}

	/** Whether a finger holds `code`: its button shows it held. */
	holds(code: string): boolean {
		for (const held of this.#held.values()) if (held.includes(code)) return true;
		return false;
	}

	/** Hears fingers lift while the buttons are on the screen, and lets every key go when they leave: an `$effect`'s. */
	listen(): () => void {
		const lift = (event: PointerEvent) => this.#lift(event.pointerId);
		window.addEventListener('pointerup', lift);
		window.addEventListener('pointercancel', lift);
		return () => {
			window.removeEventListener('pointerup', lift);
			window.removeEventListener('pointercancel', lift);
			const codes = new Set([...this.#held.values()].flat());
			this.#held.clear();
			for (const code of codes) this.#onkey(code, false);
		};
	}

	/** The new keys go down before the old ones go up, so a roll never leaves a frame with neither. */
	#press(finger: number, codes: readonly string[]) {
		const was = this.#held.get(finger) ?? [];
		if (was.length === codes.length && was.every((code) => codes.includes(code))) return;
		for (const code of codes) if (!this.holds(code)) this.#onkey(code, true);
		this.#held.set(finger, codes);
		this.#letGo(was);
	}

	#lift(finger: number) {
		const was = this.#held.get(finger);
		if (was === undefined) return;
		this.#held.delete(finger);
		this.#letGo(was);
	}

	#letGo(codes: readonly string[]) {
		for (const code of codes) if (!this.holds(code)) this.#onkey(code, false);
	}
}
