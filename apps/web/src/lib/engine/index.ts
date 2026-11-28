/**
 * Typed wrapper over the command/event seam of the engine: the Rust client
 * compiled to WASM by `bun run wasm` into `./pkg` (gitignored, may be absent).
 * The web UI is only a front-end: it sends commands and renders events. What
 * a command does lives in Rust.
 */
import { parseEvent, type Board, type Command, type EngineEvent, type Loudness, type PluginCommand, type PluginEvent, type Presets } from './protocol';
import { hostEmulator, partUrl } from './emulator';
import { rehosted } from './rehosted';
import type { Start } from './start';

export * from './protocol';

/** A running engine bound to one canvas. `free()` stops its loop and listeners. */
export interface Engine {
	command(command: Command | PluginCommand): void;
	/** Presses a key or lets it go, by the name the web gives it (`Space`), as the keyboard does: a button on the screen in place of a key. */
	key(code: string, down: boolean): void;
	/** The ground a recipe names. Sent before the recipe that names it. */
	set_field(bytes: Uint8Array): void;
	/** Where the world's files are: the base an `asset://` reference is read under. */
	set_files(base: string): void;
	/** How loud each group of what is heard is: the shell's mix, heard at once. */
	set_loudness(loudness: Loudness): void;
	/** The part of the canvas the page leaves clear of what it lays over it: a game played fills it. Null leaves room round the middle, as for a page with keys. */
	set_clear(clear: Clear | null): void;
	/** Opens the link to a world server at a socket URL, the key included. */
	connect(url: string): void;
	disconnect(): void;
	/** What the engine holds down right now. */
	held(): Held;
	/** When the engine last handed a picture to the screen, on the page's clock; 0 before the first. */
	presented(): number;
	free(): void;
}

/** What the engine holds down right now: what a readout shows of a key that stays held. */
export interface Held {
	/** The keys of the keyboard, by the names the web gives them (`KeyW`). */
	codes: string[];
	/** The core's keys (`Forward`, `Up`). */
	keys: string[];
	/** Where a stick points, `[right, forward]`. */
	stick: [number, number];
	/** Whether the engine was last told the person is away. */
	away: boolean;
}

/** A part of the canvas, CSS pixels from its top left: `[left, top, right, bottom]`. */
export type Clear = readonly [number, number, number, number];

/** A loaded engine module, ready to bind to a canvas. */
export interface EngineModule {
	/** Rejects when the browser cannot give the engine a GPU surface. `start` is how it is asked to start; empty, every default. */
	create(canvas: HTMLCanvasElement, onevent: (event: EngineEvent | PluginEvent) => void, start?: Start): Promise<Engine>;
	/** Why a 3D model's file is over what a world draws of one, or `''` within it. */
	weigh(bytes: Uint8Array): string;
	/** The light, the medium and the high picture, as the engine defines them. */
	presets(): Presets;
	/** Every voice, for the voice board. */
	voices(): Board;
	/** A voice rendered for a seed at a rate, mono: what a strike of it plays. Empty when none is called so. */
	renderVoice(name: string, seed: number, rate: number): Float32Array;
	/** The layout a cabinet's skin is painted on, as the engine lays a skin on the body. */
	cabinetTemplate(): CabinetTemplate;
}

/** The layout a cabinet's skin is painted on: the picture, at the skin's size, and where each of its places is named. */
export interface CabinetTemplate {
	image: ImageData;
	labels: TemplateLabel[];
}

/** A place of the skin named on its template: the word goes in the middle of `rect`, texels `[x0, y0, x1, y1]`, in `ink`, sRGB. */
export interface TemplateLabel {
	/** `left`, `right`, `trim`, `top`, `marquee`, `game`, `deck` or `panel`. */
	name: string;
	rect: [number, number, number, number];
	ink: [number, number, number];
}

/** The shape `wasm-bindgen --target web` emits for `shell-web`. */
interface RawEngine {
	command(json: string): void;
	key(code: string, down: boolean): void;
	set_field(bytes: Uint8Array): void;
	set_files(base: string): void;
	set_loudness(json: string): void;
	set_clear(clear?: Float64Array): void;
	set_carts(make: () => Worker): void;
	set_games(make: (context: AudioContext) => Promise<AudioWorkletNode>): void;
	connect(url: string): void;
	disconnect(): void;
	held(): string;
	presented_ms(): number;
	free(): void;
}
interface RawModule {
	default(): Promise<unknown>;
	Engine: { create(canvas: HTMLCanvasElement, onEvent: (json: string) => void, start: string): Promise<RawEngine> };
	weigh_glb(bytes: Uint8Array): string;
	presets(): string;
	voices(): string;
	render_voice(name: string, seed: number, rate: number): Float32Array;
	cabinet_template(): { width: number; height: number; rgba(): Uint8Array; labels(): string; free(): void };
}

