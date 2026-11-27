<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import { Alert, Veil } from '$lib/ds';
	import { t } from '$lib/i18n';
	import { keep, kept } from '$lib/kept';
	import { linkUp, reloadOnce } from './outdated';
	import { DamagedField, fetchField } from '$lib/fields';
	import { fieldId, type Recipe } from '$lib/world';
	import type { Link } from '$lib/instance';
	import { touch } from '$lib/touch.svelte';
	import {
		isPluginEvent,
		loadEngine,
		parseChoices,
		parsePet,
		type Clear,
		type Command,
		type Effects,
		type Loudness,
		type Engine,
		type EngineEvent,
		type Mode,
		type PluginCommand,
		type PluginEvent
	} from './index';

	// Owns the canvas lifecycle: create on mount, free on destroy. The engine
	// runs its own frame loop, input listeners and resize tracking; this
	// component keeps `recipe` and `mode` in sync through commands, and keeps
	// a veil over the picture until it is the world the page asked for. It
	// veils an arrival, the address bar's place and the home, and never a
	// travel: a place chosen inside the world is the engine's `travel`, its
	// passage the picture from start to end.
	interface Props {
		/** Null until the caller knows the whole recipe; nothing is sent meanwhile. */
		recipe: Recipe | null;
		/** Where to fetch the field a recipe names, when it names one. */
		fieldPath?: string;
		mode: Mode;
		/** Asset path of the avatar to wear. Null wears any the manifest offers. */
		avatar: string | null;
		/** What to be called. Sent before the link opens, and again whenever it changes. */
		name?: string;
		/** How the picture is made. Undefined leaves the engine as it is. */
		effects?: Effects;
		/** How loud each group of what is heard is. Undefined leaves the shell's mix as it is. */
		loudness?: Loudness;
		/** The part of the view the page leaves clear of what it lays over it, where a game played goes. Null when it lays nothing over a game. */
		clear?: Clear | null;
		/** The kinds of marker the person hides. Undefined hides none. */
		hiddenMarkers?: string[];
		/**
		 * The address bar's place, each time a hand other than the page's puts
		 * one there: the link that brought you here, a place pasted over it, a
		 * world entered from another. A new object each time, since the same
		 * place asked for twice is two walks. Honoured once the engine stands
		 * in the world the page asked for, so a place meant for a world you
		 * are not yet in waits for it. Where you stand after that is the
		 * engine's: a place carried over a recipe change by the page lands you
		 * in the sea every time the coastline moves. With none when the engine
		 * first stands in a world, the body is born at the person's home, once
		 * the world says where it is.
		 */
		stand?: { place: string } | null;
		/**
		 * The world's socket, when this is a world and not a preview. The
		 * engine connects once it stands in the recipe, and connects again
		 * whenever the link drops.
		 */
		link?: Link;
		onevent?: (event: EngineEvent | PluginEvent) => void;
	}

	let { recipe, fieldPath, mode, avatar, name = '', effects, loudness, clear = null, hiddenMarkers, stand, link, onevent }: Props = $props();

	/**
	 * A command with no prop of its own: a line said, a place gone to. Dropped
	 * while the engine is not running, as every command is until it runs.
	 */
	export function command(command: Command | PluginCommand) {
		engine?.command(command);
	}

	/** Presses a key or lets it go, by the name the web gives it, as the keyboard does. */
	export function key(code: string, down: boolean) {
		engine?.key(code, down);
	}

	/** Hands the world back: the canvas takes focus and the pointer, where there is one to lock. */
	export function take() {
		canvas?.focus();
		// Without a gesture (an Escape key is none) the browser refuses the
		// pointer: the canvas has the keys, and the next click takes it.
		if (!touch.current) Promise.resolve(canvas?.requestPointerLock()).catch(() => {});
	}

	/** Lets the world go, so a form can have the keys and the pointer. */
	export function release() {
		if (document.pointerLockElement === canvas) document.exitPointerLock();
	}

	/** The engine's own life, before any world is asked of it. */
	type Phase = 'loading' | 'ready' | 'missing' | 'unsupported' | 'failed';
	/** What the veil says while it is down, and `running` once it lifts. */
	type Status = Exclude<Phase, 'ready'> | 'shaping' | 'entering' | 'outdated' | 'unshaped' | 'damaged' | 'refused' | 'running';

	let canvas: HTMLCanvasElement | undefined = $state();
	let engine = $state.raw<Engine>();
	let phase = $state<Phase>('loading');
	/** The field is on its way. */
	let fetching = $state(false);
	/**
	 * The engine stands in the world the page asked for. False from the moment
	 * a recipe is sent until the engine says it stands in it.
	 */
	let standing = $state(false);
	/** That world is drawn whole where you stand: the streamer has nothing left to build. */
	let settled = $state(false);
	/**
	 * The page asked no place when the engine first stood in a world with a
	 * link: the veil waits for the engine to say where the body is born.
	 */
	let homing = $state(false);
	/** The engine has stood in the world the page asked for: a body is born once. */
	let born = false;
	/** The recipe names ground this instance does not serve. */
	let unshaped = $state(false);
	/** The field arrived, and its bytes are not the ground the recipe names. */
	let damaged = $state(false);
	/** The engine refused the recipe the page asked for. */
	let refused = $state(false);
	/**
	 * The world refused this build's protocol: the page reloads for the
	 * world's own build, or, once it has, asks a hand to.
	 */
	let outdated = $state<'reloading' | 'reload' | null>(null);
	// The veil stays down until the picture is the world the page asked for,
	// drawn: the engine builds a world of its own before the page's arrives,
	// the page's waits on a fetch when a field shapes it, and its ground takes
	// a moment to stream in where you land.
	const status = $derived<Status>(
		phase !== 'ready'
			? phase
			: outdated
				? 'outdated'
				: unshaped
					? 'unshaped'
					: damaged
						? 'damaged'
						: refused
							? 'refused'
							: fetching
								? 'shaping'
								: recipe && !(standing && settled && !homing)
									? 'entering'
									: 'running'
	);
	/** The word of the wait, while something is on its way. */
	const wait = $derived(
		status === 'loading' ? t('engine.loading') : status === 'shaping' ? t('engine.shaping') : status === 'entering' ? t('engine.entering') : undefined
	);

	// What the engine last reported or was last told: commands go out only on
	// a real difference, so an event echoed back by the parent sends nothing.
	let engineRecipe = '';
	let engineMode: Mode | undefined;
	let engineAvatar: string | undefined;
	let engineName: string | undefined;
	let engineEffects = '';
	/** The field already handed to the engine, by id. */
	let engineField: string | undefined;
	/** A place from the address bar, waiting for the world it was written for. */
	let pending: string | null = null;

	function arrive() {
		if (!engine || !standing || !pending) return;
		settled = false;
		engine.command({ type: 'go_to', place: pending });
		pending = null;
	}

	// The link: opened once the engine stands in the world the page wants,
	// opened again after a drop, once the page has said its key still stands.
	// The wait doubles from a second to half a minute, and a welcome resets it.
	let linked = false;
	let retryMs = 1000;
	let retry: ReturnType<typeof setTimeout> | undefined;

	async function connect(again = false) {
		retry = undefined;
		const here = engine;
		if (!here || !link || !linked) return;
		if (again) {
			await link.again?.();
			if (here !== engine || !linked) return;
		}
		here.connect(link.url);
	}

	// A world's files are read under its own base, told before any picture
	// is asked for: with the engine, and again with a world.
	$effect(() => {
		if (engine && link) engine.set_files(link.files);
	});

	function receive(event: EngineEvent | PluginEvent) {
		// A plugin's event is its panel's to read: the view passes it on.
		if (isPluginEvent(event)) return onevent?.(event);
		if (event.type === 'recipe_changed') {
			engineRecipe = JSON.stringify(event.recipe);
			// Whether the engine now stands in the world the page wanted. The
			// engine says the recipe in full and the page in what was chosen,
			// so the seed and the version are what the two agree on; the
			// engine checks the whole recipe against the server's on arrival.
			// A world the engine changed on its own, a new seed at the keys,
			// is not one the page awaits: it lifts no veil and drops none.
			const wanted =
				recipe && event.recipe.seed === recipe.seed && event.recipe.generator_version === recipe.generator_version;
			if (wanted) {
				standing = true;
				settled = false;
				if (!born) {
					born = true;
					homing = !pending && !!link;
				}
				arrive();
				if (link && !linked) {
					linked = true;
					connect();
				}
			}
		}
		// The ground is drawn where you stand. Heard for the engine's own world
		// too, which is not the one awaited.
		if (event.type === 'settled' && standing) settled = true;
		// Born at home: the first chart in the world said where it is, and
		// the body goes there as an address would take it, the veil down
		// until the ground there is drawn. A link that fails first lets the
		// body be born where it stands.
		if (event.type === 'home' && homing) {
			homing = false;
			if (event.place) {
				pending = event.place;
				arrive();
			}
		}
		if (event.type === 'session' && event.status === 'offline') homing = false;
		// A visitor's home and favourites are this browser's to keep.
		if (event.type === 'choices') keep('choices', JSON.stringify({ home: event.home, favourites: event.favourites }));
		// So is a visitor's pet, the one a session keeps; an account's is
		// the world's, and its going leaves the browser's be.
		if (event.type === 'pet') {
			if (event.pet?.session) keep('pet', JSON.stringify(event.pet));
			else if (!event.pet && visiting) keep('pet', 'null');
			visiting = Boolean(event.pet?.session);
		}
		// While a world is awaited, its recipe is the only command in flight
		// that can be refused.
		if (event.type === 'rejected' && recipe && !standing && !fetching) refused = true;
		if (event.type === 'session') {
			// Online, nothing is owed: a retry still waiting would close the
			// link that just came up, and the close it makes would wait for
			// the next, a drop a second for good.
			if (event.status === 'online') {
				retryMs = 1000;
				clearTimeout(retry);
				retry = undefined;
				linkUp(() => sessionStorage);
			}
			// No link of this build comes up again: no retry, one reload.
			if (event.status === 'outdated' && !outdated) {
				clearTimeout(retry);
				retry = undefined;
				outdated = reloadOnce(() => sessionStorage) ? 'reloading' : 'reload';
				if (outdated === 'reloading') location.reload();
			}
			if (event.status === 'offline' && linked && !retry) {
				retry = setTimeout(() => connect(true), retryMs);
				retryMs = Math.min(retryMs * 2, 30_000);
			}
		}
		if (event.type === 'mode_changed') engineMode = event.mode;
		if (event.type === 'avatar_changed') engineAvatar = event.path ?? undefined;
		if (event.type === 'effects_changed') engineEffects = JSON.stringify(event.effects);
		onevent?.(event);
	}

	onMount(() => {
		let destroyed = false;
		let created: Engine | undefined;

		(async () => {
			const module = await loadEngine();
			if (destroyed) return;
			if (!module || !canvas) {
				phase = 'missing';
				return;
			}
			try {
				created = await module.create(canvas, receive);
			} catch {
				phase = 'gpu' in navigator ? 'failed' : 'unsupported';
				return;
			}
			if (destroyed) {
				created.free();
				return;
			}
			engine = created;
			phase = 'ready';
		})();

		return () => {
			destroyed = true;
			linked = false;
			clearTimeout(retry);
			engine = undefined;
			created?.free();
		};
	});

	// A world shaped by a field cannot be generated without it, so the bytes
	// go over first and the recipe follows. The engine keeps the field, so a
	// new seed over the same ground downloads nothing.
	$effect(() => {
		if (!engine || !recipe) return;
		const next = JSON.stringify(recipe);
		if (next === engineRecipe) return;
		const wanted = fieldId(recipe);
		const here = engine;
		unshaped = false;
		damaged = false;
		refused = false;
		if (!wanted || wanted === engineField) {
			engineRecipe = next;
			standing = false;
			here.command({ type: 'set_recipe', recipe });
			return;
		}
		if (!fieldPath) {
			// Ground this instance does not serve: said, not another planet shown.
			unshaped = true;
			return;
		}
		engineRecipe = next;
		standing = false;
		fetching = true;
		fetchField(fieldPath, wanted)
			.then((bytes) => {
				if (here !== engine) return;
				here.set_field(bytes);
				engineField = wanted;
				fetching = false;
				here.command({ type: 'set_recipe', recipe });
			})
			.catch((error) => {
				engineRecipe = '';
				fetching = false;
				// Other ground is never drawn as this world's, so it is said.
				if (error instanceof DamagedField) damaged = true;
				else phase = 'failed';
			});
	});

	// Each place the address bar is given is one request; `arrive` is read
	// outside the effect's tracking so a later world does not replay it.
	$effect(() => {
		const place = stand?.place;
		if (!place) return;
		pending = place;
		untrack(arrive);
	});

	$effect(() => {
		if (!engine || (avatar && avatar === engineAvatar)) return;
		if (avatar) engineAvatar = avatar;
		engine.command(avatar ? { type: 'set_avatar', path: avatar } : { type: 'random_avatar' });
	});

	$effect(() => {
		if (!engine || name === engineName) return;
		engineName = name;
		engine.command({ type: 'set_name', name });
	});

	$effect(() => {
		if (!engine || !effects) return;
		const next = JSON.stringify(effects);
		if (next === engineEffects) return;
		engineEffects = next;
		engine.command({ type: 'set_effects', effects });
	});

	// The mix is the shell's, read whole each time: a slider is heard while
	// it is dragged.
	$effect(() => {
		if (engine && loudness) engine.set_loudness(loudness);
	});

	// Where a game played goes, handed over with each engine as it changes.
	$effect(() => {
		engine?.set_clear(clear);
	});

	// The markers hidden, handed over with each engine: it keeps none.
	$effect(() => {
		if (engine && hiddenMarkers) engine.command({ type: 'hide_markers', kinds: hiddenMarkers });
	});

	$effect(() => {
		if (!engine || mode === engineMode) return;
		engineMode = mode;
		engine.command({ type: 'set_mode', mode });
	});

	// What this browser kept of a visitor's home and favourites, handed to
	// the engine before any world answers. An account's are the world's,
	// and the engine leaves these be.
	$effect(() => {
		const choices = parseChoices(kept('choices'));
		if (engine && choices) engine.command({ type: 'set_choices', ...choices });
	});

	/** Whether the pet last said was a visitor's, this browser's to keep. */
	let visiting = false;

	// And the pet it kept of a visitor's, which the world holds while the
	// session lasts.
	$effect(() => {
		const pet = parsePet(kept('pet'));
		if (engine && pet) engine.command({ type: 'set_pet', pet });
	});
