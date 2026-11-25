<script lang="ts">
	import { Icon } from '$lib/ds';
	import { t } from '$lib/i18n';
	import { keep, kept } from '$lib/kept';
	import { LEAVE_KEY, PAD_KEYS, held, said, type Control } from './controls';
	import { Fingers } from './fingers';
	import type { Clear } from './index';
	import { WAY_KEYS, crossed, readWays, type Way, type Ways } from './ways';

	// A holder's pad on a touch screen, in the world's thumbs' place while
	// the controls are lent: a button for each control it is given, in the
	// holder's words, and one to leave. The buttons press the keys a
	// keyboard would, through the engine's own key path, so what reaches a
	// game is what the keys press. The four ways are the left thumb's, by
	// the cross or by the stick, as the person chose by the small switch
	// over them, kept by this browser, the cross first. The cross presses
	// the arrows under the thumb, two at a corner. The stick is the web
	// shell's, as on foot: the left thumb opens it anywhere on the left
	// half, and while no finger holds it a disc says where a thumb goes. A
	// and B stand large and apart under the right thumb, which rolls from
	// one to the other; leave and start sit small over them, start saying
	// what holding it does when the holder gives it something.
	//
	// The pad keeps to the foot of the screen held upright and to its two
	// sides held sideways, and what it leaves clear between is the game's:
	// the page hands that part to the engine, which lays the game in it as
	// large as it fits. With the cross, the whole glass is the pad's, so a
	// finger off it opens no stick.
	interface Props {
		/** What the holder is pressed by, each drawn once. */
		controls: readonly Control[];
		/** Whether a finger holds the stick, drawn where it is. */
		sticking: boolean;
		/** Reads a key of the holder's words; a key it lacks comes back as it went. */
		words?: (key: string) => string;
		/** Presses a key or lets it go, by the name the web gives it. */
		onkey: (code: string, down: boolean) => void;
		/** Hears the part of the stage the pad leaves clear, as it lays out, and null as it leaves. */
		onclear: (clear: Clear | null) => void;
	}

	let { controls, sticking, words, onkey, onclear }: Props = $props();

	const fingers = new Fingers((code, down) => onkey(code, down));
	$effect(() => fingers.listen());

	let ways = $state<Ways>(readWays(kept('ways')));
	function switchWays() {
		ways = ways === 'cross' ? 'stick' : 'cross';
		keep('ways', ways);
	}

	const steers = $derived(controls.includes('stick'));
	const buttons = $derived(controls.filter((control): control is keyof typeof PAD_KEYS => control !== 'stick'));

	const cross = fingers.steer((x, y, size) => crossed(x, y, size).map((way) => WAY_KEYS[way]));
	/** Each arm's cell of the cross, three across, and the arrow on it. */
	const ARMS: readonly { way: Way; cell: [number, number]; arrow: string }[] = [
		{ way: 'up', cell: [1, 0], arrow: 'M1.5 .3 1.75 .65H1.25Z' },
		{ way: 'left', cell: [0, 1], arrow: 'M.3 1.5 .65 1.25V1.75Z' },
		{ way: 'right', cell: [2, 1], arrow: 'M2.7 1.5 2.35 1.25V1.75Z' },
		{ way: 'down', cell: [1, 2], arrow: 'M1.5 2.7 1.75 2.35H1.25Z' }
	];

	// What the pad leaves clear, said each time it lays out again: turned,
	// resized, or the switch showing the other control.
	let pad = $state<HTMLElement>();
	let clear = $state<HTMLElement>();
	$effect(() => {
		if (!pad || !clear) return;
		const [outer, inner] = [pad, clear];
		const observer = new ResizeObserver(() => {
			const stage = outer.getBoundingClientRect();
			const part = inner.getBoundingClientRect();
			onclear([part.left - stage.left, part.top - stage.top, part.right - stage.left, part.bottom - stage.top]);
		});
		observer.observe(outer);
		observer.observe(inner);
		return () => {
			observer.disconnect();
			onclear(null);
		};
	});
</script>

