<script lang="ts" module>
	/** The cartridges met in a cabinet while the page lives, the newest first: a file of one's own stays in the list to be chosen again. */
	const met = $state<string[]>([]);
</script>

<script lang="ts">
	import { onMount } from 'svelte';
	import { Alert, Button, Field, Icon, IconButton, Select, Spinner } from '$lib/ds';
	import type { CartridgeCover } from '$lib/engine';
	import Cover from './Cover.svelte';
	import { games, type Game } from './games';
	import type { KindProps } from './kinds';
	import { parseHand } from './protocol';
	import { saveCover, saveTemplate } from './template';
	import { notTakenOrUnfit, takeFile, takeSkin, type NotTaken, type Unfit } from './takeFile';
	import { t } from './words';

	// The cabinet kind's own controls: the game it runs and the skin it
	// wears, a form row each, the label over what it holds. The game is a
	// select whose rows are covers, each a thumbnail read from its file
	// beside its title and its maker's mark, a line each: the games this
	// version ships, the files of one's own met since the page opened, then
	// none and a file of one's own, which opens the picker. Its trigger
	// shows the game in the cabinet, its thumbnail and title; under it, that
	// game's cover, large, and its mark. In hand, what the next cabinet carries: a game chosen places
	// one in front of the body at once, as a file dropped anywhere on the
	// page does (`Drop.svelte`), and a skin chosen waits for it. With a
	// cabinet picked in the world, that one's, set as each is chosen; a skin
	// dropped on the page goes onto it too. Beside the skin, the template it
	// is painted over, to download, and the game's cover, a reference to
	// paint from. What it carries is heard from the plugin, never kept here.
	let { seam, picked }: KindProps = $props();

	let hand = $state<{ cart: string | null; skin: string | null }>({ cart: null, skin: null });
	const cart = $derived(picked ? (picked.cart ?? null) : hand.cart);
	const skin = $derived(picked ? (picked.skin ?? null) : hand.skin);

	let shipped = $state<Game[]>([]);
	/** Each cartridge's cover by its file, once read; null for one that could not be. */
	let covers = $state.raw<Record<string, CartridgeCover | null>>({});
	let sending = $state(false);
	let refused = $state<NotTaken | Unfit | null>(null);
	let cartPicker: HTMLInputElement | undefined = $state();
	let skinPicker: HTMLInputElement | undefined = $state();

	// The games are read each time the panel opens, so one shipped while the
	// page is open is offered.
	onMount(() => void games().then((listed) => (shipped = listed)));

	$effect(() => {
		const stop = seam.listen((kind, event) => {
			if (kind !== 'hand') return;
			const said = parseHand(event);
			if (said) hand = { cart: said.cart, skin: said.skin };
		});
		seam.command('state');
		return stop;
	});

	/** A file's name as a person reads it: the start of its hash. */
	const short = (asset: string) => asset.slice('asset://'.length, 'asset://'.length + 8);

	/** The select's own values beside a game's name and a file's: no game, and a file of one's own. */
	const FILE = ':file';
	const NONE = ':none';

	$effect(() => {
		if (cart && !met.includes(cart)) met.unshift(cart);
	});

	/** Each game offered by its value: the games shipped by name, then the files met that no game shipped is, by their own. */
	const offered = $derived(
		new Map<string, { cart: string; title: string }>([
			...shipped.map((game): [string, { cart: string; title: string }] => [game.name, { cart: game.cart, title: game.title }]),
			...met
				.filter((file) => !shipped.some((game) => game.cart === file))
				.map((file): [string, { cart: string; title: string }] => [file, { cart: file, title: t('cabinet.cart.file', { name: short(file) }) }])
		])
	);
	const chosen = $derived(shipped.find((game) => game.cart === cart)?.name ?? cart ?? NONE);
	const options = $derived([
		...[...offered].map(([value, game]) => ({ value, label: covers[game.cart]?.title ?? game.title })),
		{ value: NONE, label: t('cabinet.cart.none'), rule: true },
		{ value: FILE, label: t('cabinet.cart.load') }
	]);

	/** The covers asked for, each once while the panel lives, read or not. */
	const asked = new Set<string>();
	$effect(() => {
		for (const { cart: file } of offered.values()) {
			if (asked.has(file)) continue;
			asked.add(file);
			void seam.cover(file).then((cover) => (covers = { ...covers, [file]: cover }));
		}
	});

	/** The select calls it only for another option than the one chosen. */
	function select(value: string) {
		if (value === FILE) cartPicker?.click();
		else if (value === NONE) give('cart', '');
		else give('cart', value);
	}

	/** How wide the large cover may be: the panel's width. */
	let wide = $state(0);

	/** The mark under a title: the year and the maker its cover says, none for a file with no cover. */
	function mark(cover: CartridgeCover | null | undefined): string | null {
		return cover?.maker && cover.year ? t('cabinet.cart.mark', { year: String(cover.year), maker: cover.maker }) : null;
	}

	/** Gives the cabinet picked, or the hand, a cartridge: a game by its name, or none with `""`. */
	function give(as: 'cart' | 'skin', name: string) {
		refused = null;
		if (picked) seam.command('set', { [as]: name });
		// A game chosen in hand places its cabinet, as a file chosen does.
		else seam.command('take', { item: 'cabinet', [as]: name, ...(as === 'cart' && name ? { ahead: true } : {}) });
	}

	async function send(file: File, as: 'cart' | 'skin') {
		if (sending) return;
		refused = null;
		sending = true;
		try {
			if (as === 'cart') await takeFile(seam, file, picked);
			else await takeSkin(seam, file, picked);
		} catch (error) {
			refused = notTakenOrUnfit(error);
		} finally {
			sending = false;
		}
	}

	function fileChosen(event: Event, as: 'cart' | 'skin') {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = '';
		if (file) void send(file, as);
	}