</script>

{#snippet alerts()}
	{#if status === 'outdated'}
		<Alert title={t('engine.outdated.title')}>{t(outdated === 'reloading' ? 'engine.outdated.reloading' : 'engine.outdated.reload')}</Alert>
	{:else if status === 'unshaped'}
		<Alert variant="danger" title={t('engine.unshaped.title')}>{t('engine.unshaped.body')}</Alert>
	{:else if status === 'damaged'}
		<Alert variant="danger" title={t('engine.damaged.title')}>{t('engine.damaged.body')}</Alert>
	{:else if status === 'refused'}
		<Alert variant="danger" title={t('engine.refused.title')}>{t('engine.refused.body')}</Alert>
	{:else if status === 'missing'}
		<Alert title={t('engine.missing.title')}>{t('engine.missing.body')} <code>bun run wasm</code></Alert>
	{:else if status === 'unsupported'}
		<Alert variant="danger" title={t('engine.unsupported.title')}>{t('engine.unsupported.body')}</Alert>
	{:else if status === 'failed'}
		<Alert variant="danger" title={t('engine.failed.title')}>{t('engine.failed.body')}</Alert>
	{/if}
{/snippet}

<div class="view">
	<canvas bind:this={canvas} tabindex="0" aria-label={t('engine.canvas')} onclick={() => canvas?.focus()}></canvas>
	<Veil lifted={status === 'running'} busy word={wait} children={wait ? undefined : alerts} />
</div>

<style>
	.view {
		position: absolute;
		inset: 0;
		background: var(--bg-inset);
	}
	canvas {
		display: block;
		width: 100%;
		height: 100%;
		outline: none;
		/* A finger on the world is the engine's: the browser neither scrolls
		   nor zooms the page under it, nor takes it back to do so. */
		touch-action: none;
	}
</style>
