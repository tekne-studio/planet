/**
 * The command/event seam of the engine, as JSON tagged by `type` (snake_case).
 * Pure: no WASM, no DOM.
 */
import type { Recipe } from '$lib/world';

/** On foot, in flight, or in a way of moving a plugin gives, by its full name: `ride.board`. */
export type Mode = 'walk' | 'fly' | `${string}.${string}`;

/** The most a name carries. The server cuts longer ones. */
export const NAME_CHARS = 24;

/** What a session may do, as mundos says it and the world repeats it. A builder and an admin build. */
export type Level = 'anonymous' | 'signed_in' | 'builder' | 'admin';

export type ToneMap = 'aces' | 'agx' | 'neutral' | 'reinhard' | 'linear';
export const toneMaps = ['aces', 'agx', 'neutral', 'reinhard', 'linear'] as const satisfies readonly ToneMap[];

/**
 * How loud each group of what is heard is, 0 to 1, under one level for
 * everything and a mute: the levels of the shell's mix, each group a bus
 * under one (`Engine.set_loudness`). The page keeps them, as the picture.
 */
export interface Loudness {
	/** Everything heard. */
	master: number;
	muted: boolean;
	/** What is played in the world: videos and sound parts. */
	world: number;
	/** What a body's motion makes: hooves, wings. */
	steps: number;
	/** What a body calls out, its own or a peer's: the whistle. */
	calls: number;
	/** The arcade's cabinets. */
	games: number;
}

/** How the picture is made. Mirrors `scene::Effects`; the engine clamps it. */
export interface Effects {
	shadows: boolean;
	/** Whether the shadows fade from one step of the sun to the next; off, they move a step at a time, for fewer passes. */
	shadows_fade: boolean;
	grass: boolean;
	clouds: boolean;
	/** Whether the trees are drawn, the plants that stand on a trunk; off on this machine, its own body walks through them. */
	trees: boolean;
	/** Whether the shrubs are drawn, the plants whose wood hides in their leaves, which stop no body. */
	shrubs: boolean;
	/** Whether flowers bloom in the grass, where it is drawn. */
	flowers: boolean;
	/** Whether the plants drawn cast into the sun's shadows, while it and the trees are on. */
	foliage_shadows: boolean;
	/** Whether the herds nobody rides are drawn; a ridden animal and the pets always are. */
	animals: boolean;
	/** The share of the view's pixels the world is drawn at, each way, 0.25 to 1. */
	resolution: number;
	cloud_cover: number;
	cloud_density: number;
	wind_m_s: number;
	cloud_change: number;
	exposure: number;
	bloom: number;
	bloom_threshold: number;
	haze: number;
	water_clarity: number;
	/** How far a picture that moves plays, metres; past it, a still. */
	video_play_m: number;
	/** How far a picture is drawn, metres. */
	picture_reach_m: number;
	/** How far a model, cube, landmark or portal is drawn, metres a block of its longest side. */
	model_reach_m: number;
	/** How far a plant is drawn leaf by leaf, metres; past it, in clumps. */
	foliage_near_m: number;
	/** How far a plant is drawn in clumps, metres; past it, a card a mass. */
	foliage_mid_m: number;
	/** How far plants are drawn at all, metres; past it a wood is its ground's colour. */
	foliage_reach_m: number;
	/** The share of the flowers and shrubs drawn, 0 to 1, while each is on. */
	undergrowth: number;
	tone_map: ToneMap;
}

/** The pictures offered whole: `light` for a phone or a Pi, `medium` between, `high` what the engine starts with. */
export interface Presets {
	light: Effects;
	medium: Effects;
	high: Effects;
}

/** What the voice board lists: every voice, the core's and the trials, and how many seeds a strike turns through. */
export interface Board {
	variants: number;
	voices: BoardVoice[];
}

export interface BoardVoice {
	name: string;
	/** A recipe for a plugin to come: nothing in the world strikes it. */
	trial: boolean;
	seconds: number;
}

