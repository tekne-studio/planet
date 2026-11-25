<script lang="ts">
	import type { Snippet } from 'svelte';
	import Icon from './Icon.svelte';

	// A setting and the way back to its default: the control, then, while it
	// stands off its default, a small button at its right that says the
	// default and returns to it. A row of `Knobs`, whose columns it takes:
	// the button's room stays when it is hidden, so the controls keep one
	// width.
	interface Props {
		/** It stands off its default. */
		changed: boolean;
		/** The default, as the control shows a value. */
		fallback: string;
		/** The button's accessible name and tooltip: what it returns to. */
		resetLabel: string;
		onreset: () => void;
		/** The control: a Slider, a Checkbox, a Select. */
		children: Snippet;
	}

	let { changed, fallback, resetLabel, onreset, children }: Props = $props();
</script>

<div class="knob">
	<div class="control">{@render children()}</div>
	<button type="button" class="reset" class:reset--off={!changed} aria-label={resetLabel} title={resetLabel} disabled={!changed} onclick={onreset}>
		<Icon name="rotate-ccw" />
		<span>{fallback}</span>
	</button>
</div>

<style>
	.knob {
		display: grid;
		grid-column: 1 / -1;
		grid-template-columns: subgrid;
		align-items: end;
	}
	.reset {
		display: inline-flex;
		align-items: center;
		gap: var(--sp-2);
		min-height: var(--control-h);
		padding: 0 var(--sp-2);
		border: var(--bw) solid var(--border);
		background: transparent;
		color: var(--text-muted);
		font: inherit;
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
		cursor: pointer;
	}
	.reset:hover {
		border-color: var(--border-strong);
		color: var(--text);
	}
	.reset:focus-visible {
		outline: var(--bw) solid var(--focus);
		outline-offset: var(--sp-1);
	}
	.reset--off {
		visibility: hidden;
	}
</style>
