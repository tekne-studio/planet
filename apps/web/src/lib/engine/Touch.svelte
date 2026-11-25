<script lang="ts">
	import { Icon } from '$lib/ds';
	import { t } from '$lib/i18n';
	import { Fingers } from './fingers';

	// The world's thumbs: what a touch screen has in place of the body's
	// keys. The stick is the web shell's, opened by the left thumb anywhere
	// on the left half (`Stick` draws it). The buttons press the keys a
	// keyboard would, through the engine's own key path: up held jumps on
	// foot and rises in flight, down held sinks in flight, on one's own or
	// aloft on a pegasus, the plane takes flight or lands, and the eye
	// swings the camera into the head and out. They sit in the bottom right
	// corner, where the right thumb rests. While the controls are lent, a
	// holder's `Pad` takes their place.
	interface Props {
		/** Whether the body flies, on its own or aloft on what it rides. */
		flying: boolean;
		/** Presses a key or lets it go, by the name the web gives it. */
		onkey: (code: string, down: boolean) => void;
	}

	let { flying, onkey }: Props = $props();

	const fingers = new Fingers((code, down) => onkey(code, down));
	$effect(() => fingers.listen());
</script>

<div class="thumbs" role="group" aria-label={t('engine.touch')}>
	<button type="button" class="eye" aria-label={t('engine.touch.eye')} {...fingers.tap('KeyP')}>
		<Icon name="eye" />
	</button>
	<button type="button" class="fly" aria-label={t('engine.touch.fly')} aria-pressed={flying} {...fingers.tap('KeyF')}>
		<Icon name="plane" />
	</button>
	{#if flying}
		<button type="button" class="down" class:held={fingers.holds('KeyC')} aria-label={t('engine.touch.down')} {...fingers.hold('KeyC')}>
			<Icon name="chevrons-down" />
		</button>
	{/if}
	<button type="button" class="up" class:held={fingers.holds('Space')} aria-label={t('engine.touch.up')} {...fingers.hold('Space')}>
		<Icon name="chevrons-up" />
	</button>
</div>

<style>
	/* Up at the corner, under the thumb; the plane over it, down beside it
	   in flight, the eye, pressed least, farthest. Above the foot of the
	   world, where a plugin's bar sits. */
	.thumbs {
		position: absolute;
		right: var(--sp-4);
		bottom: calc(2 * var(--sp-5) + var(--control-h));
		z-index: var(--z-panel);
		display: grid;
		grid-template: 'eye fly' auto 'down up' auto / auto auto;
		justify-items: end;
		gap: var(--sp-4);
	}
	button {
		display: grid;
		place-items: center;
		width: var(--thumb-h);
		height: var(--thumb-h);
		border: var(--bw) solid var(--border-strong);
		background: var(--bg-overlay);
		color: var(--text);
		font-size: calc(2 * var(--fs));
		touch-action: none;
		user-select: none;
		-webkit-user-select: none;
		-webkit-touch-callout: none;
	}
	button:active,
	button[aria-pressed='true'],
	.held {
		background: var(--action);
		color: var(--action-text);
	}
	.eye {
		grid-area: eye;
	}
	.fly {
		grid-area: fly;
	}
	.down {
		grid-area: down;
	}
	.up {
		grid-area: up;
	}
</style>
