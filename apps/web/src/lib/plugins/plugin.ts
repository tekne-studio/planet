/**
 * A plugin's half in the web front end, and what the stage offers it: a layer
 * over the world, the core's readings as props, and the seam under the
 * plugin's own name.
 */
import type { Component } from 'svelte';
import type { Anchor, CabinetTemplate, CartridgeCover, Command, Mode, PeerInfo } from '$lib/engine';

/** The seam, as one plugin reaches it. */
export interface Seam {
	/** Sends a command of this plugin: `say` goes out as `chat.say`. */
	command(kind: string, body?: Record<string, unknown>): void;
	/** Sends one of the core's own commands: go to a place, take a mode. */
	core(command: Command): void;
	/**
	 * Hears this plugin's events, each under its name without the plugin's:
	 * `said` for `chat.said`. Returns how to stop hearing.
	 */
	listen(hear: (kind: string, event: Record<string, unknown>) => void): () => void;
	/** Lets the world go, so a form in the layer has the keys and the pointer. */
	release(): void;
	/** Hands the world back. */
	take(): void;
	/**
	 * Sends a file to the world under the page's key, and resolves to the
	 * asset's name, `asset://<sha256>.<ext>` (`$lib/files`). Throws a
	 * `Refused` with the world's code: `kind`, `size`, `level`, `key`.
	 */
	upload(file: Blob): Promise<string>;
	/**
	 * Weighs a file the world judges by what it holds, before it is sent:
	 * a 3D model (`.glb`) against what a world draws of one, a cartridge
	 * (`.wasm`) against the contract. Throws a `Refused` with why: for a
	 * model `broken`, `triangles`, `texels`, `materials` or `vertices`; for
	 * a cartridge the code the world refuses it with, the emulator's
	 * `check`, which the world judges by too.
	 */
	weigh(file: Blob): Promise<void>;
	/** The layout a cabinet's skin is painted on, as the engine lays a skin. Null with no engine built. */
	cabinetTemplate(): Promise<CabinetTemplate | null>;
	/**
	 * The cover of a cartridge the world holds, `asset://<sha256>.wasm`,
	 * read from its file: a game this version ships or one someone sent.
	 * Null with no engine built or a file that cannot be read.
	 */
	cover(cart: string): Promise<CartridgeCover | null>;
}

/** What a plugin's layer is mounted with, while its plugin is on in this world. */
export interface LayerProps {
	seam: Seam;
	/** Whether the link to the world is open. */
	online: boolean;
	/** This client's own session, while online. */
	me: number | null;
	/**
	 * Whether this session builds here: the world said its level builds, and
	 * the screen is no touch screen, which walks a world and builds nothing.
	 */
	builds: boolean;
	/** A touch screen: fingers and no keys, a small screen held in the hand. */
	touch: boolean;
	/** The paints a cell can take, as `#rrggbb`, in order: the world's palette. */
	palette: string[];
	peers: PeerInfo[];
	/** Where every head in view is on the screen, each frame. */
	anchors: Anchor[];
	/** How the body moves: on foot, in flight, or in a way a plugin gives, `ride.board`. */
	mode: Mode;
	/** Whether the body's controls are lent to a plugin: the keys are its, Enter and Escape among them. */
	lent: boolean;
	/** How fast the body goes, metres a second, as the engine last said; null before it says. */
	speed_mps: number | null;
	/** How high the feet are over the ground or the sea under them, metres, as the engine last said; null before it says. */
	altitude_m: number | null;
	/** The pace the body goes at in a way that goes in paces, by the name its plugin gave it; null otherwise. */
	pace: string | null;
}

/** The core's readings a layer is mounted with: every prop but the seam. */
export type Readings = Omit<LayerProps, 'seam'>;

/** Where the page reads each reading from, one function a reading. */
export type Reads = { readonly [Key in keyof Readings]: () => Readings[Key] };

/**
 * A plugin's props, built once for as long as it is mounted: the seam a
 * value, which never changes, and each reading a getter of its own. Svelte
 * tracks what a getter reads, so an effect that reads the seam reruns never,
 * and one that reads `mode` reruns when the mode changes, not each frame the
 * speed does. A props object built anew for each change would rerun every
 * effect of every layer on any reading's change.
 */
export function layerProps(seam: Seam, reads: Reads): LayerProps {
	const props = { seam } as LayerProps;
	for (const key of Object.keys(reads) as (keyof Readings)[]) {
		Object.defineProperty(props, key, { get: reads[key], enumerable: true });
	}
	return props;
}

/** A key of a plugin, as the help panel lists it while the plugin is on. */
export interface Hint {
	/** What is pressed: said as it is, `B`, or in the plugin's words where words name it. */
	keys: string | (() => string);
	/** What it does, in the plugin's words. */
	does: () => string;
}

/** A plugin as the web front end holds it. Its words are its own (`words` in `$lib/i18n`). */
export interface WebPlugin {
	/** The name the world's statement says. The engine says which are on, at the version it speaks. */
	name: string;
	/** What it is called where an admin switches it. */
	label: () => string;
	/** What it draws over the world. A plugin with nothing to show has none. */
	Layer?: Component<LayerProps>;
	/**
	 * Its button over the world, one control: the page lays every plugin's
	 * in one group at the bottom right, beside the thumbs on a touch screen,
	 * and hides the group while the controls are lent.
	 */
	Button?: Component<LayerProps>;
	/** The keys it asks for, for the help panel. */
	hints?: readonly Hint[];
	/** Reads a key of its own words: the thing and the verbs of what it offers to the prompt. */
	words?: (key: string) => string;
}
