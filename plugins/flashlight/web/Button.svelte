<script lang="ts">
	import { IconButton } from '$lib/ds';
	import type { LayerProps } from '$lib/plugins/plugin';
	import { t } from './words';

	// The flashlight's button: it switches it, as `L` does, pressed while
	// the body's hand grasps it. Whether it does, the plugin says by
	// `changed` whoever switched it, and by `state` when asked: what it was
	// before this button listened.
	let { seam }: LayerProps = $props();

	let on = $state(false);
	$effect(() => {
		const stop = seam.listen((kind, event) => {
			if (kind === 'changed' || kind === 'state') on = event.on === true;
		});
		seam.command('state');
		return stop;
	});
</script>

<IconButton icon="flashlight" label={t('name')} aria-pressed={on} onclick={() => seam.command(on ? 'off' : 'on')} />
