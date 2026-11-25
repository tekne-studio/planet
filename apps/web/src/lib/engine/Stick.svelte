<script lang="ts">
	import type { Stick } from './index';

	// The stick a finger holds on a touch screen, drawn where the web shell
	// says it is: its reach round the centre and the knob under the finger.
	// The shell reads the fingers; this only draws, over the world and
	// taking nothing, so the finger stays the canvas's. One stick, on foot
	// and on a holder's pad alike.
	let { stick }: { stick: Stick } = $props();
</script>

<div class="stick" aria-hidden="true">
	<span class="reach" style:left="{stick.center[0]}px" style:top="{stick.center[1]}px" style:--reach="{stick.reach}px"></span>
	<span class="knob" style:left="{stick.at[0]}px" style:top="{stick.at[1]}px"></span>
</div>

<style>
	/* Round, since the reach is the same every way. */
	.stick {
		position: absolute;
		inset: 0;
		z-index: var(--z-panel);
		pointer-events: none;
	}
	.reach,
	.knob {
		position: absolute;
		border-radius: 50%;
		transform: translate(-50%, -50%);
	}
	.reach {
		width: calc(2 * var(--reach));
		height: calc(2 * var(--reach));
		border: var(--bw) solid var(--on-world);
		background: color-mix(in srgb, var(--on-world-shadow) 25%, transparent);
	}
	.knob {
		width: var(--control-h);
		height: var(--control-h);
		background: var(--on-world);
		box-shadow: 0 0 var(--sp-1) var(--on-world-shadow);
	}
</style>
