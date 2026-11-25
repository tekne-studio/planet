<script lang="ts">
	import { Icon } from '$lib/ds';
	import { t } from '$lib/i18n';
	import { LEAVE_KEY, PAD_KEYS, said, type Control } from './controls';
	import { Fingers } from './fingers';

	// A holder's pad on a touch screen, in the world's thumbs' place while
	// the controls are lent: a button for each control it is given, in the
	// holder's words, and one to leave. The buttons press the keys a
	// keyboard would, through the engine's own key path, so what reaches a
	// game is what the keys press. The stick is the web shell's, as on foot:
	// the left thumb opens it anywhere on the left half and it presses four
	// ways; while no finger holds it, a ring says where it rests. A and B
	// stand large and apart under the right thumb, which rolls from one to
	// the other; leave and start sit small over them, out of the stick's
	// half, where a thumb landing for the stick never leaves. The pad keeps
	// to the two sides, so the picture in the middle is never under it,
	// held upright or sideways.
	interface Props {
		/** What the holder is pressed by, each drawn once. */
		controls: readonly Control[];
		/** Whether a finger holds the stick, drawn where it is. */
		sticking: boolean;
		/** Reads a key of the holder's words; a key it lacks comes back as it went. */
		words?: (key: string) => string;
		/** Presses a key or lets it go, by the name the web gives it. */
		onkey: (code: string, down: boolean) => void;
	}

	let { controls, sticking, words, onkey }: Props = $props();

	const fingers = new Fingers((code, down) => onkey(code, down));
	$effect(() => fingers.listen());

	const buttons = $derived(controls.filter((control): control is keyof typeof PAD_KEYS => control !== 'stick'));
</script>

<div class="pad" role="group" aria-label={t('engine.controls')}>
	{#if controls.includes('stick') && !sticking}
		<span class="rest" aria-hidden="true">{said('stick', words)}</span>
	{/if}
	<div class="buttons">
		<button type="button" class="leave" {...fingers.tap(LEAVE_KEY)}>
			<Icon name="log-out" />{t('engine.controls.leave')}
		</button>
		{#each buttons as control (control)}
			<button type="button" class={control} class:held={fingers.holds(PAD_KEYS[control])} {...fingers.hold(PAD_KEYS[control], control !== 'start')}>
				{said(control, words)}
			</button>
		{/each}
	</div>
</div>

<style>
	/* Along the foot, the stick's ring at the left and the buttons at the
	   right, as high as the world's thumbs, so a plugin's bar keeps its
	   place below. The pad itself takes no finger: the stick's are the
	   canvas's. */
	.pad {
		position: absolute;
		left: var(--sp-4);
		right: var(--sp-4);
		bottom: calc(2 * var(--sp-5) + var(--control-h));
		z-index: var(--z-panel);
		display: flex;
		align-items: flex-end;
		pointer-events: none;
	}
	.rest {
		display: grid;
		place-items: center;
		width: calc(2 * var(--thumb-h));
		height: calc(2 * var(--thumb-h));
		border: var(--bw) dashed var(--on-world-quiet);
		border-radius: var(--radius-round);
		color: var(--on-world-quiet);
		font-weight: var(--fw-bold);
	}
	/* B under the thumb's root, A beyond it; leave over B and start over A,
	   as small as a pad's middle buttons. */
	.buttons {
		display: grid;
		grid-template: 'leave start' auto 'sprint jump' auto / var(--pad-h) var(--pad-h);
		gap: var(--sp-4) var(--sp-5);
		margin-left: auto;
	}
	button {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--sp-1);
		height: var(--control-h);
		padding: 0;
		border: var(--bw) solid var(--border-strong);
		background: var(--bg-overlay);
		color: var(--text);
		font-weight: var(--fw-bold);
		pointer-events: auto;
		touch-action: none;
		user-select: none;
		-webkit-user-select: none;
		-webkit-touch-callout: none;
	}
	button:active,
	.held {
		background: var(--action);
		color: var(--action-text);
	}
	.leave {
		grid-area: leave;
	}
	.start {
		grid-area: start;
	}
	.jump,
	.sprint {
		height: var(--pad-h);
		font-size: calc(2 * var(--fs));
	}
	.jump {
		grid-area: jump;
	}
	.sprint {
		grid-area: sprint;
	}
</style>