export type Command =
	| { type: 'set_recipe'; recipe: Recipe }
	| { type: 'set_mode'; mode: Mode }
	/** `path` is an asset reference: a path from `assets/manifest.json`, such as `avatars/Kyle.vrm`, or a URL. */
	| { type: 'set_avatar'; path: string }
	/** Nothing while the account wears its own avatar, as `next_avatar`. */
	| { type: 'random_avatar' }
	| { type: 'next_avatar' }
	/** Fields left out take the engine's default. */
	| { type: 'set_effects'; effects: Partial<Effects> }
	/**
	 * Stand at a place code, `4-K7M42Q`, at once: an arrival, how a shared
	 * address opens where it says and a body is born at home, under the
	 * veil. A short code names a box and you land in the middle of it.
	 */
	| { type: 'go_to'; place: string }
	/**
	 * Travel to a place code as `go_to` reads it, through the passage a
	 * portal takes a body by: how a place chosen inside the world is
	 * reached, a line of the chart, a place shared in chat. Never veiled.
	 */
	| { type: 'travel'; place: string }
	/** What to be called: in Hello and, while online, at once. Empty is a name too. */
	| { type: 'set_name'; name: string }
	/** The plugin with the pointer puts it down, with what it had in hand: a panel of the page opened. */
	| { type: 'put_down' }
	/** A visitor's home and favourites in the chart, as this browser kept them from `choices`: landmarks by their names in the world. An account's are the world's, and these are not. */
	| { type: 'set_choices'; home: number | null; favourites: number[] }
	/**
	 * Taps what the prompt offers, as E does; with `plugin` and `id`, that offer within reach, whichever way the body faces.
	 * With `hold`, holds it as E held does: `ring` says it closing until it is done or the offer leaves.
	 */
	| { type: 'act'; plugin?: string; id?: number; hold?: boolean }
	/** Presses controls for one turn while they are lent: `left`, `right`, `up`, `down`, `jump`, `sprint`, `start`. */
	| { type: 'press'; controls: string[] }
	/** Takes the controls back from whoever holds them. */
	| { type: 'leave' }
	/** Asks for every offer within reach: answered by `offers`. */
	| { type: 'offers' }
	/** Asks for every animal drawn near: answered by `animals`. */
	| { type: 'animals' }
	/** Asks for every marker given and not hidden, wherever it is: answered by `markers`. A marker's choices are commands, sent as they are. */
	| { type: 'markers' }
	/** The kinds of marker the person hides, in place of the last: `favourite`. Kept by the front end and handed over on each start. */
	| { type: 'hide_markers'; kinds: string[] }
	/** The body sounds one of the core's voices by its name, `whistle`, heard at its mouth by this client and every peer within its reach. The whistle calls the body's pet within 500 m: `whistled` says whether and how it comes. */
	| { type: 'sound'; voice: string }
	/** A visitor's pet, as this browser kept it from `pet`, handed to the engine before any world answers; null lets it go. An account's is the world's, and the engine leaves this be. */
	| { type: 'set_pet'; pet: Pet | null }
	/** Names the body's pet: trimmed, no control character, at most 32 bytes. */
	| { type: 'name_pet'; name: string }
	/** Makes the world's day this many minutes long, one of the lengths `day` lists: the world admin's to choose. The sun jumps, for everyone here at once. */
	| { type: 'set_day'; minutes: number };

/** Where a pet stands and which way it faces, since when: a sector, a place in it in blocks, a heading in radians, the world's clock in milliseconds. */
export interface Spot {
	sector: number;
	u: number;
	v: number;
	heading: number;
	at_ms: number;
}

/**
 * What an animal was born with, as values (gaia's `Traits`): colours are linear RGB, `socks` one a leg, 0 to 3. The
 * engine reads it; the page only keeps it.
 */
export interface Traits {
	coat: string;
	hair: [number, number, number];
	mane: [number, number, number];
	points: [number, number, number];
	muzzle: [number, number, number];
	hoof: [number, number, number];
	face: string;
	socks: [number, number, number, number];
	blots: { y: number; z: number; radius_m: number }[];
	size: number;
	build: number;
	mane_m: number;
	mane_falls: boolean;
	tail_m: number;
	feathers: boolean;
	wings: boolean;
	/** Cents from its kind's voice; absent or null in traits kept before it was drawn: the world fills it. */
	voice_cents?: number | null;
}

/**
 * A pet as the world told it. `name` is empty until named; `from` is where a called pet set off, `from_mps` how fast; `session` is a
 * visitor's, 0 for an account's; `traits`, what it was born with, its wings among them, kept since it was tamed.
 */
export interface Pet {
	number: number;
	name: string;
	spot: Spot;
	from?: Spot;
	from_mps?: number;
	owner: string;
	mine: boolean;
	session?: number;
	traits?: Traits;
}

