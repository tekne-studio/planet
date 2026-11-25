<script lang="ts">
	import Icon from './Icon.svelte';

	// The key to press on something in the world: a round badge a little
	// lighter than the card it sits on, holding the key as a small rounded
	// cap. With no key, a finger's mark: on a touch screen the card is the
	// button.
	interface Props {
		label?: string;
	}

	let { label }: Props = $props();
</script>

<span class="badge">
	{#if label}<span class="cap">{label}</span>{:else}<Icon name="pointer" />{/if}
</span>

<style>
	.badge {
		display: inline-grid;
		place-items: center;
		width: var(--key-mark);
		height: var(--key-mark);
		border-radius: var(--radius-round);
		background: var(--on-world-badge);
		color: var(--on-world);
	}
	.cap {
		display: grid;
		place-items: center;
		box-sizing: border-box;
		min-width: calc(var(--key-mark) / 2 + var(--sp-1));
		height: calc(var(--key-mark) / 2 + var(--sp-1));
		padding: 0 var(--sp-1);
		border: 1.5px solid var(--on-world);
		border-radius: calc(var(--radius-soft) / 2);
		font-weight: var(--fw-bold);
		line-height: 1;
	}
</style>
