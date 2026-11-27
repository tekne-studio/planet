<script lang="ts">
	import type { Snippet } from 'svelte';
	import { Badge, Button, Icon, IconButton, Input, Panel, Spinner, ThemeToggle } from '$lib/ds';
	import { assetUrl, isAsset, kindOf, upload } from '$lib/files';
	import { t } from '$lib/i18n';
	import { Refused, builds, type Link } from '$lib/instance';
	import { keep, kept } from '$lib/kept';
	import { plugins, type Seam } from '$lib/plugins';
	import { touch, visible } from '$lib/touch.svelte';
	import type { Recipe } from '$lib/world';
	import { onMount } from 'svelte';
	import { replaceState } from '$app/navigation';
	import EngineView from './EngineView.svelte';
	import Help from './Help.svelte';
	import Labels from './Labels.svelte';
	import Waiting from './Waiting.svelte';
	import Markers from './Markers.svelte';
	import Nametags from './Nametags.svelte';
	import PromptLine from './Prompt.svelte';
	import Controls from './Controls.svelte';
	import Place from './Place.svelte';
	import Settings from './Settings.svelte';
	import Touch from './Touch.svelte';
	import Pad from './Pad.svelte';
	import StickDrawn from './Stick.svelte';
	import { CONTROLS } from './controls';
	import Buttons from './Buttons.svelte';
	import { layerProps, type LayerProps, type Reads } from '$lib/plugins/plugin';
	import {
		checkCartridge,
		readCover,
		isPluginEvent,
		loadEngine,
		NAME_CHARS,
		type Anchor,
		type CartridgeCover,
		type Clear,
		type Label,
		type Waiting as WaitingGame,
		type Marker,
		type MarkerAt,
		type Effects,
		type Loudness,
		type EngineEvent,
		type Level,
		type Mode,
		type PeerInfo,
		type Pet,
		type PluginEvent,
		type PluginOn,
		type Presets,
		type Prompt,
		type PromptAt,
		type SessionStatus,
		type Stick
	} from './index';
	import { hiddenMarkers, loudnessDefaults, matching, presetName, readGame, readLoudness, type Game, type Picture, type PresetName } from './settings';
	import { who } from './who';

	// The full viewport engine and what floats over it: what a world and the
	// founding screen share. In the top right corner a bar of icons, each a
	// panel: who is here and the settings, which hold the picture, the
	// sound, the game, the plugins and the language; then the theme. Under it, the line that
	// says where the body is. One thing is open at a time: a panel, or the
	// hand of one plugin, which the engine keeps; a panel opening puts the
	// hand down, and a plugin taking the pointer closes the panel. The help
	// is one of the panels: its button is in the bottom left corner, and in
	// the bar on a touch screen. The page may bring a panel of its own, in
	// the top left corner, where a plugin's menu otherwise sits. A touch
	// screen walks, looks, jumps and flies: the stick and the buttons of
	// `Touch`, the help in the bar, and no building offered to any plugin,
	// whatever the level; and the stage keeps to what the phone's keyboard
	// leaves in view (`visible`). While the controls are lent, a holder's
	// `Pad` takes the thumbs' place, the bar and the line wait, and the
	// part the pad leaves clear goes to the engine, for the game played.

	interface Props {
		title: string;
		recipe: Recipe | null;
		/** Where to fetch the field the recipe names, when it names one. */
		fieldPath?: string;
		/** Asset path of the visitor's avatar, from the page load. */
		avatar: string | null;
		/**
		 * The avatar is the account's, as mundos signed it: the engine holds it
		 * from the world's welcome, `V` passes, and this browser keeps nothing.
		 */
		ownAvatar?: boolean;
		/** What this person is called, from the page load. Empty when nobody said. */
		name?: string;
		/** The world's socket, when this is a world other people can be in. */
		link?: Link;
		/** What the world said this person may do. Null in the offline preview, with no world to ask. */
		level?: Level | null;
		/** The page's key, which a file a plugin sends to the world rides with. Empty for a visitor. */
		key?: string;
		/** The name is an account's, as mundos signed it: shown, never edited here. */
		account?: boolean;
		/** Goes to mundos's door to sign in. Present for a visitor of a hosted world. */
		onsignin?: () => void;
		/** Turns a plugin on or off for this world. Present for its admin. */
		onswitch?: (name: string, on: boolean) => Promise<void>;
		/** Called once the engine reports which generator version it runs. */
		onready?: (generatorVersion: number) => void;
		/** The page's own top of the panel, when it has one. */
		children?: Snippet;
	}

	let { title, recipe, fieldPath, avatar, ownAvatar = false, name = '', link, level = null, key = '', account = false, onsignin, onswitch, onready, children }: Props =
		$props();

	// A visitor's name, edited where it is shown. The engine hears it at once,
	// so the world does too, and this browser keeps it for the next visit.
	// The page's name is the starting point only: from here on it is edited.
	// svelte-ignore state_referenced_locally
	let myName = $state(name);
	let editing = $state(false);
	let draft = $state('');
	let nameInput: HTMLInputElement | undefined = $state();

	function editName() {
		draft = myName;
		editing = true;
		requestAnimationFrame(() => nameInput?.select());
	}

	function keepName() {
		if (!editing) return;
		editing = false;
		const next = [...draft.split(/\s+/).filter(Boolean).join(' ')].slice(0, NAME_CHARS).join('').trim();
		if (next === myName) return;
		myName = next;
		keep('name', next);
	}

	function onNameKey(event: KeyboardEvent) {
		if (event.key === 'Enter') {
			event.preventDefault();
			keepName();
		} else if (event.key === 'Escape') {
			event.preventDefault();
			editing = false;
		}
	}

	let view = $state<ReturnType<typeof EngineView>>();
	let mode = $state<Mode>('walk');
	/** What the prompt offers in front of the body, as the engine last said. */
	let prompt = $state.raw<Prompt | null>(null);
	/** Where its point is on the screen, and where it hangs over it, while it is a point in view. */
	let promptAt = $state.raw<PromptAt | null>(null);
	/** How far its ring has closed, while a hold is held; and how many taps and holds acted on it, each felt on the card. */
	let ring = $state<number | null>(null);
	let taps = $state(0);
	/** The plugin the controls are lent to: the keys are its, and the bar says them. */
	let holder = $state<string | null>(null);
	let holds = $state(0);
	let stats = $state<Extract<EngineEvent, { type: 'stats' }>>();
	/** How long the world's day is, as the engine last said it. */
	let day = $state<Extract<EngineEvent, { type: 'day' }>>();
	let session = $state<SessionStatus>('offline');
	/** This client's own session in the world, while online. */
	let me = $state<number | null>(null);
	let peers = $state<PeerInfo[]>([]);
	let anchors = $state.raw<Anchor[]>([]);
	let labels = $state.raw<Label[]>([]);
	/** The games held for this player, where each is on the screen. */
	let waiting = $state.raw<WaitingGame[]>([]);
	/** What the plugins point out, and where each shown is for the eye, as the engine last said. */
	let markers = $state.raw<Marker[]>([]);
	let markersAt = $state.raw<MarkerAt[]>([]);
	/** The stick a finger holds, while one does. */
	let stick = $state.raw<Stick | null>(null);
	/** What a touch screen's pad leaves clear of the stage, where a game played goes, while it is in. */
	let clear = $state.raw<Clear | null>(null);
	/** Whether this session builds here: a level that builds, on a screen that is no touch screen. */
	const building = $derived(builds(level) && !touch.current);

	/** The paints a cell can take: the world's palette, which a plugin's layer shows. */
	let palette = $state.raw<string[]>([]);

	// Plugins: the engine says which are on, the ones the world speaks at the
	// version this build does, and each of them gets a layer over the world
	// and the seam under its own name.
	let spoken = $state.raw<PluginOn[]>([]);
	const mounted = $derived(plugins.filter((plugin) => spoken.some((on) => on.name === plugin.name)));
	/** The holder's words, for its controls. */
	const holderWords = $derived(mounted.find((plugin) => plugin.name === holder)?.words);
	/** A touch screen's pad stands in for the world's thumbs while the controls are lent, leaving among its buttons. */
	const pad = $derived(touch.current && holder !== null);
	// What a world's admin switches: the life of a world, a system of the
	// core, then the plugins. The world says which are on, for both alike.
	const switched: readonly { name: string; label: () => string }[] = [{ name: 'gaia', label: () => t('engine.life') }, ...plugins];

	type Hear = (kind: string, event: Record<string, unknown>) => void;
	const hearing = new Map<string, Set<Hear>>();
	const layers = new Map<string, LayerProps>();

	/** Where every plugin's layer and button read the core's readings. */
	const reads: Reads = {
		online: () => session === 'online',
		me: () => me,
		builds: () => building,
		touch: () => touch.current,
		palette: () => palette,
		peers: () => peers,
		anchors: () => anchors,
		mode: () => mode,
		lent: () => holder !== null,
		speed_mps: () => stats?.speed_mps ?? null,
		altitude_m: () => stats?.altitude_m ?? null,
		pace: () => stats?.pace ?? null
	};

	/** What a plugin's layer and button are mounted with: the same object for as long as the page is. */
	function layerOf(plugin: string): LayerProps {
		let props = layers.get(plugin);
		if (!props) {
			props = layerProps(seamOf(plugin), reads);
			layers.set(plugin, props);
		}
		return props;
	}

	/** Whether the buttons over the world show: on every screen, while the controls are the body's own. */
	const buttons = $derived(holder === null);

	/** The seam under a plugin's name. */
	function seamOf(plugin: string): Seam {
		return {
			command: (kind, body = {}) => view?.command({ ...body, type: `${plugin}.${kind}` }),
			core: (command) => view?.command(command),
			listen(hear) {
				const heard = hearing.get(plugin) ?? new Set<Hear>();
				hearing.set(plugin, heard.add(hear));
				return () => heard.delete(hear);
			},
			release: () => view?.release(),
			take: () => view?.take(),
			upload: (file) => upload(file, key),
			weigh,
			cabinetTemplate: async () => (await loadEngine())?.cabinetTemplate() ?? null,
			cover
		};
	}

	// The engine weighs a 3D model, the budget being the one it draws by,
	// and the cartridges' engine holds a cartridge to the contract the world
	// holds it to. With no engine built there is no world to send it to.
	async function weigh(file: Blob) {
		const bytes = new Uint8Array(await file.arrayBuffer());
		const refused =
			kindOf(bytes.subarray(0, 16)) === 'wasm' ? await checkCartridge(bytes) : (await loadEngine())?.weigh(bytes);
		if (refused) throw new Refused(refused);
	}

	// A file named by its content never changes: its cover is read once a
	// page. One that could not be fetched is asked again next time.
	const covers = new Map<string, Promise<CartridgeCover | null>>();
	function cover(cart: string): Promise<CartridgeCover | null> {
		if (!isAsset(cart) || !link) return Promise.resolve(null);
		let read = covers.get(cart);
		if (!read) {
			const name = cart.slice('asset://'.length).replace(/\.wasm$/, '');
			read = fetch(assetUrl(cart, link.files))
				.then((response) => (response.ok ? response.arrayBuffer() : Promise.reject(new Error(`${response.status}`))))
				.then((bytes) => readCover(new Uint8Array(bytes), name))
				.catch(() => {
					covers.delete(cart);
					return null;
				});
			covers.set(cart, read);
		}
		return read;
	}

	function heard(event: PluginEvent) {
		const dot = event.type.indexOf('.');
		const kind = event.type.slice(dot + 1);
		for (const hear of hearing.get(event.type.slice(0, dot)) ?? []) hear(kind, event);
	}

	// Switching a plugin is the admin's, and the world answers with its
	// statement: the box follows what the world says, never the click.
	let switching = $state<string | null>(null);

	async function turn(plugin: string, on: boolean) {
		switching = plugin;
		try {
			await onswitch?.(plugin, on);
		} finally {
			switching = null;
		}
	}

	// So is the day, and the world answers with the day: the one it took, or
	// the one in force and why not. Until then no other length is asked.
	let choosing = $state(false);

	function chooseDay(minutes: number) {
		choosing = true;
		view?.command({ type: 'set_day', minutes });
	}

	// The address bar is where you are. What this page writes there as you
	// move is where you already stand; what a hand puts there is somewhere to
	// go: the link that brought you here, or a place pasted over it. The
	// browser's own `hashchange` says the second and nothing else: the
	// page's `replaceState` never fires it, and nor does a rerun of the
	// loads, which hands `page.url` the hash of the last navigation, the
	// link's, long after the body walked away. A new object each time makes
	// the same place pasted twice two requests. The engine view walks there
	// once it stands in the world this page asked for.
	/** The last pose this page wrote, so standing still writes nothing. */
	const own = { pose: '' };
	let arrival = $state<{ place: string } | null>(null);

	onMount(() => {
		const read = () => {
			const place = location.hash.slice(1);
			arrival = place && place !== own.pose ? { place } : null;
		};
		read();
		addEventListener('hashchange', read);
		return () => removeEventListener('hashchange', read);
	});

	// Replaced and never pushed: the back button is the way out of the world,
	// not a trail of every step taken in it.
	$effect(() => {
		if (!stats || stats.pose === own.pose) return;
		own.pose = stats.pose;
		replaceState(`#${stats.pose}`, {});
	});

	// How the picture is made belongs to the machine, not to the account: a
	// phone and a desktop want different answers. It stays in this browser:
	// the effects asked, the preset last picked, which gives each knob its
	// default, and the knobs as last set off the presets, which Custom
	// returns to. Until a choice is kept, a touch screen is asked the light
	// picture each visit, and any other screen what the engine starts with.
	/** What is asked of the engine: the kept choice, then the panel. */
	let wanted = $state<Effects>();
	/** What the engine says is in force. */
	let effects = $state<Effects>();
	/** The light, the medium and the high picture, as the engine defines them. */
	let presets = $state<Presets>();
	let base = $state<PresetName>('high');
	let custom = $state.raw<Effects | null>(null);

	onMount(() => {
		wanted = parsed(kept('effects')) ?? undefined;
		custom = parsed(kept('custom'));
		loadEngine().then((engine) => {
			presets = engine?.presets();
			if (!presets) return;
			if (!wanted && touch.current) wanted = presets.light;
			const preset = wanted && matching(wanted, presets);
			base = presetName(kept('preset')) ?? preset ?? (touch.current ? 'light' : 'high');
			// A choice kept before Custom was kept is what Custom returns to.
			if (wanted && !preset && !custom) custom = wanted;
		});
	});

	function parsed(text: string | null): Effects | null {
		try {
			return JSON.parse(text ?? 'null');
		} catch {
			return null;
		}
	}

	/** A knob moved: the picture is Custom, and Custom is this. */
	function choose(next: Effects) {
		wanted = next;
		keep('effects', JSON.stringify(next));
		if (presets && matching(next, presets)) return;
		custom = next;
		keep('custom', JSON.stringify(next));
	}

	function pickPicture(picture: Picture) {
		if (picture === 'custom') {
			if (custom) choose(custom);
			return;
		}
		if (!presets) return;
		base = picture;
		keep('preset', picture);
		wanted = presets[picture];
		keep('effects', JSON.stringify(wanted));
	}

	// How loud each group of what is heard is, kept in this browser as the
	// picture is. Undefined until read, so the shell hears the kept levels first.
	let loudness = $state<Loudness>();
	onMount(() => (loudness = readLoudness(kept('sound'))));

	function chooseLoudness(next: Loudness) {
		loudness = next;
		keep('sound', JSON.stringify(next));
	}

	// What a person chose of the game, kept in this browser as the picture is.
	let game = $state<Game>(readGame(null));
	let gameRead = $state(false);
	onMount(() => {
		game = readGame(kept('game'));
		gameRead = true;
	});
	// The markers hidden: undefined until the game is read, so the engine
	// hears the kept choice first.
	const hidden = $derived(gameRead ? hiddenMarkers(game) : undefined);

	function chooseGame(next: Game) {
		game = next;
		keep('game', JSON.stringify(next));
	}

	/** The panel open, if any: under the bar, or the help in its corner. */
	type Opened = 'here' | 'settings' | 'help';
	let opened = $state<Opened | null>(null);

	/** Opens one panel, or none. A panel opening has the engine's hand put down what it holds. */
	function show(next: Opened | null) {
		if (opened === next) return;
		opened = next;
		if (next) view?.command({ type: 'put_down' });
	}

	function toggle(panel: Opened) {
		show(opened === panel ? null : panel);
	}

	// Escape closes the panel, as it drops a plugin's hand in the engine. A
	// field that took the key for itself (a name being edited, the chat) says
	// so with preventDefault, and keeps its panel. O opens and closes the
	// settings, unless typed into a field or the keys are lent to a plugin;
	// opening them frees the pointer, for the panel to be used.
	function onKey(event: KeyboardEvent) {
		if (event.defaultPrevented) return;
		if (event.key === 'Escape' && opened) return show(null);
		if (event.code !== 'KeyO' || event.repeat || event.ctrlKey || event.metaKey || event.altKey || holder || typing(event.target)) return;
		toggle('settings');
		if (opened === 'settings') view?.release();
	}

	function typing(target: EventTarget | null): boolean {
		return target instanceof HTMLElement && (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName));
	}

	function receive(event: EngineEvent | PluginEvent) {
		if (event.type === 'pointer_changed' && event.plugin) opened = null;
		if (event.type === 'controls_changed') {
			holder = event.plugin;
			if (holder) opened = null;
		}
		if (isPluginEvent(event)) return heard(event);
		if (event.type === 'ready') onready?.(event.generator_version);
		else if (event.type === 'mode_changed') mode = event.mode;
		else if (event.type === 'prompt') {
			prompt = event.prompt;
			promptAt = null;
		} else if (event.type === 'prompt_at') promptAt = event;
		else if (event.type === 'acted') {
			if (event.plugin === prompt?.plugin && event.id === prompt.id) {
				if (event.act === 'tap') taps += 1;
				else holds += 1;
			}
		} else if (event.type === 'ring') {
			ring = event.closed;
		}
		else if (event.type === 'stats') stats = event;
		else if (event.type === 'day') {
			day = event;
			choosing = false;
		} else if (event.type === 'session') {
			session = event.status;
			// An ask on its way went with the link.
			if (session !== 'online') choosing = false;
			me = event.session;
		} else if (event.type === 'peers') peers = event.peers;
		else if (event.type === 'statement') spoken = event.plugins;
		else if (event.type === 'anchors') anchors = event.anchors;
		else if (event.type === 'labels') labels = event.labels;
		else if (event.type === 'waiting') waiting = event.waiting;
		else if (event.type === 'markers_changed') markers = event.markers;
		else if (event.type === 'markers_at') markersAt = event.at;
		else if (event.type === 'stick') stick = event.stick;
		else if (event.type === 'palette') palette = event.colors;
		else if (event.type === 'effects_changed') effects = event.effects;
		else if (event.type === 'pet') hearPet(event.pet);
		else if (event.type === 'whistled') whistled(event.answered, event.coming_s);
		else if (event.type === 'avatar_changed') {
			fetching = event.path !== null && !event.worn;
			if (event.path && event.path !== avatar && !ownAvatar) remember(event.path);
		}
	}

	/** This body's pet, as the engine last said; and the name being given it, while one is asked. */
	let pet = $state.raw<Pet | null>(null);
	let petName = $state('');
	/** A pet just tamed is asked its name once; set aside, it is asked again on the next. */
	let naming = $state(false);
	/** Said for a few seconds after a whistle: the pet answered from past earshot and is on its way, or it is too far to answer. */
	let coming = $state(false);
	let tooFar = $state(false);
	let comingTimer: ReturnType<typeof setTimeout> | undefined;

	function hearPet(next: Pet | null) {
		if (next?.number !== pet?.number && next?.name === '') ask();
		pet = next;
		if (pet?.name) naming = false;
	}

	/** The name field, while one is asked; it has the keyboard the moment it opens. */
	let nameField = $state<HTMLInputElement>();
	/** The key that tamed is still held as the field opens: its repeats are no name. */
	let held = false;

	function ask() {
		naming = true;
		held = true;
		view?.release();
	}

	$effect(() => {
		nameField?.focus();
	});

	/**
	 * Enter keeps the name typed, and with none typed keeps the pet unnamed, as Escape does. Either way the world has the keys back; after Enter the pointer too, since the browser takes Enter as a gesture and Escape never.
	 */
	function nameKey(event: KeyboardEvent) {
		if (event.repeat && held) return event.preventDefault();
		held = false;
		const unnamed = event.key === 'Enter' && !petName.trim();
		if (event.key !== 'Escape' && !unnamed) return;
		event.preventDefault();
		closeNaming();
	}

	function closeNaming() {
		naming = false;
		petName = '';
		view?.take();
	}

	/** What the whistle's answer says: on its way from past earshot until it comes into sight, or too far, for a few seconds. */
	function whistled(answered: boolean, coming_s: number) {
		clearTimeout(comingTimer);
		tooFar = !answered;
		coming = answered && coming_s > 0;
		if (coming || tooFar)
			comingTimer = setTimeout(
				() => {
					coming = false;
					tooFar = false;
				},
				Math.max(coming_s * 1000, 3000)
			);
	}

	function namePet(event: SubmitEvent) {
		event.preventDefault();
		const name = petName.trim();
		if (!name) return;
		view?.command({ type: 'name_pet', name });
		closeNaming();
	}

	/** An avatar is on its way: the body stays as it was until it lands. */
	let fetching = $state(false);

	// An avatar picked in the world becomes this browser's choice.
	function remember(path: string) {
		keep('avatar', path);
	}