/** What holding the prompt's key does, and how long its ring takes to close once the press is more than a tap. */
export interface Hold {
	verb: string;
	ring_s: number;
}

/**
 * The prompt: the thing in front of the body, at a point of the world, or what the body is on, to leave it; and what a
 * tap and a hold on it do. The words are keys of the offering plugin's words; an offer has a tap, a hold or both.
 */
export interface Prompt {
	plugin: string;
	id: number;
	on: 'point' | 'body';
	thing: string;
	tap?: string;
	hold?: Hold;
}

/** Where the prompt's point is on the screen, as fractions of it, and the body it keeps clear of. */
export type PromptAt = Extract<EngineEvent, { type: 'prompt_at' }>;

/** An offer within reach, as `offers` lists it. */
/**
 * An animal drawn near: `number` is what a pet is told by, `place` what `go_to` takes, `away_m` along the ground
 * from the feet, `bearing_deg` the way it faces clockwise from north (null at a pole), `pet` whose it is, null while
 * it is its herd's.
 */
export interface Animal {
	number: number;
	kind: string;
	place: string;
	away_m: number;
	bearing_deg: number | null;
	ridden: boolean;
	pet: { name: string; mine: boolean } | null;
}

/**
 * A choice pressing a marker offers: the verb, a key of its giver's words, and the command it sends as it is. One with
 * `confirm` cannot be undone and asks first in a Dialog: the key of the giver's words that says plainly what it does,
 * `{name}` the label.
 */
export interface MarkerChoice {
	verb: string;
	command: Command | PluginCommand;
	confirm?: string;
}

/**
 * What a giver points out, a plugin or an owner of the core (`parts`): what it marks, an animal by its number or a
 * point of the world in metres; `kind` the word a person hides it by; `thing` a key of the giver's words said where
 * the label is empty; the label a person's text shown as text; the glyph an icon of `$lib/ds`; `note` a key of the
 * giver's words said beside the label; and what pressing it offers.
 */
export interface Marker {
	giver: string;
	id: number;
	on: { animal: number } | { point: [number, number, number] };
	kind: string;
	thing: string;
	label: string;
	glyph: string;
	note?: string;
	choices: MarkerChoice[];
}

/**
 * Where a marker is for the eye: fractions of the view from its top left, on the screen or, with `edge_deg`, on its
 * edge, the thing lying at that angle from the middle, clockwise from the screen's up; how far along the ground; and
 * whether its nametag already hangs over it in view, where nothing more is drawn.
 */
export interface MarkerAt {
	giver: string;
	id: number;
	x: number;
	y: number;
	edge_deg?: number;
	distance_m: number;
	tagged: boolean;
}

/** A marker as an agent reads it: `place` what `go_to` takes, `away_m` along the ground, `bearing_deg` the way to it from the body, clockwise from north. */
export interface Pointed extends Marker {
	place: string;
	away_m: number;
	bearing_deg: number | null;
}

export interface Reached extends Prompt {
	/** From the feet, metres: a point's alone. */
	away_m?: number;
	/** Whether the body faces it. */
	in_front: boolean;
}

/**
 * A plugin's command: its `type` is the plugin's name and the command's,
 * `chat.say`. The engine hands it to that plugin, and refuses it when the
 * plugin is off in this world.
 */
export type PluginCommand = { type: `${string}.${string}`; [field: string]: unknown };

/** A plugin's event, as the plugin wrote it: `chat.said`. */
export type PluginEvent = { type: `${string}.${string}`; [field: string]: unknown };

/** The line of what is on that says a world has life: `gaia`, the core's system its admin switches beside the plugins. */
export const lifeLine = 'gaia';

/** A plugin that is on in this world, as the world's statement says it. */
export interface PluginOn {
	name: string;
	version: number;
}

/** `outdated`: offline for good, the world refused this build's protocol; a page reloads once for the world's own. */
export type SessionStatus = 'offline' | 'connecting' | 'online' | 'outdated';
/** Why an admin's ask for a day did not land: past the session's burst, or the world could not keep it. */
export type DayRefused = 'rate' | 'fault';

/** Someone else in the world. The name is their own, or empty. */
export interface PeerInfo {
	session: number;
	name: string;
	visitor: boolean;
}

