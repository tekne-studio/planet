<script lang="ts">
	import { Button, Icon, IconButton, Panel, type IconName } from '$lib/ds';
	import { t } from '$lib/i18n';
	import type { WebPlugin } from '$lib/plugins/plugin';

	// The keys, behind a button in the bottom left corner: read once, then
	// out of the way of the world. It opens in the middle of the screen, the
	// core's keys and then each plugin that is on in this world as a group of
	// its own, flowing into as many columns as the width holds, three at
	// most. On a touch screen it says what the fingers and the buttons do
	// instead, in one column under the bar, whose button opens it: the bottom
	// left is the stick's. Whether it is open is the stage's, which keeps one
	// panel open at a time and closes it on Escape; the button here asks the
	// stage.
	interface Props {
		/** The plugins on in this world; those with keys add a group each. */
		plugins?: readonly WebPlugin[];
		/** `V` browses the avatars on offer, unless the account wears its own. */
		browse?: boolean;
		/** A touch screen: the panel alone, under the bar. */
		touch?: boolean;
		open?: boolean;
		onopen?: () => void;
		onclose?: () => void;
	}

	let { plugins = [], browse = true, touch = false, open = false, onopen, onclose }: Props = $props();

	type Line = { keys?: string; icon?: IconName; does: string };

	const keys = $derived<Line[]>([
		{ keys: t('engine.hint.look.keys'), does: t('engine.hint.look') },
		{ keys: 'W A S D', does: t('engine.hint.move') },
		{ keys: t('engine.hint.up.keys'), does: t('engine.hint.up') },
		{ keys: 'C', does: t('engine.hint.down') },
		{ keys: 'Shift', does: t('engine.hint.sprint') },
		{ keys: 'F', does: t('engine.hint.mode') },
		...(browse ? [{ keys: 'V', does: t('engine.hint.avatar') }] : []),
		{ keys: 'E', does: t('engine.hint.act') },
		{ keys: 'P', does: t('engine.hint.eye') },
		{ keys: 'Q', does: t('engine.hint.whistle') },
		{ keys: t('engine.hint.undo.keys'), does: t('engine.hint.undo') },
		{ keys: t('engine.hint.zoom.keys'), does: t('engine.hint.zoom') },
		{ keys: 'Esc', does: t('engine.hint.release') }
	]);

	const groups = $derived(
		plugins
			.filter((plugin) => plugin.hints?.length)
			.map((plugin) => ({
				name: plugin.name,
				label: plugin.label(),
				lines: (plugin.hints ?? []).map((hint) => ({ keys: typeof hint.keys === 'string' ? hint.keys : hint.keys(), does: hint.does() }))
			}))
	);

	// A plugin's hints are keys, which a touch screen has none of.
	const fingers = $derived<Line[]>([
		{ keys: t('engine.hint.move.touch'), does: t('engine.hint.move') },
		{ keys: t('engine.hint.sprint.touch'), does: t('engine.hint.sprint') },
		{ keys: t('engine.hint.look.touch'), does: t('engine.hint.look') },
		{ keys: t('engine.hint.zoom.touch'), does: t('engine.hint.zoom') },
		{ icon: 'chevrons-up', does: t('engine.hint.up') },
		{ icon: 'chevrons-down', does: t('engine.hint.down') },
		{ icon: 'plane', does: t('engine.hint.mode') },
		{ icon: 'eye', does: t('engine.hint.eye') }
	]);
</script>

{#snippet hints(lines: Line[])}
	<ul class="hints">
		{#each lines as line (line.does)}
			<li><kbd>{#if line.icon}<Icon name={line.icon} />{:else}{line.keys}{/if}</kbd> {line.does}</li>
		{/each}
	</ul>
{/snippet}

{#if touch}
	{#if open}
		<Panel title={t('engine.help')} corner="top-right" under>
			{#snippet aside()}
				<IconButton icon="x" label={t('common.close')} onclick={onclose} />
			{/snippet}
			{@render hints(fingers)}
		</Panel>
	{/if}
{:else if open}
	<Panel title={t('engine.help')} corner="middle">
		{#snippet aside()}
			<button class="close" type="button" onclick={onclose}>{t('common.close')}</button>
		{/snippet}
		<div class="groups">
			{@render hints(keys)}
			{#each groups as group (group.name)}
				<section class="group">
					<h3>{group.label}</h3>
					{@render hints(group.lines)}
				</section>
			{/each}
		</div>
	</Panel>
{:else}
	<div class="toggle">
		<Button variant="ghost" type="button" aria-expanded="false" onclick={onopen}>{t('engine.help')}</Button>
	</div>
{/if}

<style>
	.toggle {
		position: absolute;
		bottom: var(--sp-5);
		left: var(--sp-4);
		z-index: var(--z-panel);
		background: var(--bg-overlay);
	}
	.close {
		border: none;
		background: none;
		color: var(--text-muted);
		letter-spacing: var(--ls-caps);
		text-transform: uppercase;
		cursor: pointer;
	}
	.close:hover {
		color: var(--text);
	}
	/* Columns as wide as a corner panel, as many as fit, three at most: the
	   browser balances them, and each group stays whole in one. */
	.groups {
		columns: 3 var(--panel-w);
		column-gap: var(--sp-6);
	}
	.groups > * {
		break-inside: avoid;
	}
	.group {
		margin-top: var(--sp-4);
	}
	h3 {
		margin-bottom: var(--sp-2);
		letter-spacing: var(--ls-caps);
		text-transform: uppercase;
	}
	.hints {
		display: flex;
		flex-direction: column;
		gap: var(--sp-1);
		color: var(--text-muted);
	}
	kbd {
		color: var(--text);
	}
</style>
