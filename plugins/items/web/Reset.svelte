<script lang="ts">
	import { Dial } from '$lib/ds';
	import type { Resetting } from './protocol';
	import { t } from './words';

	// Start held at a cabinet: over the middle of the game, a ring that
	// fills until the game starts over, its cover shown between. The engine
	// says where the game is every frame while Start is held past a tap;
	// this only draws there. At a game of several pads both players see it
	// fill, whoever holds it.
	interface Props {
		resetting: Resetting;
	}

	let { resetting }: Props = $props();
</script>

<div class="reset" aria-hidden="true">
	<div class="over" style:left="{resetting.x * 100}%" style:top="{resetting.y * 100}%">
		<Dial glyph="rotate-ccw" closed={resetting.closed} line={t('control.start.hold')} />
	</div>
</div>

<style>
	.reset {
		position: absolute;
		inset: 0;
		overflow: hidden;
		pointer-events: none;
	}
	.over {
		position: absolute;
		transform: translate(-50%, -50%);
	}
</style>
