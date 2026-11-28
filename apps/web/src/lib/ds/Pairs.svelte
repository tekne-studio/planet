<script lang="ts">
	// Labels and their values, small and dense, a pair a line: what a thing
	// says of itself. `onWorld` takes the colours of what floats over the world.
	interface Props {
		pairs: readonly { label: string; value: string }[];
		onWorld?: boolean;
	}

	let { pairs, onWorld = false }: Props = $props();
</script>

<dl class="pairs" class:pairs--world={onWorld}>
	{#each pairs as pair, i (i)}
		<dt>{pair.label}</dt>
		<dd>{pair.value}</dd>
	{/each}
</dl>

<style>
	.pairs {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 0 var(--sp-3);
		margin: 0;
		font-variant-numeric: tabular-nums;
	}
	dt {
		color: var(--text-muted);
	}
	dd {
		margin: 0;
		overflow-wrap: anywhere;
	}
	.pairs--world dt {
		color: var(--on-world-quiet);
	}
</style>
