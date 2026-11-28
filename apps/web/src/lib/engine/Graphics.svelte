<script lang="ts">
	import { onMount } from 'svelte';
	import { Alert, Button, Checkbox, Dialog, Field, Fold, Pairs, Segmented } from '$lib/ds';
	import { t, translate } from '$lib/i18n';
	import { plain, rows, type Facts } from './facts';
	import type { Held, Presets } from './index';
	import { Live } from './live.svelte';
	import type { Effects } from './protocol';
	import { lifeKnobs, livesOf, presetNames, type Picture } from './settings';
	import { newGpu, pictureOf, powers, roads, type Draft, type Gpu, type Why } from './start';

	// How this machine draws the world, asked before the engine starts and
	// from Settings once it runs: a preset, or Custom where what starts is
	// set off them all, what lives aside, so what it shows chosen is what
	// starts; the trees, shrubs, flowers and animals while the world has
	// life, flowers only where the picture draws the grass they bloom in;
	// and, folded, the adapter and the road. Before the start, one way
	// forward, a choice always made: Esc and a click outside are its button.
	// From Settings, a Cancel, which Esc and a click outside are too. Under
	// it, what the machine says of itself, small, with a copy for whoever
	// helps when the world does not run: the same list the readout shows.
	interface Props {
		/** Why it opened before the engine; null when Settings opened it. */
		why: Why | null;
		presets: Presets;
		/** What starts, or runs: off the presets, it is offered as Custom. */
		now: Effects;
		/** What it opens on. */
		draft: Draft;
		/** Whether the world has life: what lives is asked only while it has, since with none it switches nothing. */
		life: boolean;
		/** The GPU kept: the engine takes another only as it starts. */
		gpu: Gpu;
		/** The engine runs: a new adapter or road reloads the page. */
		running: boolean;
		facts: Facts;
		/** What the engine holds now, while it runs. */
		held?: () => Held | undefined;
		/** The list of what the machine says is opened: the rest of it is asked. */
		ondescribe: () => void;
		onanswer: (draft: Draft) => void;
		/** Left from Settings without an answer: nothing changes. */
		oncancel: () => void;
	}

	let { why, presets, now, draft, life, gpu, running, facts, held, ondescribe, onanswer, oncancel }: Props = $props();

	// What it opens on, as a copy of its own: the page's draft holds its state.
	// svelte-ignore state_referenced_locally
	let chosen = $state<Draft>($state.snapshot(draft));
	let open = $state(true);
	let answered = false;
	$effect(() => {
		if (!open && !answered) oncancel();
	});
	let listed = $state(false);
	$effect(() => {
		if (listed) ondescribe();
	});

	const live = new Live();
	onMount(() => (running && held ? live.watch(held) : undefined));

	const reloads = $derived(running && newGpu(gpu, chosen.gpu));
	const offered = $derived<Picture[]>(pictureOf(now, presets) === 'custom' ? [...presetNames, 'custom'] : [...presetNames]);
	const pictures = $derived(offered.map((name) => ({ value: name, label: t(`engine.settings.${name}`) })));
	/** What a picture draws: a preset's, or Custom's, what starts. */
	const drawn = (picture: Picture): Effects => (picture === 'custom' ? now : presets[picture]);
	const powerOptions = $derived(powers.map((value) => ({ value, label: t(`engine.graphics.power.${value}`) })));
	const roadOptions = $derived(roads.map((value) => ({ value, label: t(`engine.graphics.road.${value}`) })));

	/**
	 * What the machine says it is: the adapter's name where the browser
	 * gives it, and the road the engine took, or will take: WebGPU where the
	 * browser offers an adapter or has not said, unless WebGL2 is kept.
	 */
	const says = $derived.by(() => {
		const { graphics, start } = facts;
		if (!graphics && !start?.adapter) return '…';
		const webgpu = graphics?.webgpu;
		const adapter = typeof webgpu === 'object' ? webgpu : null;
		const given = [start?.adapter?.name, ...(start?.about ?? [])].filter(Boolean).join(' · ');
		const name = given || graphics?.webgl || [adapter?.vendor, adapter?.architecture].filter(Boolean).join(' · ') || t('engine.machine.hidden');
		const offered = adapter !== null || webgpu === 'late';
		const road = start?.adapter?.backend ?? (gpu.backend !== 'webgl2' && offered ? 'webgpu' : 'webgl2');
		return `${name} · ${t(`engine.graphics.road.${road}`)}`;
	});
	const inUse = $derived(facts.start?.adapter?.backend);

	const list = $derived(rows({ ...facts, live: live.now }, t));
	let copied = $state(false);
	async function copy() {
		const english = rows({ ...facts, live: live.now }, (key, params) => translate('en', key, params));
		try {
			await navigator.clipboard.writeText(plain(english));
			copied = true;
			setTimeout(() => (copied = false), 2000);
		} catch {
			// the clipboard refused: the list stays on screen to read or photograph
		}
	}

	const label = $derived(t(reloads ? 'engine.graphics.reload' : running ? 'engine.graphics.done' : 'engine.graphics.start'));
