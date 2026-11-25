<script lang="ts" generics="T extends string">
	// The sections of a large panel, one shown at a time: a list down its
	// side, and a row across its top on a phone, which scrolls sideways when
	// the names outrun the width. The arrow keys move between them, as the
	// platform's tabs do. The section shown is the caller's, in an element
	// with `role="tabpanel"` and the id `panelId(value)` gives.
	interface Option {
		value: T;
		label: string;
	}
	interface Props {
		options: readonly Option[];
		value: T;
		/** Accessible name of the list. */
		label: string;
		/** What ties each tab to its section: the caller's prefix for both ids. */
		id: string;
		onselect?: (value: T) => void;
	}

	let { options, value, label, id, onselect }: Props = $props();

	let list: HTMLElement | undefined = $state();

	const step: Record<string, number> = { ArrowDown: 1, ArrowRight: 1, ArrowUp: -1, ArrowLeft: -1 };

	function key(event: KeyboardEvent) {
		const by = step[event.key];
		const at = options.findIndex((option) => option.value === value);
		const next = by
			? options[(at + by + options.length) % options.length]
			: event.key === 'Home'
				? options[0]
				: event.key === 'End'
					? options[options.length - 1]
					: undefined;
		if (!next) return;
		event.preventDefault();
		onselect?.(next.value);
		requestAnimationFrame(() => list?.querySelector<HTMLElement>('[aria-selected="true"]')?.focus());
	}
</script>

<script lang="ts" module>
	/** The id of the section a tab of `Tabs` shows. */
	export function panelId(id: string, value: string): string {
		return `${id}-${value}`;
	}
</script>

<div class="tabs" role="tablist" aria-label={label} bind:this={list} tabindex="-1" onkeydown={key}>
	{#each options as opt (opt.value)}
		<button
			type="button"
			role="tab"
			id="{id}-tab-{opt.value}"
			aria-selected={opt.value === value}
			aria-controls={panelId(id, opt.value)}
			tabindex={opt.value === value ? 0 : -1}
			onclick={() => onselect?.(opt.value)}
		>
			{opt.label}
		</button>
	{/each}
</div>

<style>
	.tabs {
		display: flex;
		flex-direction: column;
		gap: var(--sp-1);
	}
	.tabs:focus {
		outline: none;
	}
	button {
		min-height: var(--control-h);
		padding: var(--sp-2) var(--sp-3);
		border: none;
		border-left: var(--bw) solid transparent;
		background: transparent;
		color: var(--text-muted);
		font: inherit;
		font-weight: var(--fw-medium);
		letter-spacing: var(--ls-caps);
		text-align: left;
		text-transform: uppercase;
		white-space: nowrap;
		cursor: pointer;
	}
	button:hover {
		color: var(--text);
	}
	button[aria-selected='true'] {
		border-left-color: var(--text);
		color: var(--text);
	}
	button:focus-visible {
		outline: var(--bw) solid var(--focus);
		outline-offset: calc(-1 * var(--bw));
	}
	/* A phone: a row across the top, its mark under the name. */
	@media (max-width: 640px) {
		.tabs {
			flex-direction: row;
			overflow-x: auto;
			scrollbar-width: none;
		}
		button {
			border-left: none;
			border-bottom: var(--bw) solid transparent;
		}
		button[aria-selected='true'] {
			border-bottom-color: var(--text);
		}
	}
</style>