</script>

<div class="cabinet" role="group" aria-label={t('kind.cabinet')}>
	{#if sending}
		<Spinner label={t('cabinet.sending')} />
	{:else}
		{#if !picked}<p>{t('cabinet.drop')}</p>{/if}
		<Select label={t('cabinet.cart')} stacked {options} value={chosen} onselect={select}>
			{#snippet option(one, listed)}
				{@const game = offered.get(one.value)}
				{#if game}
					{@const cover = covers[game.cart]}
					{@const said = mark(cover)}
					<span class="row">
						<Cover picture={cover?.picture} fit={80} />
						<span class="lines">
							<span class="title">{one.label}</span>
							<!-- On the trigger, the credit is the large cover's, under it. -->
							{#if listed && said}<span>{said}</span>{/if}
						</span>
					</span>
				{:else}
					<span class="row plain">
						{#if one.value === FILE}<Icon name="plus" />{/if}
						<span>{one.label}</span>
					</span>
				{/if}
			{/snippet}
		</Select>
		{#if cart}
			{@const said = mark(covers[cart])}
			<div class="chosen" bind:clientWidth={wide}>
				<Cover picture={covers[cart]?.picture} fit={wide} />
				{#if said}<span>{said}</span>{/if}
				<Button variant="ghost" onclick={() => give('cart', '')}>{t('cabinet.clear')}</Button>
			</div>
		{/if}
		<Field label={t('cabinet.skin')} hint={t('cabinet.skin.hint')}>
			<p>{skin ? t('cabinet.skin.file', { name: short(skin) }) : t('cabinet.skin.none')}</p>
			<div class="buttons">
				<Button variant="ghost" onclick={() => skinPicker?.click()}>{t('cabinet.skin.choose')}</Button>
				{#if skin}<Button variant="ghost" onclick={() => give('skin', '')}>{t('cabinet.clear')}</Button>{/if}
				<IconButton icon="download" label={t('cabinet.template')} onclick={() => void saveTemplate(seam)} />
				{#if cart && covers[cart]}
					{@const cover = covers[cart]}
					<IconButton icon="image" label={t('cabinet.cover.save')} onclick={() => void saveCover(cover, cover.title ?? short(cart))} />
				{/if}
			</div>
		</Field>
		<input bind:this={cartPicker} type="file" accept=".wasm,application/wasm" hidden onchange={(event) => fileChosen(event, 'cart')} />
		<input bind:this={skinPicker} type="file" accept="image/png,image/jpeg,image/webp,.png,.jpg,.jpeg,.webp" hidden onchange={(event) => fileChosen(event, 'skin')} />
	{/if}
	{#if refused}
		<Alert variant="info">{t(`drop.refused.${refused}`)}</Alert>
	{/if}
</div>

<style>
	.cabinet {
		display: flex;
		flex-direction: column;
		gap: var(--sp-4);
		color: var(--text-muted);
	}
	.row {
		display: flex;
		align-items: center;
		gap: var(--sp-3);
	}
	.plain {
		min-height: var(--control-h);
	}
	.lines {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-width: 0;
	}
	/* A title and its credit, a line each: cut short only when they must. */
	.lines > span {
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
	}
	/* Bold, its capitals unspaced: a title of 16 fits beside the thumbnail. */
	.title {
		font-weight: var(--fw-bold);
	}
	.chosen {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--sp-3);
	}
	.buttons {
		display: flex;
		flex-wrap: wrap;
		gap: var(--sp-2);
	}
</style>