</script>

<Dialog
	bind:open
	title={t('engine.graphics.title')}
	confirmLabel={label}
	oneWay={!running}
	onconfirm={() => {
		answered = true;
		onanswer(chosen);
	}}
>
	{#if why === 'failed'}
		<Alert variant="danger" title={t('engine.graphics.why.failed')}>{facts.start?.failure ?? ''}</Alert>
	{:else if why}
		<p class="why">{t(`engine.graphics.why.${why}`)}</p>
	{/if}
	<Field label={t('engine.graphics.says')}>
		<p class="says">{says}</p>
	</Field>
	<Field label={t('engine.graphics.picture')}>
		<Segmented
			options={pictures}
			value={chosen.picture}
			label={t('engine.graphics.picture')}
			onselect={(picture) => (chosen = { ...chosen, picture, ...livesOf(drawn(picture)) })}
		/>
	</Field>
	{#if life}
		<div class="checks">
			{#each lifeKnobs as knob (knob)}
				<Checkbox checked={chosen[knob]} disabled={knob === 'flowers' && !drawn(chosen.picture).grass} onchange={(on) => (chosen[knob] = on)}>
					{t(`engine.settings.${knob}`)}
				</Checkbox>
			{/each}
		</div>
	{/if}
	<Fold title={t('engine.graphics.advanced')} open={why === 'failed' || newGpu({}, gpu)}>
		<Field label={t('engine.graphics.power')}>
			<Segmented
				options={powerOptions}
				value={chosen.gpu.power ?? 'high'}
				label={t('engine.graphics.power')}
				onselect={(power) => (chosen.gpu = { ...chosen.gpu, power })}
			/>
		</Field>
		<Field label={t('engine.graphics.road')} hint={inUse && t('engine.graphics.inUse', { value: t(`engine.graphics.road.${inUse}`) })}>
			<Segmented
				options={roadOptions}
				value={chosen.gpu.backend ?? 'webgpu'}
				label={t('engine.graphics.road')}
				onselect={(backend) => (chosen.gpu = { ...chosen.gpu, backend })}
			/>
		</Field>
		{#if reloads}
			<p>{t('engine.graphics.reloads')}</p>
		{/if}
	</Fold>
	<Fold title={t('engine.machine.title')} bind:open={listed}>
		<div class="copy">
			<Button type="button" variant="ghost" onclick={copy}>{t(copied ? 'engine.machine.copied' : 'engine.machine.copy')}</Button>
		</div>
		<Pairs pairs={list} />
	</Fold>
</Dialog>

<style>
	.why {
		color: var(--text);
	}
	.says {
		color: var(--text);
		overflow-wrap: anywhere;
	}
	.checks {
		display: flex;
		gap: var(--sp-5);
	}
	.copy {
		display: flex;
		justify-content: flex-end;
	}
</style>
