import type { MessageKey } from '$lib/i18n/en';
import type { Held } from './index';
import { classify, weak, type Graphics, type Machine } from './machine';
import type { Effects, StartStep } from './protocol';
import type { Picture } from './settings';
import { steps, type Gpu, type Last, type Step } from './start';

/**
 * Everything the page knows of this machine and its start, for one list
 * shown in two places: the graphics dialog, whose copy is what a person
 * sends when the world does not run for them, and the readout over the
 * world. Nothing that names the person: no account, no address but the
 * world's host.
 */
export interface Facts {
	/** The engine's version, as the world serves it; null on a page with no world to ask. */
	version: string | null;
	host: string;
	/** What the browser said of the graphics; null while it is asked, or where nothing asked it. */
	graphics: Graphics | null;
	/** What the browser said of the rest of the machine; null until a list is shown. */
	machine: Machine | null;
	last: Last;
	/** What this browser keeps: the picture and the GPU, each null when none is kept, and this GPU's key. */
	kept: { picture: Picture | null; effects: Effects | null; gpu: Gpu | null; key: string | null };
	/** The address's flags, as written. */
	said: string[];
	touch: boolean;
	/** The start under way or done. */
	start?: Begun;
	/** What is read while the list is shown. */
	live?: Now;
}

/** What the engine said of its start. */
export interface Begun {
	reached: Partial<Record<Step, number>>;
	adapter?: Extract<StartStep, { step: 'adapter' }>;
	about?: string[];
	canvas?: Extract<StartStep, { step: 'frame' }>;
	failure: string | null;
}

/** What changes as the world runs: the frame, the keys held, who has them. */
export interface Now {
	/** The last frames' interval, its mean and its longest, ms. */
	frame: { mean: number; longest: number } | null;
	held?: Held;
	points: number;
	hidden: boolean;
	focused: boolean;
}

/** A message, said in a language: the page's for the screen, English for the copy. */
export type Say = (key: MessageKey, params?: Record<string, string | number>) => string;

export interface Row {
	label: string;
	value: string;
}

/** The list, in the order it is read: the engine and the browser, the graphics, the screen and the hands, what is kept, the start. */
export function rows(facts: Facts, say: Say): Row[] {
	const { graphics, machine, start, live } = facts;
	const out: Row[] = [];
	const row = (label: MessageKey, value: string) => out.push({ label: say(label), value });
	const unknown = say('engine.machine.unknown');
	const yes = (on: boolean) => say(on ? 'engine.machine.yes' : 'engine.machine.no');

	row('engine.machine.engine', facts.version || unknown);
	row('engine.machine.world', facts.host);
	if (machine) {
		row('engine.machine.browser', machine.browser);
		row('engine.machine.system', machine.system || unknown);
	}
	if (graphics) {
		row('engine.machine.webgpu', webgpu(graphics, say));
		row('engine.machine.webgl', graphics.webgl ?? say('engine.machine.notOffered'));
		const family = classify(graphics);
		const named = say(`engine.machine.family.${family}`);
		row('engine.machine.family', weak(family) ? say('engine.machine.mayBeLight', { family: named }) : named);
	}
	if (start?.adapter) {
		const name = [start.adapter.name, ...(start.about ?? [])].filter(Boolean).join(' · ');
		row('engine.machine.adapter', name || say('engine.machine.hidden'));
		row('engine.machine.road', say(`engine.graphics.road.${start.adapter.backend}`));
	}
	const adapter = typeof graphics?.webgpu === 'object' ? graphics.webgpu : null;
	const limits = start?.adapter
		? { px: start.adapter.texture_px, mib: start.adapter.buffer_mib }
		: adapter && { px: adapter.texturePx, mib: adapter.bufferMib };
	if (limits) row('engine.machine.limits', say('engine.machine.limitsValue', limits));
	if (machine) row('engine.machine.screen', say('engine.machine.size', { ...machine.screen, ratio: ratio(machine.screen.ratio) }));
	if (start?.canvas) row('engine.machine.canvas', say('engine.machine.size', { ...start.canvas, ratio: ratio(start.canvas.ratio) }));
	if (machine) {
		row(
			'engine.machine.processor',
			say('engine.machine.processorValue', { cores: machine.cores ?? unknown, memory: machine.memory ?? unknown })
		);
		row('engine.machine.touch', say('engine.machine.touchValue', { touch: yes(facts.touch), points: machine.touchPoints }));
		row('engine.machine.language', say('engine.machine.languageValue', { language: machine.language, keyboard: machine.keyboard ?? unknown }));
	}
	row('engine.machine.last', lastSaid(facts.last, say));
	row('engine.machine.kept', keptSaid(facts.kept, say));
	row('engine.machine.key', facts.kept.key ?? unknown);
	if (facts.said.length) row('engine.machine.flags', facts.said.join(' '));
	if (start?.failure) row('engine.machine.failure', start.failure);
	if (start) {
		const next = steps.find((step) => start.reached[step] === undefined);
		for (const step of steps) {
			const ms = start.reached[step];
			if (ms === undefined && step !== next) continue;
			row(`engine.machine.step.${step}`, ms === undefined ? '…' : say('engine.machine.seconds', { s: (ms / 1000).toFixed(2) }));
		}
	}
	if (live?.frame) {
		row('engine.machine.frame', say('engine.machine.frameValue', { ms: live.frame.mean.toFixed(1), longest: live.frame.longest.toFixed(0) }));
	}
	if (live?.held) row('engine.machine.held', holding(live.held, say));
	if (live) {
		const page = live.hidden ? 'hidden' : live.focused ? 'focused' : 'unfocused';
		row(
			'engine.machine.away',
			say('engine.machine.awayValue', { engine: live.held ? yes(live.held.away) : '…', page: say(`engine.machine.page.${page}`) })
		);
	}
	return out;
}