// A glob resolves to nothing when the package was never built, so the app
// type checks, builds and runs without it.
const builds = import.meta.glob<RawModule>('./pkg/shell_web.js');

/** The chip's bytes, once its processor is loaded into `context`. */
const chipsIn = new WeakMap<BaseAudioContext, Promise<ArrayBuffer>>();

/**
 * A node a running game sounds through: the arcade's chip in an
 * AudioWorklet, one channel out, for the engine to pan at the game's screen.
 */
async function gameSound(context: AudioContext): Promise<AudioWorkletNode> {
	let module = chipsIn.get(context);
	if (!module) {
		module = Promise.all([
			partUrl('chip.wasm')
				.then((url) => fetch(url))
				.then((answer) => answer.arrayBuffer()),
			partUrl('processor.js').then((url) => context.audioWorklet.addModule(url))
		]).then(([bytes]) => bytes);
		// One that failed is tried again by the next game.
		module.catch(() => chipsIn.delete(context));
		chipsIn.set(context, module);
	}
	return new AudioWorkletNode(context, 'chip', {
		numberOfInputs: 0,
		outputChannelCount: [1],
		processorOptions: { module: await module }
	});
}

let loading: Promise<EngineModule | null> | undefined;

async function load(): Promise<EngineModule | null> {
	const build = builds['./pkg/shell_web.js'];
	if (!build) return null;
	const raw = await build();
	await raw.default();
	return {
		async create(canvas, onevent, start = {}) {
			const engine = await raw.Engine.create(
				canvas,
				(json) => {
					const event = parseEvent(json);
					if (event) onevent(event);
				},
				JSON.stringify(start)
			);
			// The cabinets' games run in a worker of their own, made the first
			// time one is in view.
			engine.set_carts(() => new Worker(new URL('./cartridges.worker.ts', import.meta.url), { type: 'module' }));
			engine.set_games(gameSound);
			return {
				command: (command) => engine.command(JSON.stringify(command)),
				key: (code, down) => engine.key(code, down),
				set_field: (bytes) => engine.set_field(bytes),
				set_files: (base) => engine.set_files(base),
				set_loudness: (loudness) => engine.set_loudness(JSON.stringify(loudness)),
				set_clear: (clear) => engine.set_clear(clear ? new Float64Array(clear) : undefined),
				connect: (url) => engine.connect(url),
				disconnect: () => engine.disconnect(),
				held: () => JSON.parse(engine.held()) as Held,
				presented: () => engine.presented_ms(),
				free: () => engine.free()
			};
		},
		weigh: (bytes) => raw.weigh_glb(bytes),
		presets: () => JSON.parse(raw.presets()) as Presets,
		voices: () => JSON.parse(raw.voices()) as Board,
		renderVoice: (name, seed, rate) => raw.render_voice(name, seed, rate),
		cabinetTemplate() {
			const template = raw.cabinet_template();
			try {
				const rgba = new Uint8ClampedArray(template.rgba());
				return {
					image: new ImageData(rgba, template.width, template.height),
					labels: JSON.parse(template.labels()) as TemplateLabel[]
				};
			} finally {
				template.free();
			}
		}
	};
}

/** Loads and initialises the WASM module once. Null when it was never built. */
export function loadEngine(): Promise<EngineModule | null> {
	return (loading ??= load());
}

/** A use of the emulator on the page; null when it was never copied. */
const onPage = rehosted(hostEmulator);

/**
 * Why a cartridge breaks the contract, as the world will judge it: the code
 * it answers with, or `''` when the file keeps it. With no emulator copied,
 * the world alone judges.
 */
export async function checkCartridge(file: Uint8Array): Promise<string> {
	return (await onPage((emulator) => emulator.check(file))) ?? '';
}

/**
 * A cartridge's cover, as a cabinet's panel shows it: what its file says of
 * it and its picture, read from the file and never by running it. A file
 * with no cover says nothing, and its picture is the stand-in, the start of
 * its name on a stopped screen's ground. The text is the font's, capitals
 * and punctuation; a page puts it on the page as text, never as markup.
 */
export interface CartridgeCover {
	/** Null for a file with no cover, as are `maker` and `year`. */
	title: string | null;
	maker: string | null;
	year: number | null;
	/** A cartridge's screen's size, four bytes a pixel. */
	picture: ImageData;
}

/** The cover of `file`, whose hash is `name`; null with no emulator copied. */
export async function readCover(file: Uint8Array, name: string): Promise<CartridgeCover | null> {
	return onPage((emulator) => {
		const { said, rgba } = emulator.cover(file, name);
		return {
			title: said?.title ?? null,
			maker: said?.maker ?? null,
			year: said?.year ?? null,
			picture: new ImageData(new Uint8ClampedArray(rgba.buffer), emulator.width, emulator.height)
		};
	});
}
