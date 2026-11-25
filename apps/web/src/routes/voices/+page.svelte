<script lang="ts">
	import { Page, PageHeader, Spinner, Topbar } from '$lib/ds';
	import { loadEngine } from '$lib/engine';
	import Voice from './Voice.svelte';

	// The voice board: every sound the engine makes by code, to hear and tune
	// by ear. A developer reference like `/ds`, written in the project
	// language; its prose does not go through i18n.

	/** The rate the board renders at; the browser resamples to its own. */
	const RATE = 48_000;

	const engine = loadEngine();
	let audio: AudioContext | undefined;

	// The browser holds few contexts: the board's goes with the page.
	$effect(() => () => {
		void audio?.close();
	});

	function play(samples: Float32Array) {
		if (samples.length === 0) return;
		audio ??= new AudioContext();
		const buffer = audio.createBuffer(1, samples.length, RATE);
		buffer.getChannelData(0).set(samples);
		const source = audio.createBufferSource();
		source.buffer = buffer;
		source.connect(audio.destination);
		source.start();
	}
</script>

<svelte:head>
	<title>Voices · planet</title>
</svelte:head>

<Topbar />

<Page>
	<PageHeader
		title="Voices"
		lede="Sound made by code: each voice a few layers of numbers, rendered from a seed to the same samples on every machine. A strike in the world turns through the seeds, so no two in a row are one sample."
		trail={[{ label: 'planet', href: '/' }]}
	/>

	{#await engine}
		<Spinner />
	{:then module}
		{#if module}
			{@const board = module.voices()}
			{@const render = (name: string, seed: number) => module.renderVoice(name, seed, RATE)}
			<div class="sections">
				<section>
					<h2>The core's</h2>
					<p class="muted">The whistle, on Q, a hoof on each ground, a jump's spring and landing, the wings, and the neigh, heard nowhere in the world yet.</p>
					{#each board.voices.filter((voice) => !voice.trial) as voice (voice.name)}
						<Voice {voice} variants={board.variants} {render} {play} />
					{/each}
				</section>
				<section>
					<h2>Trials</h2>
					<p class="muted">Recipes for the plugins that will own them. Nothing in the world strikes them.</p>
					{#each board.voices.filter((voice) => voice.trial) as voice (voice.name)}
						<Voice {voice} variants={board.variants} {render} {play} />
					{/each}
				</section>
			</div>
		{:else}
			<p class="muted">The engine is not built here: <code>bun run wasm</code>.</p>
		{/if}
	{/await}
</Page>

<style>
	.sections {
		display: flex;
		flex-direction: column;
		gap: var(--sp-7);
	}
	section {
		display: flex;
		flex-direction: column;
		gap: var(--sp-2);
	}
	h2 {
		padding-bottom: var(--sp-2);
		border-bottom: var(--bw) solid var(--border);
		letter-spacing: var(--ls-caps);
		text-transform: uppercase;
	}
	.muted {
		color: var(--text-muted);
	}
	code {
		background: var(--bg-inset);
	}
</style>
