<script lang="ts">
	import type { Snippet } from 'svelte';

	// A column of `Knob`s: the controls share one width and the ways back to
	// their defaults one column at their right, as wide as the widest and
	// never narrower than a short value's, so columns side by side line up.
	interface Props {
		/** The knobs. */
		children: Snippet;
	}

	let { children }: Props = $props();
</script>

<div class="knobs">{@render children()}</div>

<style>
	.knobs {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(calc(var(--sp-8) + var(--sp-7)), auto);
		align-items: end;
		gap: var(--sp-3);
	}
</style>
