<script lang="ts">
	import type { LayerProps } from '$lib/plugins/plugin';
	import { t } from './words';

	// Riding over the world: how fast while riding, how high on the board
	// or on a horse in flight, and at which pace on a horse, read from the
	// core's mode, speed, altitude and pace, holding nothing of its own.
	// Mounting a horse, landing and leaving are offered by the prompt, the
	// core's; the board is H's and its button's, taking off and landing F's.
	let { mode, speed_mps, altitude_m, pace }: LayerProps = $props();

	const riding = $derived(mode.startsWith('ride.'));
	const horse = $derived(mode === 'ride.horse');
	const paces = {
		walk: 'pace.walk',
		trot: 'pace.trot',
		gallop: 'pace.gallop',
		run: 'pace.run',
		back: 'pace.back',
		fly: 'pace.fly',
		ceiling: 'pace.ceiling',
		no_room: 'pace.no_room',
		wingless: 'pace.wingless',
		no_ground: 'pace.no_ground'
	} as const;
	const flying = $derived(horse && (pace === 'fly' || pace === 'ceiling'));
	const paceWord = $derived(pace !== null && pace in paces ? t(paces[pace as keyof typeof paces]) : null);
</script>

{#if riding}
	<div class="ride">
		{#if horse && paceWord !== null}
			<span class="reading">{paceWord}</span>
		{/if}
		{#if speed_mps !== null}
			<span class="reading">{t('speed', { n: speed_mps.toFixed(0) })}</span>
		{/if}
		{#if (!horse || flying) && altitude_m !== null}
			<span class="reading">{t('altitude', { n: Math.max(0, altitude_m).toFixed(0) })}</span>
		{/if}
	</div>
{/if}

<style>
	/* Over the help, on the left: the corner nothing else holds; on a touch
	   screen over the thumbs' home, where the stick never hides it. */
	.ride {
		position: absolute;
		left: var(--sp-4);
		bottom: var(--thumb-home, calc(2 * var(--sp-5) + var(--control-h)));
		z-index: var(--z-panel);
		display: flex;
		align-items: center;
		gap: var(--sp-2);
		pointer-events: none;
	}
	.reading {
		display: flex;
		align-items: center;
		min-height: var(--control-h);
		padding: 0 var(--sp-3);
		background: var(--bg-overlay);
		color: var(--text);
		font-variant-numeric: tabular-nums;
		letter-spacing: var(--ls-caps);
		text-transform: uppercase;
	}
</style>