<div class="pad" class:glass={steers && ways === 'cross'} role="group" aria-label={t('engine.controls')} bind:this={pad}>
	<div class="clear" bind:this={clear}></div>
	{#if steers}
		<div class="ways">
			<button
				type="button"
				class="switch"
				aria-label={t(ways === 'cross' ? 'engine.controls.toStick' : 'engine.controls.toCross')}
				onpointerdown={(event) => event.preventDefault()}
				onclick={switchWays}
				oncontextmenu={(event) => event.preventDefault()}
			>
				<Icon name={ways === 'cross' ? 'circle' : 'plus'} />
			</button>
			{#if ways === 'cross'}
				<div class="cross" role="img" aria-label={t('engine.controls.cross')} {...cross}>
					<svg viewBox="0 0 3 3" aria-hidden="true">
						<path class="shape" d="M1 0H2V1H3V2H2V3H1V2H0V1H1Z" />
						{#each ARMS as arm (arm.way)}
							{@const held = fingers.holds(WAY_KEYS[arm.way])}
							<rect class="arm" class:held x={arm.cell[0]} y={arm.cell[1]} width="1" height="1" />
							<path class="arrow" class:held d={arm.arrow} />
						{/each}
					</svg>
				</div>
			{:else}
				<span class="rest" class:away={sticking} role="img" aria-label={said('stick', words)}>
					<span class="knob"></span>
				</span>
			{/if}
		</div>
	{/if}
	<div class="buttons">
		<button type="button" class="leave" {...fingers.tap(LEAVE_KEY)}>
			<Icon name="log-out" />{t('engine.controls.leave')}
		</button>
		{#each buttons as control (control)}
			{@const does = held(control, words)}
			<button type="button" class={control} class:held={fingers.holds(PAD_KEYS[control])} {...fingers.hold(PAD_KEYS[control], control !== 'start')}>
				{said(control, words)}
				{#if does}<small class="does">{t('engine.controls.held', { does })}</small>{/if}
			</button>
		{/each}
	</div>
</div>

<style>
	/* Over the whole stage, a grid whose free cell is the game's: held
	   upright, the left thumb's control and the buttons along the foot,
	   as high as the world's thumbs, and the clear part over them;
	   sideways, the two thumbs' columns at the sides and the clear part
	   between, the screen's height. The clear part goes to the edges the
	   phone leaves safe; the controls keep a margin in. The pad takes no
	   finger but on its controls, so the stick's are the canvas's; with
	   the cross it takes them all. */
	.pad {
		position: absolute;
		inset: 0;
		z-index: var(--z-panel);
		display: grid;
		grid-template: 'clear clear' 1fr 'ways buttons' auto / 1fr 1fr;
		padding: env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left);
		pointer-events: none;
	}
	@media (orientation: landscape) {
		.pad {
			grid-template: 'ways clear buttons' 1fr / auto 1fr auto;
		}
	}
	.glass {
		pointer-events: auto;
		touch-action: none;
	}
	.clear {
		grid-area: clear;
	}
	.ways,
	.buttons {
		align-self: end;
		margin: var(--sp-4) var(--sp-4) calc(2 * var(--sp-5) + var(--control-h));
	}
	/* The switch over the control it switches, at its outer edge, where a
	   thumb landing on the control never goes. */
	.ways {
		grid-area: ways;
		display: grid;
		justify-items: start;
		gap: var(--sp-4);
	}
	/* A ring with a knob at its middle, as a held stick is drawn, but
	   quieter: a thumb goes here. Under a finger it gives way to the stick
	   where the finger is, and keeps its place. */
	.rest {
		display: grid;
		place-items: center;
		width: calc(2 * var(--thumb-h));
		height: calc(2 * var(--thumb-h));
		border: var(--bw) solid var(--on-world-quiet);
		border-radius: var(--radius-round);
		background: color-mix(in srgb, var(--on-world-shadow) 25%, transparent);
	}
	.rest.away {
		visibility: hidden;
	}
	.knob {
		width: var(--control-h);
		height: var(--control-h);
		border-radius: var(--radius-round);
		background: var(--on-world-quiet);
		box-shadow: 0 0 var(--sp-1) var(--on-world-shadow);
	}
	/* A plus three cells across, in the stick's colours: an arm held
	   lights as the held stick's knob does, its arrow dark on it. */
	.cross {
		width: calc(2 * var(--thumb-h));
		height: calc(2 * var(--thumb-h));
		pointer-events: auto;
		touch-action: none;
		-webkit-touch-callout: none;
	}
	svg {
		display: block;
		width: 100%;
		height: 100%;
		overflow: visible;
	}
	.shape {
		fill: color-mix(in srgb, var(--on-world-shadow) 25%, transparent);
		stroke: var(--on-world-quiet);
		stroke-width: var(--bw);
		vector-effect: non-scaling-stroke;
	}
	.arm {
		fill: transparent;
	}
	.arm.held {
		fill: var(--on-world);
	}
	.arrow {
		fill: var(--on-world-quiet);
	}
	.arrow.held {
		fill: var(--on-world-shadow);
	}
	/* B under the thumb's root, A beyond it; leave over B and start over A,
	   as small as a pad's middle buttons. */
	.buttons {
		grid-area: buttons;
		justify-self: end;
		display: grid;
		grid-template: 'leave start' auto 'sprint jump' auto / var(--pad-h) var(--pad-h);
		gap: var(--sp-4) var(--sp-5);
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
	button.held {
		background: var(--action);
		color: var(--action-text);
	}
	.switch {
		width: var(--control-h);
	}
	.leave {
		grid-area: leave;
	}
	.start {
		grid-area: start;
	}
	/* What holding it does, a quiet line under its name: the button grows
	   to hold it, its name where it was. */
	.start:has(.does) {
		flex-direction: column;
		gap: 0;
		height: auto;
		min-height: var(--control-h);
		padding: var(--sp-1) 0;
	}
	.does {
		font-size: var(--fs);
		font-weight: var(--fw-regular);
		line-height: 1.2;
		opacity: 0.8;
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
