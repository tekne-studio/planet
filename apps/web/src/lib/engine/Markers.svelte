<script lang="ts">
	import { fade } from 'svelte/transition';
	import { Dialog, Marker as Chip, MarkerCard, type IconName } from '$lib/ds';
	import { fill, isMessageKey, t } from '$lib/i18n';
	import type { WebPlugin } from '$lib/plugins/plugin';
	import type { Command, Marker, MarkerAt, MarkerChoice, PluginCommand } from './protocol';

	// What the givers point out, one manner for every giver: a chip
	// (`$lib/ds` Marker) over the thing while it is in view, sliding along the
	// screen's edge on the side it lies while it is not, its arrow turned
	// toward it. Where the thing's nametag already hangs, nothing more is
	// drawn. The engine says where each is every frame; this reads the
	// protocol in the giver's words and places the chips there.
	//
	// Pressed, a chip opens its card (`$lib/ds` MarkerCard) where it was, with
	// what its giver offers; a choice sends its command as it is. One that
	// cannot be undone asks first in a Dialog that says plainly what it does.
	// In play the pointer is the world's: Escape frees it, then a click; on
	// a touch screen, a tap.
	interface Props {
		markers: Marker[];
		at: MarkerAt[];
		/** The plugins mounted: a marker is read in its own giver's words. */
		plugins: readonly WebPlugin[];
		oncommand: (command: Command | PluginCommand) => void;
	}

	let { markers, at, plugins, oncommand }: Props = $props();

	let width = $state(0);
	let height = $state(0);

	/** The edges a chip keeps in from, pixels: clear of the bar on top and the prompt and help below. */
	const EDGE = { side: 16, top: 64, bottom: 104 };

	const key = (marker: { giver: string; id: number }) => `${marker.giver}:${marker.id}`;

	const byKey = $derived(new Map(markers.map((marker) => [key(marker), marker])));

	/**
	 * A giver's words: a plugin's own, or for an owner of the core (`parts`) the page's, `engine.marker.<giver>.<word>`.
	 * A word neither holds is said as it is.
	 */
	function words(giver: string): (word: string) => string {
		const plugin = plugins.find((mounted) => mounted.name === giver)?.words;
		if (plugin) return plugin;
		return (word) => {
			const core = `engine.marker.${giver}.${word}`;
			return isMessageKey(core) ? t(core) : word;
		};
	}

	function name(marker: Marker): string {
		return marker.label || words(marker.giver)(marker.thing);
	}

	function far(distance_m: number): string {
		return distance_m < 1000 ? t('engine.stats.metres', { n: distance_m.toFixed(0) }) : t('engine.marker.km', { n: (distance_m / 1000).toFixed(1) });
	}

	const GLYPHS: ReadonlySet<string> = new Set<IconName>(['paw-print', 'map-pin', 'house', 'star', 'users']);
	const glyph = (marker: Marker): IconName => (GLYPHS.has(marker.glyph) ? (marker.glyph as IconName) : 'map-pin');

	const clamp = (value: number, low: number, high: number) => Math.min(Math.max(value, low), Math.max(low, high));

	/**
	 * Each chip shown: its marker, and where it lands in pixels, kept inside the edges. Across, a chip hangs from its
	 * point by the share of the view it stands at, in view and on the edge alike, so it never leaves the view and
	 * turning into an edge chip moves it no more than its height.
	 */
	const shown = $derived(
		at.flatMap((where) => {
			const marker = byKey.get(key(where));
			if (!marker || where.tagged) return [];
			const edge = where.edge_deg !== undefined;
			const x = clamp(where.x * width, EDGE.side, width - EDGE.side);
			const y = edge ? clamp(where.y * height, EDGE.top, height - EDGE.bottom) : Math.min(where.y * height, height - EDGE.bottom);
			return [{ key: key(where), marker, where, x, y, edge }];
		})
	);

	/** The card open: whose, and where its chip was when pressed, so it holds still under the pointer. */
	let open = $state<{ key: string; x: number; y: number } | null>(null);
	/** What the Dialog asks about, said as it was pressed: the command, the marker's name, the verb and what it does. */
	let asking = $state<{ command: MarkerChoice['command']; name: string; verb: string; consequence: string } | null>(null);
	let askingOpen = $state(false);

	const opened = $derived(open === null ? null : (shown.find((chip) => chip.key === open?.key) ?? null));

	function press(chip: (typeof shown)[number]) {
		open = open?.key === chip.key ? null : { key: chip.key, x: chip.x, y: chip.y };
	}

	function choose(marker: Marker, choice: MarkerChoice) {
		if (choice.confirm) {
			const said = words(marker.giver);
			const named = name(marker);
			asking = { command: choice.command, name: named, verb: said(choice.verb), consequence: fill(said(choice.confirm), { name: named }) };
			askingOpen = true;
			return;
		}
		oncommand(choice.command);
		close();
	}

	function close() {
		askingOpen = false;
		open = null;
	}

	// A card whose marker leaves closes with it, and its Dialog.
	$effect(() => {
		if (open !== null && !byKey.has(open.key)) close();
	});

	// Heard before the stage's own Escape, which closes a panel: an Escape
	// that closes a card says so, and the panel stays. While the Dialog asks,
	// its own Escape closes it alone.
	function onkeydown(event: KeyboardEvent) {
		if (event.key === 'Escape' && open !== null && !askingOpen && !event.defaultPrevented) {
			event.preventDefault();
			close();
		}
	}
