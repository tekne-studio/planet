<script lang="ts">
	import { IconButton } from '$lib/ds';
	import type { LayerProps } from '$lib/plugins/plugin';
	import { out } from './out.svelte';
	import { t } from './words';

	// The chat's button, on a touch screen alone, where an open bar covers
	// the world the thumbs walk: a tap takes the chat out, a tap puts it
	// away, and while it is away a mark says someone spoke since it was last
	// out. A screen with keys opens the chat with Enter and has no button.
	let { touch }: LayerProps = $props();
</script>

{#if touch}
	<IconButton
		icon="message-circle"
		label={out.unread ? t('unread') : t('title')}
		aria-expanded={out.shown}
		mark={out.unread}
		onclick={() => (out.shown = !out.shown)}
	/>
{/if}
