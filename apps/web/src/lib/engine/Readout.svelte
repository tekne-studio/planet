<script lang="ts">
	import { onMount } from 'svelte';
	import { Pairs } from '$lib/ds';
	import { t } from '$lib/i18n';
	import { rows, type Facts } from './facts';
	import type { Held } from './index';
	import { Live } from './live.svelte';

	// While Settings or the address asks for it, what the machine says and
	// how far its start went, written over the world so a photo of the screen
	// tells where a start stopped: the list the graphics dialog shows, live,
	// in two halves side by side.
	interface Props {
		facts: Facts;
		/** What the engine holds down now, once it runs. */
		held: () => Held | undefined;
	}

	let { facts, held }: Props = $props();

	const live = new Live();
	onMount(() => live.watch(held));

	const list = $derived(rows({ ...facts, live: live.now }, t));
	const halves = $derived([list.slice(0, Math.ceil(list.length / 2)), list.slice(Math.ceil(list.length / 2))]);
</script>

<aside class="readout" aria-label={t('engine.machine.title')}>
	<h2>{t('engine.machine.title')}</h2>
	<div class="halves">
		{#each halves as half, i (i)}
			<div class="half"><Pairs onWorld pairs={half} /></div>
		{/each}
	</div>
</aside>

<style>
	/* Top right, under the bar and the place line, one line high: the left
	   holds what a page lays over the world, the founding's panel among them.
	   Its halves side by side keep it half as tall, well above the buttons
	   at the bottom right; on a narrow screen, one under the other. Never
	   in the way of a hand: a finger or the pointer goes through it. */
	.readout {
		--half: 320px;
		position: absolute;
		top: calc(2 * var(--sp-4) + var(--control-h) + 2 * var(--bw) + 1lh + 2 * var(--sp-1) + var(--sp-2));
		right: var(--sp-4);
		z-index: var(--z-panel);
		max-width: min(calc(2 * var(--half) + var(--sp-5) + 2 * var(--sp-3)), calc(100% - 2 * var(--sp-4)));
		padding: var(--sp-2) var(--sp-3);
		border-radius: var(--radius-soft);
		background: var(--on-world-card);
		box-shadow: inset 0 0 0 var(--bw) var(--on-world-edge);
		color: var(--on-world);
		pointer-events: none;
	}
	.halves {
		display: flex;
		flex-wrap: wrap;
		gap: 0 var(--sp-5);
	}
	.half {
		flex: 1 1 calc(var(--half) - var(--sp-5));
		min-width: 0;
	}
	h2 {
		margin: 0 0 var(--sp-1);
		font: inherit;
		color: var(--on-world-quiet);
	}
</style>
