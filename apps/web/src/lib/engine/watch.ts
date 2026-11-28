import { parsed, steps, type Last, type Step } from './start';

/*
 * A start's watch holds one rule: a start is unsteady only on evidence.
 * Either its page stopped saying it lives while seen (a freeze, a kill),
 * which leaves its pulse behind to go stale, or it lived on, seen, past its
 * patience without reaching steady, and says so itself. The browser keeps
 * two records: the outcome (`start`), written on evidence alone, and the
 * pulse of a start under way (`pulse`), which its page removes as it
 * leaves. A page that leaves says nothing of how its start went. A doubt
 * leaves a healthy machine be: a start wrongly read unsteady makes it draw
 * less, while one missed only asks nothing.
 */

/** An engine frame later than this after the one before, ms, is one the machine did not keep up with. */
export const LATE_MS = 250;
/** The engine's frames on time for this long, ms, once the world is shown: the start is steady. */
export const STEADY_MS = 2000;
/** The engine's frames on time for this long, ms, up to the moment its page is hidden or left: it drew well. */
export const LEFT_DRAWING_MS = 1000;
/**
 * Seen under way this long, ms, a start not steady says it is unsteady:
 * far longer than a machine that holds the world takes. The wait for the
 * world after the first frame drawn, on the network, spends none of it.
 */
export const PATIENCE_MS = 30_000;
/** How often a seen page's pulse says it lives, ms. */
export const PULSE_MS = 500;
/** The pulse of the page this one reloads, unchanged this long, ms, is a page that stopped saying it lives: six of its beats. */
export const FRESH_MS = 3000;
/** The most one frame adds to the time seen, ms: a machine asleep spends none of the patience. */
const SEEN_FRAME_MS = 1000;

/** A start under way, as its pulse says: whose page, the last step reached, and when it last said it lives, seen (`at`, the wall clock), or that it is away. */
export interface Pulse {
	id: string;
	stopped: Step | null;
	at?: number;
	away?: true;
}

/** How a start ended, as the outcome keeps it. */
export function outcome(last: Exclude<Last, 'none'>): string {
	return typeof last === 'string' ? last : JSON.stringify({ stopped: last.stopped });
}

/** How the last start ended, as its outcome says: anything else is none. */
export function readOutcome(text: string | null): Last {
	if (text === 'steady' || text === 'failed') return text;
	const kept = parsed(text);
	if (!kept || typeof kept !== 'object') return 'none';
	const { stopped } = kept as Record<string, unknown>;
	return { stopped: steps.find((step) => step === stopped) ?? null };
}

export function readPulse(text: string | null): Pulse | null {
	const kept = parsed(text);
	if (!kept || typeof kept !== 'object') return null;
	const { id, stopped, at, away } = kept as Record<string, unknown>;
	if (typeof id !== 'string') return null;
	const step = steps.find((known) => known === stopped) ?? null;
	if (away === true) return { id, stopped: step, away };
	return typeof at === 'number' ? { id, stopped: step, at } : null;
}

/**
 * What the two records say of the last start at `now`, the wall clock, read
 * by a page that reloads the one whose start was `own`, else null. A seen
 * pulse gone stale is a page that stopped while seen: unsteady at the step
 * it `stopped`, for the reader to keep as the outcome. A fresh one is a page
 * that may live: read again once it is rewritten, or unchanged after
 * `wait`, ms, on the reader's own timer. One away, or none, leaves the
 * outcome as it is, the `last` start.
 */
export type Reading = { last: Last } | { stopped: Step | null } | { wait: number };

export function reading(outcomeText: string | null, pulseText: string | null, now: number, own: string | null): Reading {
	const pulse = readPulse(pulseText);
	if (pulse?.at === undefined) return { last: readOutcome(outcomeText) };
	const fresh = freshFor(pulse, own);
	// A pulse from later than now was written under a clock since set back: its age is unknown, and it is stale.
	const age = pulse.at > now ? fresh : now - pulse.at;
	return age < fresh ? { wait: fresh - age } : { stopped: pulse.stopped };
}

/**
 * How long a seen pulse goes unchanged before it is a page that stopped. The
 * page this one reloads is gone, and waits only for its goodbye. Another
 * page may live, held by one long task of its start, as WebGL2 compiling its
 * pipelines: a page held past a start's patience is unsteady all the same.
 */
