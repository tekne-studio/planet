<script lang="ts">
	import { Icon, Ring } from '$lib/ds';
	import { t } from '$lib/i18n';
	import type { Waiting } from './protocol';

	// A game held for the player who left it: over the middle of its screen,
	// a pause in a ring that drains as the game's window does, and the
	// seconds left under it. The engine says where each one in view is every
	// frame; this only draws there, as `Labels` does. The prompt says what
	// to do in words: playing it again goes on where it held.
	interface Props {
		waiting: Waiting[];
	}

	let { waiting }: Props = $props();
</script>

<div class="waiting" aria-hidden="true">
	{#each waiting as game (game.id)}
		<div class="over" style:left="{game.x * 100}%" style:top="{game.y * 100}%" style:--near={Math.min(1, 3 / game.distance_m)}>
			<span class="mark"><Ring closed={game.remains} eased={false}><Icon name="pause" /></Ring></span>
			<span class="left">{t('engine.waiting', { s: Math.ceil(game.remains_s) })}</span>
		</div>
	{/each}
</div>

<style>
	.waiting {
		position: absolute;
		inset: 0;
		overflow: hidden;
		pointer-events: none;
	}
	.over {
		position: absolute;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--sp-3);
		/* On the screen's middle, smaller with distance, never below half. */
		transform: translate(-50%, -50%) scale(clamp(0.5, var(--near), 1));
	}
	.mark {
		display: grid;
		place-items: center;
		padding: var(--sp-4);
		border-radius: var(--radius-round);
		background: var(--on-world-card);
		box-shadow: inset 0 0 0 var(--bw) var(--on-world-edge);
		color: var(--on-world);
		font-size: calc(var(--fs) * 2.4);
	}
	.left {
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