</script>

<svelte:window onkeydown={onKey} />

<div
	class="stage"
	class:touch={touch.current}
	style:top={visible.current && `${visible.current.top}px`}
	style:bottom={visible.current && 'auto'}
	style:height={visible.current && `${visible.current.height}px`}
>
	<EngineView
		bind:this={view}
		{recipe}
		{fieldPath}
		{mode}
		{avatar}
		name={myName}
		{link}
		stand={arrival}
		effects={wanted}
		{loudness}
		clear={pad ? clear : null}
		hiddenMarkers={hidden}
		onevent={receive}
	/>
	{#if link}
		<Nametags {anchors} {peers} {me} />
	{/if}
	<Labels {labels} />
	<Waiting {waiting} />
	<Markers {markers} at={markersAt} plugins={mounted} oncommand={(command) => view?.command(command)} />
	{#each mounted as plugin (plugin.name)}
		{#if plugin.Layer}
			<plugin.Layer {...layerOf(plugin.name)} />
		{/if}
	{/each}
	{#if buttons}
		<Buttons touch={touch.current}>
			{#each mounted as plugin (plugin.name)}
				{#if plugin.Button}
					<plugin.Button {...layerOf(plugin.name)} />
				{/if}
			{/each}
			{#if stats?.calls}
				<IconButton
					icon="wind"
					label={pet?.name ? t('engine.whistle', { name: pet.name }) : t('engine.whistleUnnamed')}
					onclick={() => view?.command({ type: 'sound', voice: 'whistle' })}
				/>
			{/if}
		</Buttons>
	{/if}
	{#if children}
		<Panel {title}>
			{@render children()}
		</Panel>
	{/if}
	{#if holder && !touch.current}
		<Controls words={holderWords} />
	{/if}
	{#if !pad}
		<PromptLine {prompt} at={promptAt} {ring} {taps} pulse={holds} plugins={mounted} touch={touch.current} onkey={(down) => view?.key('KeyE', down)} />
	{/if}
	{#if touch.current}
		{#if stick}
			<StickDrawn {stick} />
		{/if}
		{#if pad}
			<Pad
				controls={CONTROLS}
				sticking={stick !== null}
				words={holderWords}
				onkey={(code, down) => view?.key(code, down)}
				onclear={(part) => (clear = part)}
			/>
		{:else}
			<Touch flying={mode === 'fly' || (mode.startsWith('ride.') && (stats?.flying ?? false))} onkey={(code, down) => view?.key(code, down)} />
		{/if}
	{/if}
	<Help
		plugins={mounted}
		browse={!ownAvatar}
		touch={touch.current}
		open={opened === 'help'}
		onopen={() => show('help')}
		onclose={() => show(null)}
	/>
	{#if naming && pet}
		<Panel title={t('engine.pet.title')} corner="middle">
			{#snippet aside()}
				<IconButton icon="x" label={t('engine.pet.later')} onclick={closeNaming} />
			{/snippet}
			<form class="pet" onsubmit={namePet}>
				<Input
					bind:value={petName}
					bind:element={nameField}
					onkeydown={nameKey}
					maxlength={32}
					placeholder={t('engine.pet.name')}
					aria-label={t('engine.pet.name')}
					autocomplete="off"
					spellcheck="false"
				/>
				<Button type="submit" disabled={!petName.trim()}>{t('engine.pet.give')}</Button>
			</form>
		</Panel>
	{/if}
	{#if coming}
		<div class="notice" role="status">
			{pet?.name ? t('engine.pet.coming', { name: pet.name }) : t('engine.pet.comingUnnamed')}
		</div>
	{:else if tooFar}
		<div class="notice" role="status">
			{pet?.name ? t('engine.pet.tooFar', { name: pet.name }) : t('engine.pet.tooFarUnnamed')}
		</div>
	{/if}
	{#if fetching}
		<div class="fetching">
			<Spinner label={t('engine.avatar.loading')} />
			<span aria-hidden="true">{t('engine.avatar.loading')}</span>
		</div>
	{/if}
	{#if !pad}
		<nav class="bar" aria-label={t('engine.bar')}>
			{#if link}
				<IconButton
					icon="users"
					label={t('engine.here')}
					badge={session === 'online' ? String(peers.length + 1) : undefined}
					aria-expanded={opened === 'here'}
					onclick={() => toggle('here')}
				/>
			{/if}
			<IconButton icon="settings" label={t('engine.settings')} aria-expanded={opened === 'settings'} onclick={() => toggle('settings')} />
			{#if touch.current}
				<IconButton icon="circle-help" label={t('engine.help')} aria-expanded={opened === 'help'} onclick={() => toggle('help')} />
			{/if}
			<ThemeToggle />
		</nav>
	{/if}
	{#if stats && game.place && !pad && opened !== 'here' && !(opened === 'help' && touch.current)}
		<Place place={stats.place} hour_min={stats.hour_min} bearing_deg={stats.bearing_deg} altitude_m={stats.altitude_m} speed_mps={stats.speed_mps} />
	{/if}
	{#if opened === 'here' && link}
		<Panel title={t('engine.here')} corner="top-right" under>
			{#snippet aside()}
				<span class="corner">
					<Badge variant={session === 'online' ? 'solid' : 'outline'}>{t(`engine.session.${session}`)}</Badge>
					<IconButton icon="x" label={t('common.close')} onclick={() => show(null)} />
				</span>
			{/snippet}
			{#if onsignin}<Button variant="ghost" onclick={onsignin}>{t('door.signIn')}</Button>{/if}
			<ul class="here">
				{#if session === 'online'}
					<li class="you">
						{#if editing}
							<Input
								bind:element={nameInput}
								bind:value={draft}
								maxlength={NAME_CHARS * 2}
								placeholder={t('engine.here.name')}
								aria-label={t('engine.here.name')}
								autocomplete="off"
								spellcheck="false"
								onkeydown={onNameKey}
								onblur={keepName}
							/>
						{:else}
							<span>{myName || t('engine.here.you')}</span>
							{#if myName}<span class="muted">{t('engine.here.you')}</span>{/if}
							{#if !account}
								<button type="button" class="edit" title={t('engine.here.rename')} onclick={editName}>
									<Icon name="pencil" label={t('engine.here.rename')} />
								</button>
							{/if}
						{/if}
					</li>
				{/if}
				{#each peers as peer (peer.session)}
					<li>{who(peer)}</li>
				{/each}
			</ul>
		</Panel>
	{:else if opened === 'settings'}
		<Settings
			{effects}
			{presets}
			{base}
			{custom}
			onpicture={pickPicture}
			onchange={choose}
			fps={stats?.fps}
			loudness={loudness ?? loudnessDefaults}
			onloudness={chooseLoudness}
			{game}
			ongame={chooseGame}
			plugins={onswitch && link
				? {
						list: switched,
						on: (name) => spoken.some((on) => on.name === name),
						busy: session !== 'online' || switching !== null,
						onturn: turn
					}
				: undefined}
			day={level === 'admin' && link && day
				? {
						minutes: day.minutes,
						lengths: day.lengths,
						busy: session !== 'online' || choosing,
						refused: day.refused,
						onchoose: chooseDay
					}
				: undefined}
			onclose={() => show(null)}
		/>
	{/if}
</div>

<style>
	/* Nothing over the world is text to select: a finger that drags the
	   world and then taps a button would otherwise paint the whole page
	   selected. What is typed into stays selectable. */
	.stage {
		position: fixed;
		inset: 0;
		overflow: hidden;
		user-select: none;
		-webkit-user-select: none;
		-webkit-touch-callout: none;
	}
	/* The thumbs' home on a touch screen: from the foot of the world up as
	   high as the right thumb's two rows. The stick opens wherever the left
	   thumb lands in it, so what is read or pressed on the left (the card,
	   a plugin's readings) and the buttons over the world stand above it,
	   never under a thumb. */
	.stage.touch {
		--thumb-home: calc(2 * var(--sp-5) + var(--control-h) + 2 * var(--thumb-h) + 2 * var(--sp-4));
	}
	.stage :global(:is(input, textarea)) {
		user-select: text;
		-webkit-user-select: text;
	}
	.bar {
		position: absolute;
		top: var(--sp-4);
		right: var(--sp-4);
		z-index: var(--z-panel);
		display: flex;
		align-items: center;
		gap: var(--sp-2);
	}
	.bar {
		--control-bg: var(--bg-overlay);
	}
	/* The bar leaves room for the first button of the top left corner, a
	   plugin's menu: on the narrowest phone its buttons narrow instead. */
	@media (pointer: coarse) {
		.bar {
			max-width: calc(100% - 3 * var(--sp-4) - var(--control-h));
		}
	}
	.pet {
		display: flex;
		gap: var(--sp-2);
	}
	/* What a whistle that called nothing says, at the top middle, which
	   the bar and a plugin's menu leave free: the pet heard nothing, so
	   the person hears why. */
	.notice {
		position: absolute;
		top: var(--sp-4);
		left: 50%;
		z-index: var(--z-panel);
		max-width: calc(100% - 2 * var(--sp-4));
		padding: var(--sp-2) var(--sp-3);
		transform: translateX(-50%);
		background: var(--bg-overlay);
		color: var(--text);
		pointer-events: none;
	}
	/* The wait for an avatar on its way, at the bottom middle, in the voice
	   of the engine's own waits. It shows after a beat, so a file that lands
	   at once never flashes it. */
	.fetching {
		position: absolute;
		bottom: var(--sp-5);
		left: 50%;
		z-index: var(--z-panel);
		display: flex;
		align-items: center;
		gap: var(--sp-2);
		min-height: var(--control-h);
		padding: 0 var(--sp-3);
		background: var(--bg-overlay);
		color: var(--text-muted);
		letter-spacing: var(--ls-caps);
		text-transform: uppercase;
		pointer-events: none;
		transform: translateX(-50%);
		animation: appear 200ms ease 300ms both;
	}
	@keyframes appear {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}
	.corner {
		display: flex;
		align-items: center;
		gap: var(--sp-2);
	}
	.here {
		display: flex;
		flex-direction: column;
		gap: var(--sp-1);
	}
	.you {
		display: flex;
		align-items: center;
		gap: var(--sp-2);
		min-height: var(--control-h);
	}
	.muted {
		color: var(--text-muted);
	}
	.edit {
		display: inline-grid;
		place-items: center;
		width: var(--control-h);
		height: var(--control-h);
		border: none;
		background: none;
		color: var(--text-muted);
		cursor: pointer;
	}
	.edit:hover {
		color: var(--text);
	}
</style>