function freshFor(pulse: Pulse, own: string | null): number {
	return pulse.id === own ? FRESH_MS : PATIENCE_MS;
}

/** One start watched, from its module loaded until it ends or its page lets it go. */
export interface Watch {
	/** Under way and seen; under way and hidden, or in the back-forward cache; or over: ended, or let go. */
	phase: 'seen' | 'away' | 'over';
	id: string;
	step: Step | null;
	/** When the first frame was drawn, and the world shown; the page's clock, null before. */
	drawn: number | null;
	world: number | null;
	/** The engine's last frame seen, and since when its frames have come on time; the page's clock. */
	frame: number;
	onTimeSince: number;
	/** The time seen under way, ms, and the page's clock at its last frame. */
	seenMs: number;
	frameAt: number;
	/** The page's clock when the pulse last said the page lives. */
	pulsedAt: number;
	/** Whether it said it is unsteady, its patience spent. */
	told: boolean;
}

/**
 * What a watch hears, `now` on the page's clock and `wall` on the wall
 * clock: a step reached; a frame of the page, with the engine's last frame
 * (`presented`); the page hidden or put in the back-forward cache, and seen
 * again; the page left, or the start let go; the engine's refusal.
 */
export type Event =
	| { kind: 'step'; step: Step; now: number; wall: number }
	| { kind: 'frame'; now: number; presented: number; wall: number }
	| { kind: 'hidden'; now: number; presented: number }
	| { kind: 'shown'; now: number; wall: number }
	| { kind: 'left'; now: number; presented: number }
	| { kind: 'failed' };

/** A record written, or the pulse removed where it is still the page's own. */
export type Write = { record: 'start' | 'pulse'; value: string } | { forget: string };

/** A watch begun as the module loaded, `now`, its page seen or hidden. */
export function begun(id: string, now: number, wall: number, hidden: boolean): [Watch, Write[]] {
	const watch: Watch = {
		phase: hidden ? 'away' : 'seen',
		id,
		step: 'module',
		drawn: null,
		world: null,
		frame: 0,
		onTimeSince: 0,
		seenMs: 0,
		frameAt: now,
		pulsedAt: now,
		told: false
	};
	return [watch, [beat(watch, wall)]];
}

/** The watch after `event`, and what it writes. */
export function next(watch: Watch, event: Event): [Watch, Write[]] {
	if (watch.phase === 'over') return [watch, []];
	switch (event.kind) {
		case 'step': {
			const reached = {
				...watch,
				step: event.step,
				drawn: event.step === 'drawn' ? event.now : watch.drawn,
				world: event.step === 'world' ? event.now : watch.world
			};
			return watch.phase === 'seen' ? pulsed(reached, event.now, event.wall) : [reached, []];
		}
		case 'frame': {
			if (watch.phase !== 'seen') return [watch, []];
			const awaiting = watch.drawn !== null && watch.world === null;
			const spent = awaiting ? 0 : Math.min(Math.max(event.now - watch.frameAt, 0), SEEN_FRAME_MS);
			let seen = {
				...see(watch, event.presented),
				seenMs: watch.seenMs + spent,
				frameAt: event.now
			};
			if (onTimeFor(seen, STEADY_MS)) return ended(seen, 'steady');
			const writes: Write[] = [];
			if (!seen.told && seen.seenMs >= PATIENCE_MS) {
				seen = { ...seen, told: true };
				writes.push({ record: 'start', value: outcome({ stopped: seen.step }) });
			}
			if (event.now - seen.pulsedAt < PULSE_MS) return [seen, writes];
			const [beaten, beats] = pulsed(seen, event.now, event.wall);
			return [beaten, [...writes, ...beats]];
		}
		case 'hidden': {
			if (watch.phase !== 'seen') return [watch, []];
			const seen = see(watch, event.presented);
			if (drewWell(seen, event.now)) return ended(seen, 'steady');
			const away = { ...seen, phase: 'away' as const };
			return [away, [beat(away, 0)]];
		}
		case 'shown':
			if (watch.phase !== 'away') return [watch, []];
			return pulsed({ ...watch, phase: 'seen', frameAt: event.now }, event.now, event.wall);
		case 'left': {
			const seen = watch.phase === 'seen' ? see(watch, event.presented) : watch;
			if (watch.phase === 'seen' && drewWell(seen, event.now)) return ended(seen, 'steady');
			return [{ ...seen, phase: 'over' }, [{ forget: watch.id }]];
		}
		case 'failed':
			return ended(watch, 'failed');
	}
}

