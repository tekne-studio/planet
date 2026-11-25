<script lang="ts">
	// A cartridge's cover, pixel exact: as wide as `fit` allows in whole
	// device pixels a game's pixel, never smoothed; under one device pixel a
	// pixel, as wide as `fit`. The inset ground while it is read.
	let { picture, fit }: { picture: ImageData | null | undefined; fit: number } = $props();

	let canvas: HTMLCanvasElement | undefined = $state();

	/** The cover's own size: a cartridge's screen's. */
	const SIZE = [160, 120] as const;

	const width = $derived.by(() => {
		const ratio = typeof devicePixelRatio === 'number' ? devicePixelRatio : 1;
		const whole = Math.floor((fit * ratio) / SIZE[0]);
		return whole >= 1 ? (whole * SIZE[0]) / ratio : fit;
	});

	$effect(() => {
		if (!canvas || !picture) return;
		canvas.width = picture.width;
		canvas.height = picture.height;
		canvas.getContext('2d')?.putImageData(picture, 0, 0);
	});
</script>

<canvas bind:this={canvas} style:width="{width}px" style:height="{(width * SIZE[1]) / SIZE[0]}px" aria-hidden="true"></canvas>

<style>
	canvas {
		display: block;
		flex: none;
		image-rendering: pixelated;
		background: var(--bg-inset);
	}
</style>
