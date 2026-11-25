import { keep, kept } from '$lib/kept';

/**
 * Whether the chat is out on a touch screen, where it waits behind its
 * button, and whether a line came while it was not on the screen: what the
 * button and the bar share, one chat a page. The browser keeps whether it
 * is out, as it keeps the page's other choices; a screen with keys always
 * has the bar and never reads this.
 */
class Out {
	#shown = $state(kept('chat') === 'out');
	/** Someone else said a line while the bar was off the screen. */
	unread = $state(false);

	get shown(): boolean {
		return this.#shown;
	}

	set shown(next: boolean) {
		this.#shown = next;
		keep('chat', next ? 'out' : 'away');
	}
}

export const out = new Out();
