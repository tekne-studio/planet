<script lang="ts">
	import type { Snippet } from 'svelte';

	// A clock round what it holds: how far a hold has closed, 0 to 1,
	// sweeping clockwise from the top, eased. At rest it draws nothing, not
	// even the cap of an empty arc. It follows `closed` frame by frame and
	// drains fast at null; each tap that `taps` counts sweeps it round once,
	// fast, and each hold that `pulse` counts, and each tap, sends a pulse
	// out from it, so a press is felt. With nothing to hold it is a glyph the
	// size of a letter: the mark of a hold beside its verb, which fills as
	// the ring does. Not `eased`, it is a clock told the time, a countdown
	// draining at the pace it is told.
	interface Props {
		closed: number | null;
		taps?: number;
		pulse?: number;
		eased?: boolean;
		children?: Snippet;
	}

	let { closed, taps = 0, pulse = 0, eased = true, children }: Props = $props();

	/** Slow out of the start and into the end, so the sweep reads as one motion. */
	const ease = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2);

	const shown = $derived(eased ? ease(closed ?? 0) : (closed ?? 0));
</script>

<span class="ring" class:held={closed !== null} class:glyph={!children}>
	{@render children?.()}
	<svg viewBox="0 0 40 40" aria-hidden="true">
		<circle class="track" cx="20" cy="20" r="17" />
		<!-- Hidden at zero: a round cap on an arc of no length is a dot. -->
		<circle class="arc" class:empty={shown <= 0} cx="20" cy="20" r="17" pathLength="1" style:stroke-dashoffset={1 - shown} />
		{#key taps}
			{#if taps}<circle class="arc sweep" cx="20" cy="20" r="17" pathLength="1" />{/if}
		{/key}
	</svg>
	{#key `${pulse}:${taps}`}
		{#if pulse || taps}<span class="pulse"></span>{/if}
	{/key}
</span>

<style>
	.ring {
		position: relative;
		display: inline-grid;
		place-items: center;
	}
	svg {
		position: absolute;
		inset: calc(-1 * var(--sp-2));
		width: calc(100% + 2 * var(--sp-2));
		height: calc(100% + 2 * var(--sp-2));
		/* From twelve o'clock, clockwise. */
		transform: rotate(-90deg);
		overflow: visible;
		pointer-events: none;
	}
	circle {
		fill: none;
		stroke-width: 3;
	}
	.track {
		stroke: var(--on-world-veil);
		opacity: 0;
		transition: opacity var(--dur-fast) var(--ease-out);
	}
	.held .track {
		opacity: 1;
	}
	.arc {
		stroke: var(--on-world);
		stroke-dasharray: 1;
		stroke-linecap: round;
		/* Drains fast and fades as it goes; while held it follows what it is told. */
		transition:
			stroke-dashoffset var(--dur-fast) var(--ease-out),
			opacity var(--dur-fast) var(--ease-out);
	}
	.held .arc {
		transition: none;
	}
	.arc.empty {
		opacity: 0;
	}
	/* Round once in a tenth of a second, then gone. */
	.sweep {
		opacity: 0;
		animation: sweep 220ms linear forwards;
	}
	@keyframes sweep {
		0% {
			stroke-dashoffset: 1;
			opacity: 1;
		}
		45% {
			stroke-dashoffset: 0;
			opacity: 1;
		}
		100% {
			stroke-dashoffset: 0;
			opacity: 0;
		}
	}
	.glyph {
		width: 1em;
		height: 1em;
	}
	.glyph svg {
		inset: 0;
		width: 100%;
		height: 100%;
	}
	.glyph circle {
		stroke-width: 6;
	}
	.glyph .track {
		opacity: 1;
		stroke: var(--on-world-quiet);
	}
	.pulse {
		position: absolute;
		inset: calc(-1 * var(--sp-2));
		border: 2px solid var(--on-world);
		border-radius: var(--radius-round);
		animation: pulse 240ms var(--ease-out) forwards;
		pointer-events: none;
	}
	@keyframes pulse {
		from {
			transform: scale(1);
			opacity: 1;
		}
		to {
			transform: scale(1.4);
			opacity: 0;
		}
	}
</style>
