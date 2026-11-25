<script lang="ts">
	import { Dial } from '$lib/ds';
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
			<Dial glyph="pause" closed={game.remains} eased={false} line={t('engine.waiting', { s: Math.ceil(game.remains_s) })} />
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
		/* On the screen's middle, smaller with distance, never below half. */
		transform: translate(-50%, -50%) scale(clamp(0.5, var(--near), 1));
	}
</style>
