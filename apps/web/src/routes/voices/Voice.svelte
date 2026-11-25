<script lang="ts">
	import { Button, Input } from '$lib/ds';
	import type { BoardVoice } from '$lib/engine';

	// One voice on the board: its wave for the seed in the field, played on
	// a press. Next plays the following seed, as a strike in the world turns
	// through them.

	interface Props {
		voice: BoardVoice;
		variants: number;
		render: (name: string, seed: number) => Float32Array;
		play: (samples: Float32Array) => void;
	}

	let { voice, variants, render, play }: Props = $props();

	let typed = $state('0');
	let canvas = $state<HTMLCanvasElement>();
	const seed = $derived(Math.max(0, Math.floor(Number(typed)) || 0) >>> 0);
	const samples = $derived(render(voice.name, seed));

	$effect(() => {
		if (canvas) draw(canvas, samples);
	});

	/** The wave as the loudest and the quietest sample under each column of pixels. */
	function draw(canvas: HTMLCanvasElement, samples: Float32Array) {
		const ratio = window.devicePixelRatio;
		const width = Math.max(1, Math.round(canvas.clientWidth * ratio));
		const height = Math.max(1, Math.round(canvas.clientHeight * ratio));
		canvas.width = width;
		canvas.height = height;
		const pen = canvas.getContext('2d');
		if (!pen) return;
		pen.clearRect(0, 0, width, height);
		pen.fillStyle = getComputedStyle(canvas).color;
		const middle = height / 2;
		const perColumn = samples.length / width;
		for (let x = 0; x < width; x++) {
			const from = Math.floor(x * perColumn);
			const to = Math.min(samples.length, Math.max(from + 1, Math.floor((x + 1) * perColumn)));
			let low = 0;
			let high = 0;
			for (let i = from; i < to; i++) {
				low = Math.min(low, samples[i]);
				high = Math.max(high, samples[i]);
			}
			pen.fillRect(x, middle - high * middle, 1, Math.max(1, (high - low) * middle));
		}
	}

	function next() {
		typed = String((seed + 1) % variants);
		play(samples);
	}
</script>

<div class="voice">
	<div class="name">
		<span class="strong">{voice.name}</span>
		<span class="muted">{voice.seconds.toFixed(2)} s</span>
	</div>
	<canvas bind:this={canvas} class="wave" aria-hidden="true"></canvas>
	<div class="controls">
		<label class="seed">
			<span class="muted">Seed</span>
			<Input type="number" min="0" bind:value={typed} />
		</label>
		<Button onclick={() => play(samples)}>Play</Button>
		<Button variant="ghost" onclick={next}>Next</Button>
	</div>
</div>

<style>
	.voice {
		display: grid;
		grid-template-columns: calc(4 * var(--sp-8)) 1fr auto;
		align-items: center;
		gap: var(--sp-4);
		padding: var(--sp-3) 0;
		border-bottom: var(--bw) solid var(--border);
	}
	.name {
		display: flex;
		flex-direction: column;
		gap: var(--sp-1);
	}
	.wave {
		width: 100%;
		height: var(--sp-8);
		color: var(--text);
		background: var(--bg-inset);
	}
	.controls {
		display: flex;
		align-items: center;
		gap: var(--sp-2);
	}
	.seed {
		display: flex;
		align-items: center;
		gap: var(--sp-2);
		width: calc(2 * var(--sp-8));
	}
	.strong {
		font-weight: var(--fw-bold);
	}
	.muted {
		color: var(--text-muted);
	}
	@media (max-width: 640px) {
		.voice {
			grid-template-columns: 1fr;
		}
	}
</style>
