<script lang="ts">
	import { Checkbox, Knob, Knobs, LangSwitch, Panel, Segmented, Select, Slider, Tabs, panelId } from '$lib/ds';
	import { t } from '$lib/i18n';
	import { toneMaps, type Effects, type Loudness, type Presets, type ToneMap } from './index';
	import { loudnessDefaults, markerKinds, matching, presetNames, type Game, type Picture, type PresetName } from './settings';

	// Settings: one large panel in the middle of the screen, which the gear in the
	// top right corner and the O key open, its sections in a list down its
	// side. Graphics is how the picture is made: a preset, or Custom the
	// moment a knob leaves it, every knob in sight with the way back to its
	// value in the preset last picked. Sound is how loud each group of what
	// is heard is, under everything and a mute. Game is what floats over the
	// world: the place line and the markers, by kind.
	// Plugins, for who may switch them, and the language. A front end only:
	// it shows what the engine said is in force and asks for changes, which
	// the engine clamps and answers; the stage keeps the choices.
	interface Switches {
		list: readonly { name: string; label: () => string }[];
		on: (name: string) => boolean;
		/** Offline, or a switch on its way: none can be turned. */
		busy: boolean;
		onturn: (name: string, on: boolean) => void;
	}
	interface Props {
		/** What the engine last reported. Undefined until it has. */
		effects: Effects | undefined;
		/** The presets. Undefined until the engine is loaded. */
		presets: Presets | undefined;
		/** The preset last picked: each knob's default. */
		base: PresetName;
		/** The knobs as last set off the presets, which Custom returns to; null while none was. */
		custom: Effects | null;
		onpicture: (picture: Picture) => void;
		onchange: (effects: Effects) => void;
		/** Frames a second, as the engine last counted them. */
		fps: number | undefined;
		loudness: Loudness;
		/** Heard at once, while a slider is dragged. */
		onloudness: (loudness: Loudness) => void;
		game: Game;
		ongame: (game: Game) => void;
		/** The plugins' switches, for one who may switch them; absent, the section is. */
		plugins?: Switches;
		onclose: () => void;
	}

	let { effects, presets, base, custom, onpicture, onchange, fps, loudness, onloudness, game, ongame, plugins, onclose }: Props = $props();

	type Section = 'graphics' | 'sound' | 'game' | 'plugins' | 'language';
	let section = $state<Section>('graphics');
	const sections = $derived<{ value: Section; label: string }[]>([
		{ value: 'graphics', label: t('engine.settings.graphics') },
		{ value: 'sound', label: t('engine.settings.sound') },
		{ value: 'game', label: t('engine.settings.game') },
		...(plugins ? [{ value: 'plugins' as const, label: t('engine.plugins') }] : []),
		{ value: 'language', label: t('common.language') }
	]);

	const picture = $derived<Picture>(effects && presets ? (matching(effects, presets) ?? 'custom') : base);
	const pictures = $derived<{ value: Picture; label: string }[]>([
		...presetNames.filter((name) => presets?.[name]).map((name) => ({ value: name, label: t(`engine.settings.${name}`) })),
		{ value: 'custom', label: t('engine.settings.custom') }
	]);
	/** Each knob's default: its value in the preset last picked. */
	const defaults = $derived(presets?.[base]);

	function pick(next: Picture) {
		if (next === picture || (next === 'custom' && !custom)) return;
		onpicture(next);
	}

	function set<K extends keyof Effects>(key: K, value: Effects[K]) {
		if (effects) onchange({ ...effects, [key]: value });
	}

	// The knobs, in three groups: the picture, the sky and the water, and how
	// far each kind of thing is drawn. `off` greys one whose effect is off.
	type Range = { kind: 'range'; key: NumberKey; min: number; max: number; step?: number; format?: (n: number) => string };
	type Knobbed = { label: string; off?: (e: Effects) => boolean } & (
		| Range
		| { kind: 'check'; key: BooleanKey }
		| { kind: 'select'; key: 'tone_map' }
	);
	type NumberKey = { [K in keyof Effects]: Effects[K] extends number ? K : never }[keyof Effects];
	type BooleanKey = { [K in keyof Effects]: Effects[K] extends boolean ? K : never }[keyof Effects];

	const percent = (n: number) => t('engine.settings.percent', { n: (n * 100).toFixed(0) });
	const metres = (n: number) => t('engine.stats.metres', { n: n.toFixed(0) });
	const decimals = (n: number) => n.toFixed(2);
	const noClouds = (e: Effects) => !e.clouds;

	const groups = $derived<{ name: string; title: string; knobs: Knobbed[] }[]>([
		{
			name: 'picture',
			title: t('engine.settings.picture'),
			knobs: [
				{ kind: 'range', key: 'resolution', label: t('engine.settings.resolution'), min: 0.25, max: 1, step: 0.25, format: percent },
				{ kind: 'check', key: 'shadows', label: t('engine.settings.shadows') },
				{ kind: 'check', key: 'shadows_fade', label: t('engine.settings.shadowsFade'), off: (e) => !e.shadows },
				{ kind: 'check', key: 'grass', label: t('engine.settings.grass') },
				{ kind: 'range', key: 'exposure', label: t('engine.settings.exposure'), min: 0.1, max: 4 },
				{ kind: 'range', key: 'bloom', label: t('engine.settings.bloom'), min: 0, max: 3 },
				{
					kind: 'range',
					key: 'bloom_threshold',
					label: t('engine.settings.bloomThreshold'),
					min: 0.2,
					max: 8,
					step: 0.05,
					off: (e) => e.bloom <= 0
				},
				{ kind: 'select', key: 'tone_map', label: t('engine.settings.toneMap') }
			]
		},
		{
			name: 'sky',
			title: t('engine.settings.sky'),
			knobs: [
				{ kind: 'check', key: 'clouds', label: t('engine.settings.clouds') },
				{ kind: 'range', key: 'cloud_cover', label: t('engine.settings.cloudCover'), min: 0, max: 1, off: noClouds },
				{ kind: 'range', key: 'cloud_density', label: t('engine.settings.cloudDensity'), min: 0.1, max: 3, off: noClouds },
				{
					kind: 'range',
					key: 'wind_m_s',
					label: t('engine.settings.wind'),
					min: 0,
					max: 80,
					step: 1,
					format: (n) => t('engine.stats.metresPerSecond', { n: n.toFixed(0) }),
					off: noClouds
				},
				{ kind: 'range', key: 'cloud_change', label: t('engine.settings.cloudChange'), min: 0, max: 6, step: 0.1, off: noClouds },
				{ kind: 'range', key: 'haze', label: t('engine.settings.haze'), min: 0, max: 6, step: 0.05 },
				{ kind: 'range', key: 'water_clarity', label: t('engine.settings.waterClarity'), min: 0.25, max: 12, step: 0.25 }
			]
		},
		{
			name: 'distances',
			title: t('engine.settings.distances'),
			knobs: [
				{ kind: 'range', key: 'video_play_m', label: t('engine.settings.videoPlay'), min: 16, max: 1024, step: 16, format: metres },
				{ kind: 'range', key: 'picture_reach_m', label: t('engine.settings.pictureReach'), min: 32, max: 2048, step: 32, format: metres },
				{
					kind: 'range',
					key: 'model_reach_m',
					label: t('engine.settings.modelReach'),
					min: 16,
					max: 1024,
					step: 16,
					format: (n) => t('engine.settings.metresABlock', { n: n.toFixed(0) })
				},
				{ kind: 'range', key: 'foliage_near_m', label: t('engine.settings.plantsNear'), min: 0, max: 600, step: 10, format: metres },
				{ kind: 'range', key: 'foliage_mid_m', label: t('engine.settings.plantsMid'), min: 0, max: 600, step: 10, format: metres },
				{ kind: 'range', key: 'foliage_reach_m', label: t('engine.settings.plantsReach'), min: 128, max: 600, step: 8, format: metres },
				{ kind: 'range', key: 'undergrowth', label: t('engine.settings.undergrowth'), min: 0, max: 1, step: 0.05, format: percent }
			]
		}
	]);

	const toneMapOptions = $derived(toneMaps.map((value) => ({ value, label: t(`engine.settings.toneMap.${value}`) })));

	/** A knob's value as its control shows it. */
	function shown(knob: Knobbed, effects: Effects): string {
		if (knob.kind === 'check') return t(effects[knob.key] ? 'engine.settings.on' : 'engine.settings.off');
		if (knob.kind === 'select') return t(`engine.settings.toneMap.${effects.tone_map}`);
		return (knob.format ?? decimals)(effects[knob.key]);
	}

	function reset(knob: Knobbed) {
		if (defaults) set(knob.key, defaults[knob.key]);
	}

	// The levels of what is heard, each under everything: what is played in
	// the world, then what is made by code, by the groups a person names.
	type Level = { [K in keyof Loudness]: Loudness[K] extends number ? K : never }[keyof Loudness];
	const levels = $derived<{ key: Level; label: string }[]>([
		{ key: 'master', label: t('engine.settings.sound.master') },
		{ key: 'world', label: t('engine.settings.sound.world') },
		{ key: 'steps', label: t('engine.settings.sound.steps') },
		{ key: 'calls', label: t('engine.settings.sound.calls') },
		{ key: 'games', label: t('engine.settings.sound.games') }
	]);

	function hear<K extends keyof Loudness>(key: K, value: Loudness[K]) {
		onloudness({ ...loudness, [key]: value });
	}

	const id = 'settings';
