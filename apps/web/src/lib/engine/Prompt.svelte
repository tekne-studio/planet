<script lang="ts">
	import { fade, scale } from 'svelte/transition';
	import { WorldCard } from '$lib/ds';
	import type { WebPlugin } from '$lib/plugins/plugin';
	import type { Prompt, PromptAt } from './protocol';

	// What is in front of the body, one manner for every plugin: a card with
	// the key, the thing's name and what a tap and a hold do, in the
	// offering plugin's own words. A thing in front has it floating by it,
	// just over its top where the engine says, so card and thing read as
	// one; it eases in as the thing becomes the choice and out as it stops
	// being it, each where it stood, so two things in front never pop. With
	// nothing in front, what the body is on is offered by the same card low
	// in the middle, "Board [E] Leave"; on a touch screen under the left
	// thumb, a row over the readings a plugin shows there, clear of the
	// thumbs' buttons.
	//
	// A press on the card is E going down, and letting go is E coming up,
	// through the engine's own key path: the engine times it into a tap or
	// a hold and says the ring closing. The press never takes the focus, so
	// the keys stay the world's. A touch screen has no key: the card is it,
	// and a long press holds.
	interface Props {
		prompt: Prompt | null;
		at: PromptAt | null;
		/** How far the ring has closed, while a hold is held. */
		ring: number | null;
		/** Counts the taps that acted, each a sweep of the ring. */
		taps: number;
		/** Counts the holds that closed. */
		pulse: number;
		/** The plugins mounted: an offer is read in its own plugin's words. */
		plugins: readonly WebPlugin[];
		touch: boolean;
		/** Presses the prompt's key or lets it go. */
		onkey: (down: boolean) => void;
	}

	let { prompt, at, ring, taps, pulse, plugins, touch, onkey }: Props = $props();

	let width = $state(0);
	let height = $state(0);
	/** The card hung by a thing, as it was last laid out. */
	let cardW = $state(0);
	let cardH = $state(0);

	const words = $derived(prompt && plugins.find((plugin) => plugin.name === prompt.plugin)?.words);

	/** The gap between a thing's top and its card, and the edges it keeps in, pixels. */
	const GAP = 8;
	const EDGE = { side: 12, top: 56, bottom: 96 };
	/** How far clear of the body the card must be to come back over the thing. */
	const CLEAR = 24;

	type Side = 'over' | 'left' | 'right';
	type Rect = [number, number, number, number];

	/**
	 * Where the card goes for each thing: over it, or at the side away from
	 * the body once over it would cover the body. Kept between frames, not
	 * state: a side stays while the body stands there, so the card never
	 * flips as the body shifts.
	 */
	let sided: { key: string; side: Side } | null = null;

	const meets = ([l, t, r, b]: Rect, [L, T, R, B]: Rect, margin = 0) => l < R + margin && r > L - margin && t < B + margin && b > T - margin;

	/** The card's offset from the point, pixels, for a side. */
	function offset(side: Side): [number, number] {
		if (side === 'over') return [-cardW / 2, -GAP - cardH];
		return [side === 'right' ? GAP : -GAP - cardW, -cardH / 2];
	}

	function place(key: string, point: [number, number], body: Rect | null): Side {
		const rect = (side: Side): Rect => {
			const [dx, dy] = offset(side);
			return [point[0] + dx, point[1] + dy, point[0] + dx + cardW, point[1] + dy + cardH];
		};
		const fits = ([l, , r]: Rect) => l >= EDGE.side && r <= width - EDGE.side;
		// Away from the body first; the other side when that one is off the
		// screen or on the body; over the thing when neither will do.
		const aside = (body: Rect): Side => {
			const away: Side = (body[0] + body[2]) / 2 < point[0] ? 'right' : 'left';
			const other: Side = away === 'right' ? 'left' : 'right';
			const clear = (side: Side) => fits(rect(side)) && !meets(rect(side), body);
			return clear(away) ? away : clear(other) ? other : 'over';
		};
		let side: Side = sided?.key === key ? sided.side : 'over';
		if (!body) side = 'over';
		else if (side === 'over') side = meets(rect('over'), body) ? aside(body) : 'over';
		else if (!meets(rect('over'), body, CLEAR)) side = 'over';
		else if (meets(rect(side), body) || !fits(rect(side))) side = aside(body);
		sided = { key, side };
		return side;
	}

	/** A thing in front, while the engine says where: one at a time, keyed so the last eases out where it was. */
	const floating = $derived.by(() => {
		if (prompt?.on !== 'point' || !at || !words) return [];
		const key = `${prompt.plugin}:${prompt.id}`;
		const point: [number, number] = [at.x * width, at.y * height];
		const body: Rect | null = at.body && [at.body[0] * width, at.body[1] * height, at.body[2] * width, at.body[3] * height];
		const [dx, dy] = offset(place(key, point, body));
		// Held inside the screen.
		const x = Math.min(Math.max(point[0] + dx, EDGE.side), width - EDGE.side - cardW) - point[0];
		const y = Math.min(Math.max(point[1] + dy, EDGE.top), height - EDGE.bottom - cardH) - point[1];
		return [{ key, prompt, point, shift: [x, y], words }];
	});
	const bar = $derived(prompt?.on === 'body' && words ? { prompt, words } : null);

	const EASE_MS = 120;

	const press = {
		onpointerdown(event: PointerEvent) {
			event.preventDefault();
			(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
			onkey(true);
		},
		onpointerup: () => onkey(false),
		onpointercancel: () => onkey(false),
		oncontextmenu: (event: Event) => event.preventDefault()
	};
</script>

{#snippet card(prompt: Prompt, words: (key: string) => string)}
	<WorldCard
		key={touch ? undefined : 'E'}
		title={words(prompt.thing)}
		action={prompt.tap && words(prompt.tap)}
		hold={prompt.hold && words(prompt.hold.verb)}
		closed={ring}
		{taps}
		{pulse}
		{...press}
	/>
{/snippet}

<div class="prompts" bind:clientWidth={width} bind:clientHeight={height}>
	{#each floating as item (item.key)}
		<!-- The point follows the thing each frame; the card's side eases. -->
		<div class="at" style:left="{item.point[0]}px" style:top="{item.point[1]}px">
			<div class="side" style:translate="{item.shift[0]}px {item.shift[1]}px">
				<div class="card" bind:clientWidth={cardW} bind:clientHeight={cardH} transition:scale={{ duration: EASE_MS, start: 0.9, opacity: 0 }}>
					{@render card(item.prompt, item.words)}
				</div>
			</div>
		</div>
	{/each}

	{#if bar}
		<div class="bar" class:touch transition:fade={{ duration: EASE_MS }}>
			{@render card(bar.prompt, bar.words)}
		</div>
	{/if}
</div>

<style>
	.prompts {
		position: absolute;
		inset: 0;
		z-index: var(--z-panel);
		overflow: hidden;
		pointer-events: none;
	}
	.prompts :global(button) {
		pointer-events: auto;
	}
	.at {
		position: absolute;
	}
	/* The card's top left, from the point: eased when it changes side,
	   never when the thing moves. */
	.side {
		position: absolute;
		left: 0;
		top: 0;
		transition: translate 160ms var(--ease-out);
	}
	.card {
		transform-origin: bottom center;
	}
	.bar {
		position: absolute;
		left: 50%;
		bottom: calc(2 * var(--sp-5) + var(--control-h));
		translate: -50% 0;
	}
	.bar.touch {
		left: var(--sp-4);
		bottom: calc(var(--thumb-home) + var(--control-h) + var(--sp-4));
		translate: none;
	}
</style>
