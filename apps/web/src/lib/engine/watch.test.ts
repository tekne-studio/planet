import { describe, expect, test } from 'bun:test';
import type { Last, Step } from './start';
import { begun, FRESH_MS, lastFrame, lastOn, next, PATIENCE_MS, readPulse, write, type Clock, type Event, type Shelf, type Watch } from './watch';

/** The wall clock's time when a browser's clocks read 0. */
const EPOCH = 1_800_000_000_000;
const HOUR = 3_600_000;

/** One browser: its records, shared by its pages and heard by the others, and a clock that moves only when told. */
class Browser implements Shelf, Clock {
	t = 0;
	/** How far the wall clock was set off the page's, ms. */
	setBack = 0;
	/** The start of the page the next visit reloads; null for a page that reloads none. */
	reloads: string | null = null;
	/** The longest wait asked, ms. */
	longest = 0;
	#records = new Map<string, string>();
	#heard = new Map<string, Set<() => void>>();
	#timers: { at: number; then: () => void }[] = [];

	get(name: string) {
		return this.#records.get(name) ?? null;
	}
	set(name: string, value: string) {
		this.#records.set(name, value);
		this.#tell(name);
	}
	remove(name: string) {
		this.#records.delete(name);
		this.#tell(name);
	}
	heard(name: string, heard: () => void) {
		const all = this.#heard.get(name) ?? new Set();
		this.#heard.set(name, all.add(heard));
		return () => void all.delete(heard);
	}
	wall() {
		return EPOCH + this.t - this.setBack;
	}
	after(ms: number, then: () => void) {
		this.longest = Math.max(this.longest, ms);
		const timer = { at: this.t + ms, then };
		this.#timers.push(timer);
		return () => void (this.#timers = this.#timers.filter((one) => one !== timer));
	}
	/** Time goes on `ms`, each wait due called as it falls due. */
	wait(ms: number) {
		const end = this.t + ms;
		for (let due = this.#due(end); due; due = this.#due(end)) {
			this.#timers = this.#timers.filter((one) => one !== due);
			this.t = Math.max(this.t, due.at);
			due.then();
		}
		this.t = end;
	}
	/** The next visit's read of the last start, its waits run out. */
	async visit(): Promise<Last> {
		const read = this.read();
		this.wait(PATIENCE_MS);
		return read;
	}
	/** The next visit's read, under way: what it read once it has, else undefined. */
	read() {
		const read = lastOn(this, this, this.reloads);
		void read.then((last) => (this.last = last));
		return read;
	}
	last: Last | undefined;
	#due(end: number) {
		return this.#timers.filter((one) => one.at <= end).sort((a, b) => a.at - b.at)[0];
	}
	#tell(name: string) {
		for (const heard of [...(this.#heard.get(name) ?? [])]) heard();
	}
}

/** A page of the world starting, as its watch hears it. */
class Tab {
	watch: Watch;
	/** When the engine last handed a picture to the screen; 0 before the first. */
	presented = 0;

	constructor(
		readonly browser: Browser,
		readonly id: string,
		hidden = false
	) {
		const [watch, writes] = begun(id, browser.t, browser.wall(), hidden);
		this.watch = watch;
		write(browser, writes);
	}

	hear(event: Event) {
		const [watch, writes] = next(this.watch, event);
		this.watch = watch;
		write(this.browser, writes);
	}
	step(step: Step) {
		this.hear({ kind: 'step', step, now: this.browser.t, wall: this.browser.wall() });
	}
	/** The engine's steps up to its first frame drawn, the world the page awaits on its way. */
	drawn() {
		for (const step of ['adapter', 'device', 'pipelines', 'frame', 'drawn'] as const) this.step(step);
	}
	/** The steps up to the world shown. */
	entered() {
		this.drawn();
		this.step('world');
	}
	/** The page's frames for `ms`, one each `every` ms, the engine drawing each one. */
	run(ms: number, every = 16) {
		for (const end = this.browser.t + ms; this.browser.t < end;) {
			this.browser.wait(every);
			this.presented = this.browser.t;
			this.hear({
				kind: 'frame',
				now: this.browser.t,
				presented: this.presented,
				wall: this.browser.wall()
			});
		}
	}
	/** The page's frames for `ms`, the engine drawing none. */
	hang(ms: number) {
		for (const end = this.browser.t + ms; this.browser.t < end;) {
			this.browser.wait(16);
			this.hear({
				kind: 'frame',
				now: this.browser.t,
				presented: this.presented,
				wall: this.browser.wall()
			});
		}
	}
	hide() {
		this.hear({ kind: 'hidden', now: this.browser.t, presented: this.presented });
	}
	show() {
		this.hear({ kind: 'shown', now: this.browser.t, wall: this.browser.wall() });
	}
	leave() {
		this.hear({ kind: 'left', now: this.browser.t, presented: this.presented });
	}
	/** Reloaded with no goodbye, frozen: the next visit is its reload. */
	reload() {
		this.browser.reloads = this.id;
	}
}

/** A browser whose last start was steady. */
function steadyBefore(): Browser {
	const browser = new Browser();
	browser.set('start', 'steady');
	browser.wait(HOUR);
	return browser;
}

const unsteady = (stopped: Step | null): Last => ({ stopped });

/**
 * Each row a story of a start, from a browser whose last start was steady:
 * what it does, then the outcome the next visit reads. A start is unsteady
 * only on evidence.
 */
const rows: [string, (browser: Browser) => void, Last][] = [
	[
		'steady: frames on time for two seconds once the world is shown',
		(b) => {
			const tab = new Tab(b, 'a');
			tab.drawn();
			tab.run(5000);
			expect(tab.watch.phase).toBe('seen');
			tab.step('world');
			tab.run(2100);
			expect(tab.watch.phase).toBe('over');
		},
		'steady'
	],
	['refused by the engine', (b) => new Tab(b, 'a').hear({ kind: 'failed' }), 'failed'],
	// What a page left before steady says: nothing.
	[
		'reloaded, closed or a link followed before steady',
		(b) => {
			const tab = new Tab(b, 'a');
			tab.step('adapter');
			tab.step('pipelines');
			tab.run(800);
			tab.leave();
		},
		'steady'
	],
	[
		'remounted by a hot reload, the new start steady',
		(b) => {
			const first = new Tab(b, 'a');
			first.drawn();
			first.run(300);
			first.leave();
			const again = new Tab(b, 'b');
			again.entered();
			again.run(2100);
		},
		'steady'
	],
	[
		'switched away from, then closed',
		(b) => {
			const tab = new Tab(b, 'a');
			tab.drawn();
			tab.run(500);
			tab.hide();
			b.wait(60_000);
			tab.leave();
		},
		'steady'
	],
	[
		'opened in the background, never seen, closed',
		(b) => {
			const tab = new Tab(b, 'a', true);
			tab.step('adapter');
			b.wait(HOUR);
			tab.leave();
		},
		'steady'
	],
	[
		'kept in the back-forward cache, then restored and steady',
		(b) => {
			const tab = new Tab(b, 'a');
			tab.entered();
			tab.run(500);
			tab.hide();
			b.wait(HOUR);
			tab.show();
			tab.run(2100);
			expect(tab.watch.phase).toBe('over');
		},
		'steady'
	],
	[
		'hidden while the engine draws well',
		(b) => {
			const tab = new Tab(b, 'a');
			tab.entered();
			tab.run(1100);
			tab.hide();
			expect(tab.watch.phase).toBe('over');
		},
		'steady'
	],
	[
		'the machine asleep mid-start and woken: the same page goes on to steady',
		(b) => {
			const tab = new Tab(b, 'a');
			tab.entered();
			tab.run(500);
			b.wait(8 * HOUR);
			tab.run(2100);
			expect(tab.watch.phase).toBe('over');
			expect(tab.watch.seenMs).toBeLessThan(5000);
		},
		'steady'
	],
	[
		'the machine asleep, the page hidden for it',
		(b) => {
			const tab = new Tab(b, 'a');
			tab.entered();
			tab.run(500);
			tab.hide();
			b.wait(8 * HOUR);
			tab.show();
			tab.run(2100);
		},
		'steady'
	],
	[
		'the browser quit with several tabs under way, each saying goodbye',
		(b) => {
			const tabs = ['a', 'b', 'c'].map((id) => new Tab(b, id));
			tabs[0].drawn();
			tabs[1].step('adapter');
			tabs[2].hide();
			for (const tab of tabs) tab.run(400);
			for (const tab of [tabs[1], tabs[2], tabs[0]]) tab.leave();
			expect(b.get('pulse')).toBeNull();
		},
		'steady'
	],
	[
		'two pages at once, one left after the other was steady',
		(b) => {
			const one = new Tab(b, 'a');
			const two = new Tab(b, 'b');
			one.entered();
			two.step('adapter');
			one.run(2100);
			two.run(200);
			expect(readPulse(b.get('pulse'))?.id).toBe('b');
			two.leave();
		},
		'steady'
	],
	[
		'the browser killed with a tab hidden: no evidence',
		(b) => {
			const tab = new Tab(b, 'a');
			tab.step('adapter');
			tab.hide();
			b.wait(HOUR);
		},
		'steady'
	],
	// Evidence: a page that stopped while seen, or one seen past its patience.
	[
		'frozen while seen, its tab killed',
		(b) => {
			const tab = new Tab(b, 'a');
			tab.step('adapter');
			tab.step('pipelines');
			b.wait(60_000);
		},
		unsteady('pipelines')
	],
	[
		'frozen, then reloaded at once',
		(b) => {
			const tab = new Tab(b, 'a');
			tab.drawn();
			b.wait(400);
			tab.reload();
		},
		unsteady('drawn')
	],
	// A build the world refuses: the engine draws, the world is never shown,
	// and the page reloads once for the world's own build, or stays and asks a
	// hand to. Neither is a start that failed or stalled.
	[
		'refused by the world for its build, reloaded at once with its goodbye',
		(b) => {
			const tab = new Tab(b, 'a');
			tab.drawn();
			tab.run(400);
			tab.leave();
			tab.reload();
		},
		'steady'
	],
	[
		'refused by the world for its build again, held on the veil past the patience',
		(b) => {
			const tab = new Tab(b, 'a');
			tab.drawn();
			tab.run(2 * PATIENCE_MS, 100);
			expect(tab.watch.phase).toBe('seen');
			expect(b.get('start')).toBe('steady');
			tab.leave();
		},
		'steady'
	],
	[
		'the browser killed with no goodbye, a tab seen under way: a freeze, as far as anyone can tell',
		(b) => {
			const tab = new Tab(b, 'a');
			tab.step('device');
			tab.run(300);
		},
		unsteady('device')
	],
	[
		'seen past its patience, the engine never drawing',
		(b) => {
			const tab = new Tab(b, 'a');
			tab.step('adapter');
			tab.hang(PATIENCE_MS + 100);
			expect(b.get('start')).toBe('{"stopped":"adapter"}');
			tab.leave();
		},
		unsteady('adapter')
	],
	[
		'seen past its patience, its frames late',
		(b) => {
			const tab = new Tab(b, 'a');
			tab.entered();
			tab.run(PATIENCE_MS + 100, 300);
			tab.leave();
		},
		unsteady('world')
	],
	[
		'seen past its patience, then steady',
		(b) => {
			const tab = new Tab(b, 'a');
			tab.entered();
			tab.run(PATIENCE_MS + 100, 300);
			tab.run(2100);
		},
		'steady'
	],
	[
		'awaiting the world past the patience, its frames late: the network waits, and spends none of it',
		(b) => {
			const tab = new Tab(b, 'a');
			tab.drawn();
			tab.run(2 * PATIENCE_MS, 300);
			expect(tab.watch.seenMs).toBeLessThan(1000);
			tab.leave();
		},
		'steady'
	],
	[
		'seen past its patience once the world is shown, after a long wait for it',
		(b) => {
			const tab = new Tab(b, 'a');
			tab.drawn();
			tab.run(2 * PATIENCE_MS);
			tab.step('world');
			tab.run(PATIENCE_MS + 100, 300);
			tab.leave();
		},
		unsteady('world')
	],
	[
		'the engine trapped while seen: its frames stop and the page lives on past its patience',
		(b) => {
			const tab = new Tab(b, 'a');
			tab.entered();
			tab.run(500);
			let trapped = false;
			const presented = lastFrame(() => {
				if (trapped) throw new Error('unreachable');
				return b.t;
			});
			const last = presented();
			trapped = true;
			expect(presented()).toBe(last);
			for (const end = b.t + PATIENCE_MS + 100; b.t < end; ) {
				b.wait(16);
				tab.hear({ kind: 'frame', now: b.t, presented: presented(), wall: b.wall() });
			}
			tab.hear({ kind: 'left', now: b.t, presented: presented() });
		},
		unsteady('world')
	]
];

describe("a start's watch", () => {
	test.each(rows)('%s', async (_, story, read) => {
		const browser = steadyBefore();
		story(browser);
		expect(await browser.visit()).toEqual(read);
	});

	test('evidence read is kept as the outcome, and the pulse goes with it', async () => {
		const browser = steadyBefore();
		new Tab(browser, 'a').step('device');
		browser.wait(HOUR);
		expect(await browser.visit()).toEqual(unsteady('device'));
		expect(browser.get('start')).toBe('{"stopped":"device"}');
		expect(browser.get('pulse')).toBeNull();
		expect(await browser.visit()).toEqual(unsteady('device'));
	});

	test('a page held by a long task, read from a second tab, lives on: the read waits for its beat', async () => {
		const browser = steadyBefore();
		const tab = new Tab(browser, 'a');
		tab.step('pipelines');
		tab.run(400);
		// WebGL2 compiles the pipelines: no beat for three seconds and more.
		browser.wait(FRESH_MS + 500);
		const read = browser.read();
		browser.wait(1000);
		await Promise.resolve();
		expect(browser.last).toBeUndefined();
		tab.drawn();
		await read;
		expect(browser.last).toBe('steady');
		expect(browser.get('start')).toBe('steady');
		expect(tab.watch.phase).toBe('seen');
	});

	test("another tab's page frozen while seen is unsteady once unheard for a start's patience", async () => {
		const browser = steadyBefore();
		new Tab(browser, 'a').step('device');
		browser.wait(FRESH_MS);
		const read = browser.read();
		browser.wait(PATIENCE_MS - FRESH_MS - 100);
		await Promise.resolve();
		expect(browser.last).toBeUndefined();
		browser.wait(100);
		expect(await read).toEqual(unsteady('device'));
	});

	test('the page a reload replaces, its pulse stale, is unsteady at once', async () => {
		const browser = steadyBefore();
		const tab = new Tab(browser, 'a');
		tab.step('pipelines');
		browser.wait(FRESH_MS);
		tab.reload();
		void browser.read();
		await Promise.resolve();
		expect(browser.last).toEqual(unsteady('pipelines'));
		expect(browser.longest).toBe(0);
	});

	test('a clock set back after a seen start was killed waits on no time to come: the pulse is stale', async () => {
		const browser = steadyBefore();
		new Tab(browser, 'a').step('pipelines');
		browser.wait(60_000);
		browser.setBack = 2 * HOUR;
		void browser.read();
		await Promise.resolve();
		expect(browser.last).toEqual(unsteady('pipelines'));
		expect(browser.longest).toBe(0);
	});

	test("a clock set back during the wait: the reader's own timer decides, once", async () => {
		const browser = steadyBefore();
		const tab = new Tab(browser, 'a');
		tab.step('device');
		browser.wait(1000);
		tab.reload();
		const read = browser.read();
		browser.setBack = 2 * HOUR;
		browser.wait(FRESH_MS);
		expect(await read).toEqual(unsteady('device'));
		expect(browser.longest).toBeLessThanOrEqual(FRESH_MS);
	});

	test('a page that lives says so within the wait, and its start is no evidence', async () => {
		const browser = steadyBefore();
		const tab = new Tab(browser, 'a');
		tab.drawn();
		let read: Last | undefined;
		void lastOn(browser, browser, null).then((last) => (read = last));
		tab.run(600);
		await Promise.resolve();
		expect(read).toBe('steady');
		expect(tab.watch.phase).toBe('seen');
	});

	test("a page leaving forgets its own pulse, never another's", () => {
		const browser = steadyBefore();
		const one = new Tab(browser, 'a');
		const two = new Tab(browser, 'b');
		one.leave();
		expect(readPulse(browser.get('pulse'))?.id).toBe('b');
		two.leave();
		expect(browser.get('pulse')).toBeNull();
	});

	test('a watch over hears nothing more', () => {
		const browser = steadyBefore();
		const tab = new Tab(browser, 'a');
		tab.hear({ kind: 'failed' });
		tab.run(PATIENCE_MS + 100);
		tab.show();
		tab.leave();
		expect(browser.get('start')).toBe('failed');
	});
});