/**
 * A read of when the engine last handed a picture to the screen that keeps
 * its last answer once the engine has trapped, since a trapped engine
 * throws: it draws no more, and its start is watched on as one that hangs.
 */
export function lastFrame(read: () => number): () => number {
	let last = 0;
	return () => {
		try {
			last = read();
		} catch {
			// trapped: no picture comes after the last one
		}
		return last;
	};
}

/** The engine's newest frame, and whether it came late. */
function see(watch: Watch, presented: number): Watch {
	if (presented <= watch.frame) return watch;
	const late = presented - watch.frame > LATE_MS;
	return { ...watch, frame: presented, onTimeSince: late ? presented : watch.onTimeSince };
}

/**
 * Whether the engine's frames have come on time for `ms` since the world was
 * shown, up to its last: the frames before draw the engine's own world, a
 * lighter one than the page awaits.
 */
function onTimeFor(watch: Watch, ms: number): boolean {
	return watch.world !== null && watch.frame - Math.max(watch.onTimeSince, watch.world) >= ms;
}

/** Whether the engine drew well up to `now`: its frames on time for the last second, the last one recent. */
function drewWell(watch: Watch, now: number): boolean {
	return now - watch.frame <= LATE_MS && onTimeFor(watch, LEFT_DRAWING_MS);
}

function ended(watch: Watch, how: 'steady' | 'failed'): [Watch, Write[]] {
	return [{ ...watch, phase: 'over' }, [{ record: 'start', value: how }, { forget: watch.id }]];
}

function pulsed(watch: Watch, now: number, wall: number): [Watch, Write[]] {
	const beaten = { ...watch, pulsedAt: now };
	return [beaten, [beat(beaten, wall)]];
}

/** The pulse of a watch: seen, with the wall clock's time; else away. */
function beat(watch: Watch, wall: number): Write {
	const { id, step: stopped } = watch;
	const pulse: Pulse = watch.phase === 'seen' ? { id, stopped, at: wall } : { id, stopped, away: true };
	return { record: 'pulse', value: JSON.stringify(pulse) };
}

/** The records a start keeps, and how a page hears another page write one. */
export interface Shelf {
	get(name: string): string | null;
	set(name: string, value: string): void;
	remove(name: string): void;
	/** Calls `heard` each time another page writes `name`, until the returned stop. */
	heard(name: string, heard: () => void): () => void;
}

/** The wall clock, and a call after a wait on the page's own timer. */
export interface Clock {
	wall(): number;
	after(ms: number, then: () => void): () => void;
}

/** Writes what a watch wrote; the pulse is forgotten only where it is still that page's. */
export function write(shelf: Shelf, writes: Write[]) {
	for (const one of writes) {
		if (!('forget' in one)) shelf.set(one.record, one.value);
		else if (readPulse(shelf.get('pulse'))?.id === one.forget) shelf.remove('pulse');
	}
}

/**
 * How the last start ended, as `shelf` keeps it, read by a page that reloads
 * the one whose start was `own`, else null. A fresh pulse is a page that
 * lives, or one that froze a moment ago: the read waits for it to be
 * written again, which a page that lives does within its next beats, or to
 * stay unchanged, the evidence of a page that stopped while seen, kept as
 * the outcome.
 */
export function lastOn(shelf: Shelf, clock: Clock, own: string | null): Promise<Last> {
	return new Promise((done) => {
		const pulse = shelf.get('pulse');
		const said = reading(shelf.get('start'), pulse, clock.wall(), own);
		if ('last' in said) return done(said.last);
		if ('stopped' in said) return done(stopped(shelf, said.stopped));
		const lives = () => {
			stops.forEach((stop) => stop());
			done(readOutcome(shelf.get('start')));
		};
		const stops = [
			shelf.heard('pulse', lives),
			shelf.heard('start', lives),
			clock.after(said.wait, () => {
				if (shelf.get('pulse') !== pulse) return lives();
				stops.forEach((stop) => stop());
				done(stopped(shelf, readPulse(pulse)?.stopped ?? null));
			})
		];
	});
}

/** Keeps a page that stopped while seen, at `step`, as the outcome, its pulse gone. */
function stopped(shelf: Shelf, step: Step | null): Last {
	const last = { stopped: step };
	shelf.set('start', outcome(last));
	shelf.remove('pulse');
	return last;
}
