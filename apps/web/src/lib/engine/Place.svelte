<script lang="ts">
	import { t } from '$lib/i18n';

	// Where the body is, in one line of small white text at the top right,
	// under the bar: the place, the hour there, the way it faces, how high
	// and how fast. It floats on the world as the prompt's card does, on a
	// dark translucent strip that reads over snow at noon and over the
	// night. The place is how people tell each other where they are: a click
	// copies it.
	interface Props {
		place: string;
		/** The hour here, minutes past midnight; null on the moon, which keeps no clock. */
		hour_min: number | null;
		/** Clockwise from north, degrees; null at a pole. */
		bearing_deg: number | null;
		altitude_m: number;
		speed_mps: number;
	}

	let { place, hour_min, bearing_deg, altitude_m, speed_mps }: Props = $props();

	// A 24 hour clock, 00:00 to 23:59: digits, the same in every language.
	const hour = $derived(hour_min === null ? null : `${String(Math.floor(hour_min / 60)).padStart(2, '0')}:${String(hour_min % 60).padStart(2, '0')}`);

	// The compass point a bearing lands on, and the angle beside it. A rose has
	// sixteen points, so each is 22.5 degrees wide. A pole has no bearing.
	const facing = $derived.by(() => {
		if (bearing_deg === null) return t('engine.stats.atThePole');
		const rose = t('engine.compass.rose').split(',');
		const point = rose[Math.round(bearing_deg / 22.5) % rose.length];
		return `${point} ${t('engine.stats.degrees', { n: bearing_deg.toFixed(0) })}`;
	});

	/** The place was just copied: the line says so for a moment. */
	let copied = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;

	async function copy() {
		try {
			await navigator.clipboard.writeText(place);
		} catch {
			// a browser that refuses the clipboard: the address bar holds the place too
			return;
		}
		clearTimeout(timer);
		copied = true;
		timer = setTimeout(() => (copied = false), 1500);
	}
</script>

<div class="line">
	<button type="button" class="place" title={t('engine.place.copy')} aria-label="{t('engine.stats.place')} {place}. {t('engine.place.copy')}" onclick={copy}>
		{copied ? t('engine.place.copied') : place}
	</button>
	{#if hour !== null}
		<span title={t('engine.stats.hour')}>{hour}</span>
	{/if}
	<span title={t('engine.stats.facing')}>{facing}</span>
	<span title={t('engine.stats.altitude')}>{t('engine.stats.metres', { n: altitude_m.toFixed(1) })}</span>
	<span title={t('engine.stats.speed')}>{t('engine.stats.metresPerSecond', { n: speed_mps.toFixed(1) })}</span>
</div>

<style>
	/* Under the bar of buttons, at its right edge: nothing else holds the
	   top right below the bar, and the chat and the menus keep to the
	   bottom and the left. */
	.line {
		position: absolute;
		top: calc(2 * var(--sp-4) + var(--control-h) + 2 * var(--bw));
		right: var(--sp-4);
		z-index: var(--z-panel);
		display: flex;
		align-items: center;
		gap: var(--sp-3);
		max-width: calc(100% - 2 * var(--sp-4));
		padding: var(--sp-1) var(--sp-3);
		border-radius: var(--radius-soft);
		background: var(--on-world-card);
		box-shadow: inset 0 0 0 var(--bw) var(--on-world-edge);
		color: var(--on-world-quiet);
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
		pointer-events: none;
	}
	.place {
		min-width: 0;
		overflow: hidden;
		padding: 0;
		border: none;
		background: none;
		color: var(--on-world);
		font: inherit;
		text-overflow: ellipsis;
		cursor: copy;
		pointer-events: auto;
	}
	.place:hover {
		text-decoration: underline;
	}
	.place:focus-visible {
		outline: var(--bw) solid var(--on-world);
		outline-offset: var(--sp-1);
	}
</style>
