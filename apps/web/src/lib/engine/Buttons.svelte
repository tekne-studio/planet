<script lang="ts">
	import type { Snippet } from 'svelte';
	import { t } from '$lib/i18n';

	// The buttons over the world at the bottom right, the core's and each
	// plugin's `Button`, laid out as one group here, so a plugin's button
	// brings no place of its own. The first sits nearest the corner. On a
	// keyboard they stand over the foot of the world; on a touch screen in a
	// column over the thumbs' buttons, and on a screen held sideways in a
	// row beside their top row, so none climbs to the bar. Never over the
	// card or the stick, the left thumb's.
	interface Props {
		touch: boolean;
		children: Snippet;
	}

	let { touch, children }: Props = $props();
</script>

<div class="buttons" class:touch role="group" aria-label={t('engine.buttons')}>
	{@render children()}
</div>

<style>
	.buttons {
		--control-bg: var(--bg-overlay);
		position: absolute;
		right: var(--sp-4);
		bottom: calc(2 * var(--sp-5) + var(--control-h));
		z-index: var(--z-panel);
		display: flex;
		flex-direction: column-reverse;
		align-items: flex-end;
		gap: var(--sp-2);
	}
	.buttons.touch {
		bottom: var(--thumb-home);
	}
	@media (orientation: landscape) and (max-height: 540px) {
		.buttons.touch {
			right: calc(3 * var(--sp-4) + 2 * var(--thumb-h));
			bottom: calc(2 * var(--sp-5) + var(--control-h) + var(--sp-4) + 1.5 * var(--thumb-h) - var(--control-h) / 2);
			flex-direction: row-reverse;
			align-items: center;
		}
	}
</style>
