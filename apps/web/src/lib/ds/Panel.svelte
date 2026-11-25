<script lang="ts">
	import type { Snippet } from 'svelte';

	// A bordered box that floats over the engine canvas. It positions itself in
	// a corner of the nearest positioned ancestor, or in its middle, as wide as
	// its content up to a page. A large panel is a middle one of a fixed
	// size, a page wide and most of the height, whatever it shows: one with
	// sections keeps still as they change. On a phone it is the whole screen.
	// A panel scrolls on itself, its bar at its edge; a large panel's content
	// brings its own scroller, which runs to the panel's right and bottom
	// edges, so the bar hugs the edge, and pads its content by `--panel-pad`.
	interface Props {
		title: string;
		/** Sits opposite the title, for example a way out. */
		aside?: Snippet;
		corner?: 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right' | 'middle';
		/** Placed under the row of buttons in its top corner, which opened it. */
		under?: boolean;
		/** In the middle at a fixed size, the whole screen on a phone. */
		large?: boolean;
		children: Snippet;
	}

	let { title, aside, corner = 'top-left', under = false, large = false, children }: Props = $props();
</script>

<section class="panel panel--{large ? 'middle' : corner}" class:panel--under={under && !large} class:panel--large={large}>
	<header>
		<h2>{title}</h2>
		{#if aside}{@render aside()}{/if}
	</header>
	{@render children()}
</section>

<style>
	.panel {
		position: absolute;
		z-index: var(--z-panel);
		display: flex;
		flex-direction: column;
		gap: var(--sp-4);
		width: min(var(--panel-w), calc(100% - 2 * var(--sp-4)));
		max-height: calc(100% - 2 * var(--sp-4));
		overflow-y: auto;
		padding: var(--sp-4);
		border: var(--bw) solid var(--border-strong);
		background: var(--bg-overlay);
		color: var(--text);
		backdrop-filter: blur(var(--sp-2));
	}
	.panel--top-left {
		top: var(--sp-4);
		left: var(--sp-4);
	}
	.panel--top-right {
		top: var(--sp-4);
		right: var(--sp-4);
	}
	/* A panel stops short of the row of buttons in the corners across from
	   it, so a tall one never covers a button: it scrolls instead. */
	.panel--top-left,
	.panel--top-right {
		max-height: calc(100% - 2 * var(--sp-4) - var(--sp-5) - var(--control-h) - 2 * var(--bw));
	}
	.panel--bottom-left,
	.panel--bottom-right {
		max-height: calc(100% - 3 * var(--sp-4) - var(--control-h) - 2 * var(--bw));
	}
	/* Under the row of buttons that opened it, and short of the row in the
	   bottom corners, so a tall one covers neither. */
	.panel--under {
		top: calc(2 * var(--sp-4) + var(--control-h) + 2 * var(--bw));
		max-height: calc(100% - 3 * var(--sp-4) - var(--sp-5) - 2 * var(--control-h) - 4 * var(--bw));
	}
	/* A phone is too narrow for a panel beside the row of buttons across
	   from it: on a touch screen every top panel stands under the rows. */
	@media (pointer: coarse) {
		.panel--top-left,
		.panel--top-right {
			top: calc(2 * var(--sp-4) + var(--control-h) + 2 * var(--bw));
			max-height: calc(100% - 3 * var(--sp-4) - var(--sp-5) - 2 * var(--control-h) - 4 * var(--bw));
		}
	}
	.panel--bottom-left {
		bottom: var(--sp-4);
		left: var(--sp-4);
	}
	.panel--bottom-right {
		bottom: var(--sp-4);
		right: var(--sp-4);
	}
	/* In the middle, short of the rows of buttons above and below it. */
	.panel--middle {
		top: 50%;
		left: 50%;
		translate: -50% -50%;
		width: max-content;
		max-width: min(var(--page-wide), calc(100% - 2 * var(--sp-4)));
		max-height: calc(100% - 2 * (var(--sp-5) + var(--sp-4) + var(--control-h) + 2 * var(--bw)));
	}
	.panel--large {
		--panel-pad: var(--sp-6);
		width: min(var(--page-wide), calc(100% - 2 * var(--sp-4)));
		height: min(var(--panel-large-h), calc(100% - 2 * (var(--sp-5) + var(--sp-4) + var(--control-h) + 2 * var(--bw))));
		padding: var(--sp-5) 0 0 var(--panel-pad);
		/* Its sections scroll inside it, under a title that stays. */
		overflow: hidden;
	}
	@media (max-width: 640px) {
		.panel--large {
			inset: 0;
			translate: none;
			width: 100%;
			max-width: none;
			height: 100%;
			max-height: none;
			--panel-pad: var(--sp-4);
			z-index: var(--z-topbar);
			padding: var(--sp-4) 0 0 var(--panel-pad);
			border: none;
			background: var(--bg);
		}
	}
	.panel--large > header {
		padding-right: var(--panel-pad);
	}
	header {
		display: flex;
		justify-content: space-between;
		gap: var(--sp-3);
	}
	h2 {
		letter-spacing: var(--ls-caps);
		text-transform: uppercase;
	}
</style>
