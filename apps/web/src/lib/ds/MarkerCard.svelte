<script lang="ts">
	import Icon, { type IconName } from './Icon.svelte';

	// What a pressed marker offers: a card of the over-the-world family, its
	// title the marker's glyph, name, note and how far, and below it the
	// choices, each a verb in capitals. Opened, it takes the focus to its
	// first choice, so the keys reach it. Whoever opens it sets where it
	// stands, and closes it.
	interface Props {
		glyph: IconName;
		name: string;
		/** How far, said: `12 m`, `2.4 km`. */
		distance: string;
		/** How it stands now, said beside the name. */
		note?: string;
		/** In order; the first takes the focus. */
		choices: readonly { label: string; onclick: () => void }[];
	}

	let { glyph, name, distance, note, choices }: Props = $props();

	function focusFirst(card: HTMLElement) {
		card.querySelector<HTMLElement>('button')?.focus();
	}
</script>

<div class="card" role="dialog" aria-label={name} {@attach focusFirst}>
	<div class="title">
		<Icon name={glyph} />
		<span>{name}</span>
		{#if note}<span class="quiet">{note}</span>{/if}
		<span class="quiet far">{distance}</span>
	</div>
	<div class="choices">
		{#each choices as choice, index (index)}
			<button type="button" class="choice" onclick={choice.onclick}>{choice.label}</button>
		{/each}
	</div>
</div>

<style>
	.card {
		display: flex;
		flex-direction: column;
		gap: var(--sp-3);
		min-width: 12rem;
		max-width: min(20rem, calc(100vw - 2 * var(--sp-4)));
		padding: var(--sp-3) var(--sp-4);
		border-radius: var(--radius-soft);
		background: var(--on-world-card);
		box-shadow: inset 0 0 0 var(--bw) var(--on-world-edge);
		color: var(--on-world);
	}
	.title {
		display: flex;
		align-items: center;
		gap: var(--sp-2);
		font-weight: var(--fw-bold);
		white-space: nowrap;
	}
	.quiet {
		color: var(--on-world-quiet);
	}
	.far {
		font-variant-numeric: tabular-nums;
	}
	.choices {
		display: flex;
		flex-wrap: wrap;
		gap: var(--sp-2);
	}
	.choice {
		min-height: var(--control-h);
		padding: var(--sp-2) var(--sp-4);
		border: none;
		border-radius: var(--radius-soft);
		background: transparent;
		box-shadow: inset 0 0 0 var(--bw) var(--on-world-edge);
		color: var(--on-world);
		font: inherit;
		font-weight: var(--fw-bold);
		letter-spacing: var(--ls-caps);
		text-transform: uppercase;
		cursor: pointer;
	}
	.choice:hover,
	.choice:focus-visible {
		box-shadow: inset 0 0 0 var(--bw) var(--on-world);
	}
</style>
