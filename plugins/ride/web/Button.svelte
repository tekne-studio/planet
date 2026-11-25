<script lang="ts">
	import { IconButton } from '$lib/ds';
	import type { LayerProps } from '$lib/plugins/plugin';
	import { t } from './words';

	// The board's button: it mounts the board and leaves it, as `H` does
	// and an agent's `ride.mount` and `ride.leave`, pressed while the body
	// rides it, as the core's mode says.
	let { seam, mode }: LayerProps = $props();

	const on = $derived(mode === 'ride.board');
</script>

<IconButton icon="hoverboard" label={t('board')} aria-pressed={on} onclick={() => seam.command(on ? 'leave' : 'mount')} />