</script>

<Panel title={t('engine.settings')} large>
	{#snippet aside()}
		<button class="close" type="button" onclick={onclose}>{t('common.close')}</button>
	{/snippet}
	<div class="body">
		<Tabs options={sections} value={section} label={t('engine.settings.sections')} {id} onselect={(value) => (section = value)} />
		<div class="section" role="tabpanel" id={panelId(id, section)} aria-labelledby="{id}-tab-{section}">
			{#if section === 'graphics'}
				<div class="head">
					{#if presets}
						<Segmented options={pictures} value={picture} label={t('engine.settings.preset')} onselect={pick} />
					{/if}
					{#if fps !== undefined}
						<span class="fps">{t('engine.settings.fps', { n: fps.toFixed(0) })}</span>
					{/if}
				</div>
				{#if effects}
					<div class="groups">
						{#each groups as group (group.name)}
							<section class="group">
								<h3>{group.title}</h3>
								<Knobs>
									{#each group.knobs as knob (knob.key)}
										<Knob
											changed={!!defaults && effects[knob.key] !== defaults[knob.key]}
											fallback={defaults ? shown(knob, defaults) : ''}
											resetLabel={t('engine.settings.reset', { value: defaults ? shown(knob, defaults) : '' })}
											onreset={() => reset(knob)}
										>
											{#if knob.kind === 'range'}
												<Slider
													label={knob.label}
													value={effects[knob.key]}
													min={knob.min}
													max={knob.max}
													step={knob.step}
													format={knob.format}
													disabled={knob.off?.(effects)}
													oninput={(value) => set(knob.key, value)}
												/>
											{:else if knob.kind === 'check'}
												<Checkbox checked={effects[knob.key]} onchange={(value) => set(knob.key, value)}>{knob.label}</Checkbox>
											{:else}
												<Select
													label={knob.label}
													options={toneMapOptions}
													value={effects.tone_map}
													onselect={(value: ToneMap) => set('tone_map', value)}
												/>
											{/if}
										</Knob>
									{/each}
								</Knobs>
							</section>
						{/each}
					</div>
				{/if}
			{:else if section === 'sound'}
				<div class="levels">
					<Knobs>
						{#each levels as level (level.key)}
							<Knob
								changed={loudness[level.key] !== loudnessDefaults[level.key]}
								fallback={percent(loudnessDefaults[level.key])}
								resetLabel={t('engine.settings.reset', { value: percent(loudnessDefaults[level.key]) })}
								onreset={() => hear(level.key, loudnessDefaults[level.key])}
							>
								<Slider
									label={level.label}
									value={loudness[level.key]}
									min={0}
									max={1}
									step={0.05}
									format={percent}
									disabled={loudness.muted}
									oninput={(value) => hear(level.key, value)}
								/>
							</Knob>
							{#if level.key === 'master'}
								<Knob
									changed={loudness.muted !== loudnessDefaults.muted}
									fallback={t('engine.settings.off')}
									resetLabel={t('engine.settings.reset', { value: t('engine.settings.off') })}
									onreset={() => hear('muted', loudnessDefaults.muted)}
								>
									<Checkbox checked={loudness.muted} onchange={(muted) => hear('muted', muted)}>{t('engine.settings.sound.mute')}</Checkbox>
								</Knob>
							{/if}
						{/each}
					</Knobs>
				</div>
			{:else if section === 'game'}
				<div class="rows">
					<Checkbox checked={game.place} onchange={(place) => ongame({ ...game, place })}>{t('engine.settings.placeLine')}</Checkbox>
				</div>
				<section class="group">
					<h3>{t('engine.settings.markers')}</h3>
					<div class="rows">
						{#each markerKinds as kind (kind)}
							<Checkbox checked={game[kind]} onchange={(on) => ongame({ ...game, [kind]: on })}>{t(`engine.settings.markers.${kind}`)}</Checkbox>
						{/each}
					</div>
				</section>
			{:else if section === 'plugins' && plugins}
				<div class="rows">
					{#each plugins.list as plugin (plugin.name)}
						<Checkbox checked={plugins.on(plugin.name)} disabled={plugins.busy} onchange={(on) => plugins.onturn(plugin.name, on)}>
							{plugin.label()}
						</Checkbox>
					{/each}
				</div>
			{:else if section === 'language'}
				<LangSwitch layout="list" />
			{/if}
		</div>
	</div>
</Panel>

<style>
	.close {
		border: none;
		background: none;
		color: var(--text-muted);
		letter-spacing: var(--ls-caps);
		text-transform: uppercase;
		cursor: pointer;
	}
	.close:hover {
		color: var(--text);
	}
	/* The list of sections at the left, the section shown at its right,
	   scrolling alone: the list and the title stay. */
	.body {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		gap: var(--sp-6);
		flex: 1;
		min-height: 0;
	}
	.section {
		display: flex;
		flex-direction: column;
		gap: var(--sp-5);
		min-height: 0;
		overflow-y: auto;
		/* The panel's scroller, to its right and bottom edges: the bar hugs
		   the edge and the content keeps the panel's inset. The bar's room
		   is kept whether the section scrolls or not: the columns and the
		   frame count never move. */
		padding: 0 calc(var(--panel-pad) - var(--scroll-w)) var(--sp-5) 0;
		scrollbar-gutter: stable;
	}
	.head {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--sp-3);
	}
	.fps {
		color: var(--text-muted);
		font-variant-numeric: tabular-nums;
	}
	/* Columns as wide as a knob reads well, as many as fit: each group stays
	   whole in one. */
	.groups {
		columns: 2 calc(var(--panel-w) + var(--sp-8));
		column-gap: var(--sp-7);
	}
	.group {
		display: flex;
		flex-direction: column;
		gap: var(--sp-3);
		margin-bottom: var(--sp-6);
		break-inside: avoid;
	}
	h3 {
		color: var(--text-muted);
		letter-spacing: var(--ls-caps);
		text-transform: uppercase;
	}
	/* One column of levels, as wide as a group of the picture's knobs. */
	.levels {
		max-width: calc(var(--panel-w) + var(--sp-8));
	}
	.rows {
		display: flex;
		flex-direction: column;
		gap: var(--sp-1);
	}
	@media (max-width: 640px) {
		.body {
			grid-template-columns: minmax(0, 1fr);
			grid-template-rows: auto minmax(0, 1fr);
			gap: var(--sp-4);
		}
	}
</style>
