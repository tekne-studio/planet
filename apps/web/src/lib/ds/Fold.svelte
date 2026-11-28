<script lang="ts">
	import type { Snippet } from 'svelte';

	// A part folded under its title: what most people pass by, one click
	// from sight. Native <details>, so it opens from the keyboard too.
	interface Props {
		title: string;
		open?: boolean;
		children: Snippet;
	}

	let { title, open = $bindable(false), children }: Props = $props();
</script>

<details class="fold" bind:open>
	<summary>{title}</summary>
	<div class="body">{@render children()}</div>
</details>

<style>
	summary {
		display: flex;
		align-items: center;
		gap: var(--sp-3);
		min-height: var(--control-h);
		color: var(--text);
		letter-spacing: var(--ls-caps);
		text-transform: uppercase;
		list-style: none;
		cursor: pointer;
	}
	summary::-webkit-details-marker {
		display: none;
	}
	summary::before {
		content: '+';
		width: 1ch;
	}
	.fold[open] > summary::before {
		content: '−';
	}
	summary:focus-visible {
		outline: var(--bw) solid var(--focus);
		outline-offset: var(--sp-1);
	}
	.body {
		display: flex;
		flex-direction: column;
		gap: var(--sp-3);
		padding-top: var(--sp-3);
	}
</style>
