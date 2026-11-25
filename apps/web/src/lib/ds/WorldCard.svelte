<script lang="ts">
	import type { HTMLButtonAttributes } from 'svelte/elements';
	import KeyBadge from './KeyBadge.svelte';
	import Ring from './Ring.svelte';

	// A thing in the world and what a key does to it: a small dark card that
	// hugs its words, the key's badge at its left with the ring of a hold
	// round it, and at its right always a title and an action: the thing's
	// name dimmed above, what a tap does bright below. A hold's verb takes a
	// third, smaller line, marked by the ring's glyph, which needs no words;
	// while it is held the two swap weight, the hold coming forward as the
	// ring fills. With no tap, the hold is the action. The whole card is the
	// button.
	interface Props extends HTMLButtonAttributes {
		/** The key to show; none on a touch screen, where a finger's mark stands in. */
		key?: string;
		title: string;
		/** What a tap does. */
		action?: string;
		/** What a hold does. */
		hold?: string;
		/** How far the hold's ring has closed, while it is held. */
		closed: number | null;
		/** Counts the taps that acted: each a fast sweep of the ring. */
		taps?: number;
		/** Counts the holds that closed: each a pulse of the ring. */
		pulse?: number;
	}

	let { key, title, action, hold, closed, taps = 0, pulse = 0, ...rest }: Props = $props();

	const holding = $derived(closed !== null && closed > 0);
</script>

<button type="button" class="card" {...rest}>
	<Ring {closed} {taps} {pulse}><KeyBadge label={key} /></Ring>
	<span class="lines">
		<span class="title">{title}</span>
		{#if action}<span class="action" class:back={holding && hold}>{action}</span>{/if}
		{#if hold}
			<span class="hold" class:action={!action} class:forward={holding && action}><Ring {closed} />{hold}</span>
		{/if}
	</span>
</button>

<style>
	.card {
		display: inline-flex;
		align-items: center;
		gap: var(--sp-4);
		padding: var(--sp-3) var(--sp-5) var(--sp-3) var(--sp-3);
		border: none;
		border-radius: var(--radius-soft);
		background: var(--on-world-card);
		box-shadow: inset 0 0 0 var(--bw) var(--on-world-edge);
		color: var(--on-world);
		font: inherit;
		text-align: left;
		cursor: pointer;
		user-select: none;
		-webkit-user-select: none;
		-webkit-touch-callout: none;
		touch-action: none;
	}
	.card:focus-visible {
		outline: var(--bw) solid var(--on-world);
		outline-offset: var(--sp-1);
	}
	.lines {
		display: flex;
		flex-direction: column;
		white-space: nowrap;
	}
	.title,
	.hold {
		color: var(--on-world-quiet);
	}
	.action,
	.forward {
		color: var(--on-world);
		font-weight: var(--fw-bold);
		letter-spacing: var(--ls-caps);
		text-transform: uppercase;
	}
	.back {
		color: var(--on-world-quiet);
		font-weight: var(--fw-regular);
	}
	.hold {
		display: inline-flex;
		align-items: center;
		gap: var(--sp-2);
	}
	.title,
	.action,
	.hold {
		transition: color var(--dur-fast) var(--ease-out);
	}
</style>
