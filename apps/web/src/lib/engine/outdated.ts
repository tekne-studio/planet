/**
 * A page refused for its protocol is older than its world: one reload
 * fetches the world's own build. A mark in the tab's session says it was
 * tried, so a reload that comes back refused, from a cache that kept the old
 * files, stays and asks a hand: never a loop. A link that comes up clears
 * it, so the world's next update reloads again.
 */
const MARK = 'planet.outdated';

type Store = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;

/** Whether to reload now: once, until a link comes up. */
export function reloadOnce(store: () => Store): boolean {
	try {
		const kept = store();
		if (kept.getItem(MARK) !== null) return false;
		kept.setItem(MARK, '1');
		return true;
	} catch {
		// storage blocked: no mark would stop a second reload, so a hand reloads
		return false;
	}
}

/** A link came up: the next refusal reloads again. */
export function linkUp(store: () => Store): void {
	try {
		store().removeItem(MARK);
	} catch {
		// storage blocked: nothing was marked
	}
}