</script>

<svelte:window onkeydowncapture={onkeydown} />

<div class="markers" bind:clientWidth={width} bind:clientHeight={height}>
	{#each shown as chip (chip.key)}
		{@const said = words(chip.marker.giver)}
		<!-- Placement: where it hangs, and the share of itself it hangs by, eased between in view and the edge. -->
		<div
			class="place"
			class:edge={chip.edge}
			style:left="{chip.x}px"
			style:top={chip.edge ? `${chip.y}px` : `max(${chip.y}px, calc(${EDGE.top}px + var(--control-h) + var(--sp-2)))`}
			style:--ax="{(-chip.x / Math.max(width, 1)) * 100}%"
			style:--ay="{(-chip.y / Math.max(height, 1)) * 100}%"
			transition:fade={{ duration: 150 }}
		>
			<Chip
				glyph={glyph(chip.marker)}
				name={name(chip.marker)}
				note={chip.marker.note ? said(chip.marker.note) : undefined}
				distance={far(chip.where.distance_m)}
				arrow={chip.where.edge_deg}
				open={opened === chip}
				onclick={() => press(chip)}
			/>
		</div>
	{/each}
	{#if opened && open}
		{@const marker = opened.marker}
		{@const said = words(marker.giver)}
		<div
			class="place card"
			style:left="{open.x}px"
			style:top="{open.y}px"
			style:--dx={open.x > width / 2 ? '-100%' : '0%'}
			style:--dy={open.y > height / 2 ? 'calc(-100% - var(--sp-6))' : 'var(--sp-6)'}
			transition:fade={{ duration: 120 }}
		>
			<MarkerCard
				glyph={glyph(marker)}
				name={name(marker)}
				note={marker.note ? said(marker.note) : undefined}
				distance={far(opened.where.distance_m)}
				choices={marker.choices.map((choice) => ({ label: said(choice.verb), onclick: () => choose(marker, choice) }))}
			/>
		</div>
	{/if}
</div>

{#if asking}
	{@const { command, verb, consequence } = asking}
	<Dialog
		bind:open={askingOpen}
		title={asking.name}
		confirmLabel={verb}
		danger
		onconfirm={() => {
			oncommand(command);
			close();
		}}
	>
		<p>{consequence}</p>
	</Dialog>
{/if}

<style>
	.markers {
		position: absolute;
		inset: 0;
		overflow: hidden;
		pointer-events: none;
	}
	.place {
		position: absolute;
		display: flex;
		pointer-events: auto;
		/* Over the thing, a little above it, held inside the view across; on
		   the edge, held inside the view from the point it lies at: at the
		   left edge it hangs to the right of it, at the bottom above it. The
		   switch between the two eases. */
		transform: translate(var(--ax), calc(-100% - var(--sp-2)));
		transition: transform var(--dur-fast) var(--ease-out);
	}
	.place.edge {
		transform: translate(var(--ax), var(--ay));
	}
	/* The card opens where its chip was pressed, toward the middle of the view. */
	.place.card {
		transform: translate(var(--dx), var(--dy));
		transition: none;
	}
</style>