/** The list as plain text, a row a line: what the copy button puts on the clipboard. */
export function plain(list: Row[]): string {
	return list.map(({ label, value }) => `${label}: ${value}`).join('\n');
}

function webgpu(graphics: Graphics, say: Say): string {
	const silent = { absent: 'notOffered', none: 'noAdapter', unasked: 'unasked', late: 'late' } as const;
	if (typeof graphics.webgpu === 'string') return say(`engine.machine.${silent[graphics.webgpu]}`);
	const { vendor, architecture, device, description, fallback } = graphics.webgpu;
	const said = [vendor, architecture, device, description].filter(Boolean).join(' · ') || say('engine.machine.hidden');
	return fallback ? say('engine.machine.software', { said }) : said;
}

/** What is kept, each part only where it is: the picture, and a GPU asked off the engine's default. */
function keptSaid(kept: Facts['kept'], say: Say): string {
	const on = (is: boolean) => say(is ? 'engine.machine.on' : 'engine.machine.off');
	const picture =
		kept.picture &&
		kept.effects &&
		say('engine.machine.keptPicture', {
			picture: say(`engine.settings.${kept.picture}`),
			trees: on(kept.effects.trees),
			shrubs: on(kept.effects.shrubs),
			flowers: on(kept.effects.flowers),
			animals: on(kept.effects.animals)
		});
	const gpu =
		kept.gpu &&
		say('engine.machine.keptGpu', {
			power: say(`engine.graphics.power.${kept.gpu.power ?? 'high'}`),
			road: say(`engine.graphics.road.${kept.gpu.backend ?? 'webgpu'}`)
		});
	if (picture && gpu) return say('engine.machine.keptBoth', { picture, gpu });
	return picture || gpu || say('engine.machine.nothing');
}

function lastSaid(last: Last, say: Say): string {
	if (typeof last === 'string') return say(`engine.machine.last.${last}`);
	return last.stopped
		? say('engine.machine.last.stopped', { step: say(`engine.machine.step.${last.stopped}`) })
		: say('engine.machine.last.stoppedEarly');
}

function holding(held: Held, say: Say): string {
	const [right, forward] = held.stick;
	const stick = right || forward ? [say('engine.machine.stick', { right: right.toFixed(2), forward: forward.toFixed(2) })] : [];
	const all = [...held.codes, ...held.keys, ...stick];
	return all.length ? all.join(' ') : say('engine.machine.nothing');
}

/** Screen pixels to a CSS pixel, as a person reads it: 2, 1.25, never 1.2499999. */
function ratio(value: number): string {
	return String(Math.round(value * 100) / 100);
}
