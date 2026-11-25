<script lang="ts">
	import { t } from '$lib/i18n';
	import { CONTROLS, held, said } from './controls';

	// The keys while the controls are lent: a row at the top of the world,
	// clear of the prompt's card and the bars below, each key and what it
	// presses, in the holder's words (`said`), and what holding it does
	// when the holder gives it something (`held`). Leaving is always the
	// last.
	interface Props {
		/** Reads a key of the holder's words; a key it lacks comes back as it went. */
		words?: (key: string) => string;
	}

	let { words }: Props = $props();
</script>

<ul class="controls" aria-label={t('engine.controls')}>
	{#each CONTROLS as control (control)}
		{@const does = held(control, words)}
		<li><kbd>{t(`engine.controls.${control}.keys`)}</kbd> {said(control, words)}</li>
		{#if does}
			<li><kbd>{t(`engine.controls.${control}.keys`)}</kbd> {t('engine.controls.held', { does })}</li>
		{/if}
	{/each}
	<li><kbd>{t('engine.controls.leave.keys')}</kbd> {t('engine.controls.leave')}</li>
</ul>

<style>
	.controls {
		position: absolute;
		left: 50%;
		top: var(--sp-4);
		transform: translateX(-50%);
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		gap: var(--sp-1) var(--sp-4);
		max-width: calc(100% - 2 * var(--sp-4));
		margin: 0;
		padding: var(--sp-2) var(--sp-4);
		list-style: none;
		border-radius: var(--radius-soft);
		background: var(--on-world-card);
		color: var(--on-world-quiet);
		pointer-events: none;
	}
	kbd {
		margin-right: var(--sp-1);
		padding: 0 var(--sp-1);
		border: 1.5px solid var(--on-world-edge);
		border-radius: calc(var(--radius-soft) / 2);
		color: var(--on-world);
		font: inherit;
		font-weight: var(--fw-bold);
	}
</style>
