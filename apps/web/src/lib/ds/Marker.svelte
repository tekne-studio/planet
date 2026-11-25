<script lang="ts">
	import type { HTMLButtonAttributes } from 'svelte/elements';
	import Icon, { type IconName } from './Icon.svelte';

	// Something pointed out in the world: a chip of the over-the-world
	// family, as tall as a control, with a glyph, a name, a note when there
	// is one, and how far, dimmed. Over the thing it stands bare; at the
	// screen's edge a small triangle first in it turns toward where the
	// thing lies. While its card is open an outline says so. It knows
	// nothing of where it stands: whoever places it sets its position.
	interface Props extends HTMLButtonAttributes {
		glyph: IconName;
		name: string;
		/** How far, said: `12 m`, `2.4 km`. */
		distance: string;
		/** How it stands now, said beside the name: `on its way`. */
		note?: string;
		/** At the screen's edge, where the thing lies: degrees clockwise from the screen's up. Absent over the thing. */
		arrow?: number;
		/** Its card is open. */
		open?: boolean;
	}

	let { glyph, name, distance, note, arrow, open = false, ...rest }: Props = $props();
</script>

<button type="button" class="marker" class:open aria-haspopup="dialog" aria-expanded={open} {...rest}>
	{#if arrow !== undefined}
		<span class="arrow" style:--turn="{arrow}deg" aria-hidden="true"></span>
	{/if}
	<Icon name={glyph} />
	<span class="name">{name}</span>
	{#if note}<span class="quiet">{note}</span>{/if}
	<span class="quiet far">{distance}</span>
</button>

<style>
	.marker {
		display: inline-flex;
		align-items: center;
		gap: var(--sp-2);
		min-height: var(--control-h);
		padding: var(--sp-1) var(--sp-3);
		border: none;
		border-radius: var(--radius-soft);
		background: var(--on-world-card);
		box-shadow: inset 0 0 0 var(--bw) var(--on-world-edge);
		color: var(--on-world);
		font: inherit;
		white-space: nowrap;
		cursor: pointer;
		user-select: none;
		-webkit-user-select: none;
	}
	.marker.open,
	.marker:focus-visible {
		outline: var(--bw) solid var(--on-world);
		outline-offset: var(--sp-1);
	}
	.name {
		font-weight: var(--fw-bold);
		max-width: 12em;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.quiet {
		color: var(--on-world-quiet);
	}
	.far {
		font-variant-numeric: tabular-nums;
	}
	/* Drawn pointing up, turned toward where the thing lies. */
	.arrow {
		width: 0;
		height: 0;
		border-left: var(--sp-2) solid transparent;
		border-right: var(--sp-2) solid transparent;
		border-bottom: var(--sp-3) solid var(--on-world);
		transform: rotate(var(--turn));
	}
</style>
