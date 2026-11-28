import { forget, keep, kept, onKept } from '$lib/kept';
import type { Begun } from './facts';
import type { StartStep } from './protocol';
import type { Last, Step } from './start';
import { begun, lastFrame, lastOn, next, write, type Clock, type Event, type Shelf, type Watch, type Write } from './watch';

const browserShelf: Shelf = {
	get: kept,
	set: keep,
	remove: forget,
	heard: (name, heard) => onKept(name, heard)
};
const browserClock: Clock = {
	wall: () => Date.now(),
	after: (ms, then) => {
		const timer = setTimeout(then, ms);
		return () => clearTimeout(timer);
	}
};

/** Where this tab keeps the start its page began, for the page that reloads it. */
const OWN = 'planet.start';

/** How the last start in this browser ended (`lastOn`), read by this page. */
export function lastStart(): Promise<Last> {
	return lastOn(browserShelf, browserClock, reloaded());
}

/**
 * The start of the page this one reloads, as this tab kept it; null where
 * this page is no reload. A duplicated tab keeps the same, and is no reload.
 */
function reloaded(): string | null {
	try {
		const [navigation] = performance.getEntriesByType('navigation') as PerformanceNavigationTiming[];
		return navigation?.type === 'reload' ? sessionStorage.getItem(OWN) : null;
	} catch {
		// storage blocked: no page is known as this tab's own
		return null;
	}
}

/** The page's own clocks and events, as a watch hears them. */
export interface Page {
	now(): number;
	wall(): number;
	hidden(): boolean;
	/** Calls `frame` at the page's next frame, until the returned stop. */
	frame(then: () => void): () => void;
	/** Calls `heard` with each event of the page that bears on a start, until the returned stop. */
	listen(heard: (said: 'hidden' | 'shown' | 'left') => void): () => void;
	/** Keeps `id` as the start this tab's page began. */
	began(id: string): void;
}

const browserPage: Page = {
	now: () => performance.now(),
	wall: () => Date.now(),
	hidden: () => document.hidden,
	frame: (then) => {
		const request = requestAnimationFrame(then);
		return () => cancelAnimationFrame(request);
	},
	listen: (heard) => {
		// Hidden, or put in the back-forward cache, a page lives with no frames;
		// left for good, it says goodbye.
		const seen = () => heard(document.hidden ? 'hidden' : 'shown');
		const hide = (event: PageTransitionEvent) => heard(event.persisted ? 'hidden' : 'left');
		const show = (event: PageTransitionEvent) => event.persisted && !document.hidden && heard('shown');
		document.addEventListener('visibilitychange', seen);
		addEventListener('pagehide', hide);
		addEventListener('pageshow', show);
		return () => {
			document.removeEventListener('visibilitychange', seen);
			removeEventListener('pagehide', hide);
			removeEventListener('pageshow', show);
		};
	},
	began: (id) => {
		try {
			sessionStorage.setItem(OWN, id);
		} catch {
			// storage blocked: a reload reads this start as another tab's
		}
	}
};

/**
 * One start of the engine, from its module loaded until it is steady: the
 * steps reached and what the engine said on the way, for the list of
 * facts, and its watch (`watch.ts`), kept in this browser. Steady is
 * measured on the engine's own frames, the pictures it hands to the
 * screen, never the page's.
 */
export class Started implements Begun {
	reached = $state<Partial<Record<Step, number>>>({});
	adapter = $state<Begun['adapter']>();
	about = $state<string[]>();
	canvas = $state<Begun['canvas']>();
	failure = $state<string | null>(null);

	#shelf: Shelf;
	#page: Page;
	#watch: Watch | null = null;
	#presented: () => number = () => 0;
	#unframe = () => {};
	#unlisten = () => {};

	constructor(shelf: Shelf = browserShelf, page: Page = browserPage) {
		this.#shelf = shelf;
		this.#page = page;
	}

	/**
	 * The engine's start about to be asked, its module loaded at `loaded`:
	 * watched from here until it ends. `presented` reads when the engine
	 * last handed a picture to the screen, 0 before the first.
	 */
	begin(loaded: number, presented: () => number) {
		this.#presented = lastFrame(presented);
		this.reached.module = loaded;
		// Unique enough to tell this page's pulse from another's.
		const id = `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
		this.#page.began(id);
		this.#apply(...begun(id, loaded, this.#page.wall(), this.#page.hidden()));
		this.#unlisten = this.#page.listen((said) => {
			const { now, wall } = { now: this.#page.now(), wall: this.#page.wall() };
			if (said === 'shown') this.#hear({ kind: 'shown', now, wall });
			else this.#hear({ kind: said, now, presented: this.#presented() });
			if (said === 'shown') this.#frames();
		});
		this.#frames();
	}

	/** A step the engine said. */
	hear(event: StartStep & { ms: number }) {
		if (event.step === 'adapter') this.adapter = event;
		if (event.step === 'device') this.about = event.about;
		if (event.step === 'frame') this.canvas = event;
		this.#reach(event.step, event.ms);
	}

	/** The world shown: the veil lifted. */
	shown() {
		if (this.reached.world === undefined) this.#reach('world', this.#page.now());
	}

	/** The engine refused to start: said at once. */
	fail(error: unknown) {
		this.failure = String(error);
		this.#hear({ kind: 'failed' });
	}

	/** The page lets this start go: a new one in its place, or the world left. */
	stop() {
		this.#hear({ kind: 'left', now: this.#page.now(), presented: this.#presented() });
	}

	#reach(step: Step, ms: number) {
		this.reached[step] = ms;
		this.#hear({ kind: 'step', step, now: ms, wall: this.#page.wall() });
	}

	/** Each frame of the page heard while the watch is seen; a page hidden has none, and is heard again once seen. */
	#frames() {
		this.#unframe();
		const frame = () => {
			const watch = this.#hear({
				kind: 'frame',
				now: this.#page.now(),
				presented: this.#presented(),
				wall: this.#page.wall()
			});
			this.#unframe = watch?.phase === 'seen' ? this.#page.frame(frame) : () => {};
		};
		this.#unframe = this.#page.frame(frame);
	}

	#hear(event: Event): Watch | null {
		if (this.#watch) this.#apply(...next(this.#watch, event));
		return this.#watch;
	}

	#apply(watch: Watch, writes: Write[]) {
		this.#watch = watch;
		write(this.#shelf, writes);
		if (watch.phase === 'over') {
			this.#unframe();
			this.#unlisten();
		}
	}
}