/** A head on the screen: fractions of the viewport from the top left, and how far. */
export interface Anchor {
	session: number;
	x: number;
	y: number;
	distance_m: number;
}

/** A named part on the screen, a landmark: its name, the point over its star as fractions of the viewport from the top left, and how far. */
export interface Label {
	id: number;
	text: string;
	x: number;
	y: number;
	distance_m: number;
}

/**
 * A cabinet whose game waits for its player: the middle of its screen as
 * fractions of the viewport, how far it is, and how much of its window
 * remains, a share from 1 to 0, and in seconds.
 */
export interface Waiting {
	id: number;
	x: number;
	y: number;
	distance_m: number;
	remains: number;
	remains_s: number;
}

/**
 * A step of the engine's start, reached: the adapter the browser gave, its
 * device open, the renderer's pipelines made, the first frame handed to the
 * GPU, the GPU done with it. Said at every start.
 */
export type StartStep =
	| {
			step: 'adapter';
			/** As the browser says it; empty where it hides it. */
			name: string;
			backend: 'webgpu' | 'webgl2';
			/** The largest side of a 2D texture, texels. */
			texture_px: number;
			/** The largest buffer, MiB. */
			buffer_mib: number;
	  }
	/** What WebGPU says of the adapter once a device is open: vendor, architecture, device, description. */
	| { step: 'device'; about: string[] }
	| { step: 'pipelines' }
	/** The canvas's size, pixels, and the screen's pixels to a CSS pixel. */
	| { step: 'frame'; width: number; height: number; ratio: number }
	| { step: 'drawn' };

/** A stick a finger holds on the canvas, in CSS pixels from its top left. */
export interface Stick {
	/** Its centre, which follows the finger pulled past the reach. */
	center: [number, number];
	/** The finger. */
	at: [number, number];
	/** How far from the centre the finger walks before it runs and drags the centre. */
	reach: number;
}

