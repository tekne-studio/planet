import type { HTMLButtonAttributes } from 'svelte/elements';
import { SvelteMap } from 'svelte/reactivity';

/**
 * The keys that buttons on a touch screen press, by the name the web gives
 * each key. A finger holds the key of the button it lands on until it
 * lifts or the page takes it; two fingers hold two keys, and a key two
 * fingers hold goes up with the last. A finger that slides onto another
 * button that rolls lets the first go and holds the other, as a thumb
 * rolls across a game's A and B; off every button it keeps what it held.
 * A press keeps the focus where it was, so the canvas holds on to the
 * finger on the stick. A finger lifts wherever it is, off its button or
 * off one gone, and what is held when the buttons leave the screen is let
 * go: no key outlives its finger or its button.
 */
export class Fingers {
	/** The key each finger holds, by the pointer it is; read where a button shows it held. */
	readonly #held = new SvelteMap<number, string>();
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
				this.#press(event.pointerId, code);
			},
			onpointermove: (event) => {
				if (!rolls || !this.#held.has(event.pointerId)) return;
				const under = document.elementFromPoint(event.clientX, event.clientY)?.closest<HTMLElement>('[data-rolls]');
				const next = under?.dataset.key;
				if (next && next !== this.#held.get(event.pointerId)) this.#press(event.pointerId, next);
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
		for (const held of this.#held.values()) if (held === code) return true;
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
			const codes = new Set(this.#held.values());
			this.#held.clear();
			for (const code of codes) this.#onkey(code, false);
		};
	}

	/** The new key goes down before the old one goes up, so a roll never leaves a frame with neither. */
	#press(finger: number, code: string) {
		const was = this.#held.get(finger);
		if (!this.holds(code)) this.#onkey(code, true);
		this.#held.set(finger, code);
		if (was !== undefined && was !== code) this.#letGo(was);
	}

	#lift(finger: number) {
		const was = this.#held.get(finger);
		if (was === undefined) return;
		this.#held.delete(finger);
		this.#letGo(was);
	}

	#letGo(code: string) {
		if (!this.holds(code)) this.#onkey(code, false);
	}
}
