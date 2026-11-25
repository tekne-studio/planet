<script lang="ts">
	import Icon, { type IconName } from './Icon.svelte';
	import Ring from './Ring.svelte';

	// A dial over the world: a glyph in a ring, on a round chip of the
	// over-the-world family, and a line under it. The ring is a hold's,
	// eased, or a countdown's, told its time; the glyph sits clear inside
	// it, by the ring's own measure. It knows nothing of where it stands:
	// whoever places it sets its position.
	interface Props {
		glyph: IconName;
		/** How far the ring has closed, 0 to 1; null at rest. */
		closed: number | null;
		/** A hold's ring eases; a countdown's is told its time. */
		eased?: boolean;
		/** Said under it: `4 s`, `reset`. */
		line: string;
	}

	let { glyph, closed, eased = true, line }: Props = $props();
</script>

<span class="dial">
	<span class="chip"><Ring {closed} {eased}><span class="face"><Icon name={glyph} /></span></Ring></span>
	<span class="line">{line}</span>
</span>

<style>
	.dial {
		display: inline-flex;
		flex-direction: column;
		align-items: center;
		gap: var(--sp-3);
	}
	.chip {
		display: grid;
		place-items: center;
		padding: var(--sp-4);
		border-radius: var(--radius-round);
		background: var(--on-world-card);
		box-shadow: inset 0 0 0 var(--bw) var(--on-world-edge);
		color: var(--on-world);
	}
	/* What the ring is drawn round: the dial's one measure. The glyph is the
	   square clear inside the ring's inner edge, so no corner of it, an
	   arrow's head, touches the stroke. */
	.face {
		--face: calc(var(--fs) * 3.4);
		--icon-side: calc((var(--face) + 2 * var(--ring-out)) * var(--ring-clear));
		display: grid;
		place-items: center;
		width: var(--face);
		height: var(--face);
	}
	.line {
		color: var(--on-world);
		font-weight: var(--fw-bold);
		font-variant-numeric: tabular-nums;
		text-shadow:
			0 0 var(--sp-1) var(--on-world-shadow),
			0 0 var(--sp-1) var(--on-world-shadow),
			0 var(--bw) var(--sp-1) var(--on-world-shadow);
		white-space: nowrap;
	}
</style>