export type EngineEvent =
	| { type: 'ready'; generator_version: number }
	/** The web shell's own, not the client's: a stick awake, whenever it moves, and null once it goes. */
	| { type: 'stick'; stick: Stick | null }
	/** The web shell's own: a step of the start, `ms` on the page's clock (`performance.now()`). */
	| ({ type: 'start'; ms: number } & StartStep)
	/** The link to the world server, whenever it changes. */
	/** `level` is null until a world has spoken; its last word stands through a dropped link. */
	| { type: 'session'; status: SessionStatus; session: number | null; level: Level | null }
	/** Who else is here, whenever that changes. Empty when offline. */
	| { type: 'peers'; peers: PeerInfo[] }
	/** How long the world's day is, minutes, whenever that changes and once at the start; `lengths` are the ones its admin chooses among. */
	| { type: 'day'; minutes: number; lengths: number[]; refused: DayRefused | null }
	/** The plugins that are on in this world, whenever that changes. Empty when offline. */
	| { type: 'statement'; plugins: PluginOn[] }
	/** Every head in view, yours included, every frame while there is one and once empty after. */
	| { type: 'anchors'; anchors: Anchor[] }
	/** Every landmark in view within a nametag's reach, every frame while there is one and once empty after. */
	| { type: 'labels'; labels: Label[] }
	/** Every cabinet in view whose game waits for the player who left it, every frame while there is one and once empty after. */
	| { type: 'waiting'; waiting: Waiting[] }
	| { type: 'recipe_changed'; recipe: Recipe }
	/** A command was refused. The message is for logs, not for people. */
	| { type: 'rejected'; message: string }
	/** Where the body is born: the place of this person's home as the first chart after the welcome says it, null with none. Once a link: a page that asked no place goes there. */
	| { type: 'home'; place: string | null }
	/** A visitor's home and favourites, whenever they change: keep them in this browser and hand them back with `set_choices`. */
	| { type: 'choices'; home: number | null; favourites: number[] }
	/** This body's own pet whenever it changes: tamed (its name empty until named), named, left, called; null once it has none. A visitor's: keep it in this browser and hand it back with `set_pet`. */
	| { type: 'pet'; pet: Pet | null }
	/** The body whistled with a pet: how far it was, metres, whether it answers, and how long it is out of sight before it comes into it, seconds. Within earshot, 150 m, it runs in at once, 0; past it, under two seconds, and it gallops the last stretch; past 500 m it does not answer. */
	| { type: 'whistled'; away_m: number; coming_s: number; answered: boolean }
	/** Nothing is left to build for this view: the world is drawn whole. Again after a leap or a new recipe. */
	| { type: 'settled' }
	| { type: 'mode_changed'; mode: Mode }
	/** What the prompt offers, whenever that changes; null when nothing is offered. */
	| { type: 'prompt'; prompt: Prompt | null }
	/**
	 * Where the prompt's point, the thing's top, is on the screen, every frame while the prompt is a point in view, as a
	 * head's anchor; and where the body stands on the screen while it is seen, `[left, top, right, bottom]`, which the
	 * prompt keeps clear of.
	 */
	| { type: 'prompt_at'; x: number; y: number; distance_m: number; body: [number, number, number, number] | null }
	/** How far the ring around the prompt's key has closed, 0 to 1, every frame a hold is held past a tap; 1 as it closes, then null once it is done, let go or broken. */
	| { type: 'ring'; closed: number | null }
	/** An offer was acted on, by the key, a finger or an agent: the card showing it sweeps its ring on a tap, pulses on a hold. */
	| { type: 'acted'; plugin: string; id: number; act: 'tap' | 'hold' }
	/** Every offer within reach: the answer to `offers`. */
	| { type: 'offers'; offers: Reached[] }
	/** Every animal drawn near, nearest first: the answer to `animals`. */
	| { type: 'animals'; animals: Animal[] }
	/** What the givers point out and the person does not hide, whenever that changes. */
	| { type: 'markers_changed'; markers: Marker[] }
	/** Where the markers shown are for the eye, the nearest few: each frame one moved past a pixel, and once empty after the last. */
	| { type: 'markers_at'; at: MarkerAt[] }
	/** Every marker given, nearest first, with where its thing is: the answer to `markers`. */
	| { type: 'markers'; markers: Pointed[] }
	/** The paints a cell can take, as `#rrggbb`, in order: the world's palette. Once, at the start. */
	| { type: 'palette'; colors: string[] }
	/**
	 * The avatar you are in, or on your way into. `path` is the choice: keep it as this browser's,
	 * unless it is the account's own.
	 * While `worn` is false the file is on its way and the body stays as it was: show the wait.
	 * A null path is the box figure, nothing wanted.
	 */
	| { type: 'avatar_changed'; path: string | null; worn: boolean }
	| { type: 'effects_changed'; effects: Effects }
	/** Which plugin has the pointer, a tool in hand, or none. */
	| { type: 'pointer_changed'; plugin: string | null }
	/** Which plugin the controls are lent to, or none: while one holds them the keys are its. */
	| { type: 'controls_changed'; plugin: string | null }
	| {
			type: 'stats';
			fps: number;
			altitude_m: number;
			speed_mps: number;
			/** Where the body is, as a person says it: `4-K7M42Q`, `m4-K7M42Q@40` on the moon. */
			place: string;
			/**
			 * The same, plus the way of looking: what a link carries, so whoever
			 * opens it stands where you stood seeing what you saw. Put it in the
			 * address bar and hand it back whole; never take it apart here.
			 */
			pose: string;
			/** Degrees clockwise from north, or null at a pole. */
			bearing_deg: number | null;
			/** The hour of the day here, minutes past midnight (0 to 1439): the sun's at the middle meridian of the place's time zone, the same for everyone in it. Null on the moon. */
			hour_min: number | null;
			/** The pace the body goes at in a way that goes in paces, a word of its plugin's; null otherwise. */
			pace: string | null;
			/** Whether a whistle calls someone now: the body has a pet, is not on it, and stands where animals live. */
			calls: boolean;
			/** Whether the body flies, on its own or aloft on what it rides, a pegasus: what rises and sinks it. */
			flying: boolean;
	  };

// Every event type of the core, once: a record over the union, so a type
// added to `EngineEvent` and left out here fails the check.
const CORE_EVENTS = {
	ready: true,
	stick: true,
	start: true,
	recipe_changed: true,
	rejected: true,
	settled: true,
	mode_changed: true,
	prompt: true,
	prompt_at: true,
	ring: true,
	acted: true,
	offers: true,
	animals: true,
	markers_changed: true,
	markers_at: true,
	markers: true,
	avatar_changed: true,
	effects_changed: true,
	pointer_changed: true,
	controls_changed: true,
	stats: true,
	session: true,
	peers: true,
	day: true,
	statement: true,
	anchors: true,
	labels: true,
	waiting: true,
	palette: true,
	home: true,
	choices: true,
	pet: true,
	whistled: true
} as const satisfies Record<EngineEvent['type'], true>;
const EVENT_TYPES: ReadonlySet<string> = new Set(Object.keys(CORE_EVENTS));

/** Whether a value has the shape of an animal's traits: each field there, of its type. */
function traits(value: unknown): value is Traits {
	if (typeof value !== 'object' || value === null) return false;
	const t = value as Record<string, unknown>;
	const numbers = (x: unknown, length: number) => Array.isArray(x) && x.length === length && x.every(Number.isFinite);
	const blot = (b: unknown) =>
		typeof b === 'object' && b !== null && ['y', 'z', 'radius_m'].every((key) => Number.isFinite((b as Record<string, unknown>)[key]));
	return (
		typeof t.coat === 'string' &&
		typeof t.face === 'string' &&
		['hair', 'mane', 'points', 'muzzle', 'hoof'].every((key) => numbers(t[key], 3)) &&
		numbers(t.socks, 4) &&
		(t.socks as number[]).every((sock) => Number.isInteger(sock) && sock >= 0 && sock <= 255) &&
		Array.isArray(t.blots) &&
		t.blots.every(blot) &&
		['size', 'build', 'mane_m', 'tail_m'].every((key) => Number.isFinite(t[key])) &&
		['mane_falls', 'feathers', 'wings'].every((key) => typeof t[key] === 'boolean') &&
		(t.voice_cents === undefined || t.voice_cents === null || Number.isFinite(t.voice_cents))
	);
}

/** Reads a pet a browser kept as JSON. Null for nothing kept, or for what is no pet. */
export function parsePet(text: string | null): Pet | null {
	try {
		const pet = JSON.parse(text ?? 'null');
		const spot = (value: unknown): value is Spot =>
			typeof value === 'object' &&
			value !== null &&
			['sector', 'u', 'v', 'heading', 'at_ms'].every((key) => Number.isFinite((value as Record<string, unknown>)[key]));
		if (typeof pet !== 'object' || pet === null) return null;
		if (!Number.isSafeInteger(pet.number) || pet.number <= 0 || typeof pet.name !== 'string' || !spot(pet.spot)) return null;
		if (pet.from !== undefined && !spot(pet.from)) return null;
		// Traits the engine could not read are left for the world to draw again.
		const kept = traits(pet.traits) ? { traits: pet.traits } : {};
		return {
			number: pet.number,
			name: pet.name,
			spot: pet.spot,
			...(pet.from ? { from: pet.from } : {}),
			...(Number.isFinite(pet.from_mps) && pet.from_mps > 0 ? { from_mps: pet.from_mps } : {}),
			...kept,
			owner: '',
			mine: true
		};
	} catch {
		return null;
	}
}

/** A visitor's home and favourites, as `choices` says them and `set_choices` hands them back. */
export type Choices = Omit<Extract<EngineEvent, { type: 'choices' }>, 'type'>;

/** Reads choices a browser kept as JSON. Null for nothing kept, or for what is no choices. */
export function parseChoices(text: string | null): Choices | null {
	try {
		const { home, favourites } = JSON.parse(text ?? 'null') ?? {};
		const id = (n: unknown): n is number => Number.isSafeInteger(n) && (n as number) > 0;
		if (home !== null && !id(home)) return null;
		if (!Array.isArray(favourites) || !favourites.every(id)) return null;
		return { home, favourites };
	} catch {
		return null;
	}
}

/** Whether an event is a plugin's: its `type` has the plugin's name and a dot before the event's. */
export function isPluginEvent(event: EngineEvent | PluginEvent): event is PluginEvent {
	return event.type.includes('.');
}

/** Parses one event, the core's or a plugin's. Unknown types and malformed payloads yield null. */
export function parseEvent(json: string): EngineEvent | PluginEvent | null {
	try {
		const value: unknown = JSON.parse(json);
		if (typeof value === 'object' && value !== null && 'type' in value && typeof value.type === 'string') {
			if (EVENT_TYPES.has(value.type)) return value as EngineEvent;
			if (/^[a-z_]+\.[a-z_]+$/.test(value.type)) return value as PluginEvent;
		}
	} catch {
		// not JSON: ignored like any unknown event
	}
	return null;
}
