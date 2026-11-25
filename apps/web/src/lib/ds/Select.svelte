<script lang="ts" generics="T extends string">
	import type { Snippet } from 'svelte';

	// One of a few named choices, where a Segmented row would not fit. The
	// value is the parent's: a choice the parent does not take, such as an
	// option that opens a file picker, leaves the select where it was.
	// Stacked, it is a form row as Field lays one, the label over the select
	// across the width and a hint under it; else the label sits beside it.
	//
	// Plain, it is the platform's own select: keyboard, touch and screen
	// readers come with it. With `option`, a snippet that draws an option in
	// the list and the chosen one on the trigger, told which by its second
	// argument (a picture beside a name, which a native option cannot hold),
	// it is a combobox on the ARIA 1.2 pattern, all of that
	// brought along: focus stays on the trigger and points at the option
	// with aria-activedescendant; arrows, Home and End walk the list, a
	// letter jumps to the option it starts, Enter and Space choose, Escape
	// and a press outside close, Tab closes and moves on.
	interface Option {
		value: T;
		label: string;
		/** A rule before it: the options after it are of another kind. */
		rule?: boolean;
	}
	interface Props {
		label: string;
		options: readonly Option[];
		value: T;
		disabled?: boolean;
		stacked?: boolean;
		hint?: string;
		onselect?: (value: T) => void;
		/** Draws an option: in the list, `listed` true; on the trigger, the one chosen. */
		option?: Snippet<[Option, boolean]>;
	}

	let { label, options, value, disabled = false, stacked = false, hint, onselect, option }: Props = $props();

	const uid = $props.id();

	function changed(event: Event & { currentTarget: HTMLSelectElement }) {
		const chosen = event.currentTarget.value as T;
		event.currentTarget.value = value;
		onselect?.(chosen);
	}

	let open = $state(false);
	/** The option aria-activedescendant points at, by its place. */
	let active = $state(-1);
	let trigger: HTMLButtonElement | undefined = $state();
	let list: HTMLUListElement | undefined = $state();
	/** Where the list stands, against the viewport: under the trigger, or over it when there is no room under. */
	let at = $state({ left: 0, top: 0, width: 0, maxHeight: 0 });

	const selected = $derived(options.find((one) => one.value === value));

	// Open, the trigger has the focus, which a click gives no button in
	// Safari: the keys walk the list, and the page keeps them from the
	// world, as for a native select.
	function show() {
		if (disabled) return;
		active = Math.max(0, options.findIndex((one) => one.value === value));
		open = true;
		trigger?.focus();
	}

	function hide() {
		open = false;
		active = -1;
	}

	// Choosing the option already chosen is no choice, as a native select
	// fires no change for it.
	function choose(i: number) {
		const one = options[i];
		hide();
		trigger?.focus();
		if (one && one.value !== value) onselect?.(one.value);
	}

	// A letter jumps to the option it starts; letters typed close together
	// spell the start.
	let typed = '';
	let forget: ReturnType<typeof setTimeout> | undefined;
	function seek(key: string) {
		clearTimeout(forget);
		typed += key.toLowerCase();
		forget = setTimeout(() => (typed = ''), 600);
		const i = options.findIndex((one) => one.label.toLowerCase().startsWith(typed));
		if (i < 0) return;
		if (open) active = i;
		else if (options[i].value !== value) onselect?.(options[i].value);
	}

	function keydown(event: KeyboardEvent) {
		const letter = event.key.length === 1 && event.key !== ' ' && !event.metaKey && !event.ctrlKey && !event.altKey;
		if (!open) {
			if (['Enter', ' ', 'ArrowDown', 'ArrowUp'].includes(event.key)) {
				event.preventDefault();
				show();
			} else if (letter) {
				event.preventDefault();
				seek(event.key);
			}
			return;
		}
		const last = options.length - 1;
		const to: Record<string, () => void> = {
			ArrowDown: () => (active = Math.min(last, active + 1)),
			ArrowUp: () => (active = Math.max(0, active - 1)),
			Home: () => (active = 0),
			End: () => (active = last),
			Enter: () => choose(active),
			' ': () => choose(active),
			Escape: hide
		};
		if (to[event.key]) {
			event.preventDefault();
			to[event.key]();
		} else if (event.key === 'Tab') hide();
		else if (letter) {
			event.preventDefault();
			seek(event.key);
		}
	}

	function place() {
		if (!trigger) return;
		const box = trigger.getBoundingClientRect();
		const below = innerHeight - box.bottom;
		const under = below >= Math.min(box.top, 320) || below >= box.top;
		const room = (under ? below : box.top) - 8;
		at = { left: box.left, top: under ? box.bottom + 2 : box.top - 2, width: box.width, maxHeight: room };
		list?.classList.toggle('over', !under);
	}

	// Open, the list is in the top layer, so no panel that clips or scrolls
	// cuts it, and follows its trigger as the page scrolls or resizes.
	$effect(() => {
		if (!open || !list) return;
		if (typeof list.showPopover === 'function' && !list.matches(':popover-open')) list.showPopover();
		place();
		const outside = (event: PointerEvent) => {
			const target = event.target as Node;
			if (!trigger?.contains(target) && !list?.contains(target)) hide();
		};
		addEventListener('resize', place);
		addEventListener('scroll', place, true);
		document.addEventListener('pointerdown', outside, true);
		return () => {
			removeEventListener('resize', place);
			removeEventListener('scroll', place, true);
			document.removeEventListener('pointerdown', outside, true);
		};
	});

	// The option pointed at stays in sight.
	$effect(() => {
		if (open && active >= 0) list?.querySelector(`[data-i="${active}"]`)?.scrollIntoView({ block: 'nearest' });
	});
</script>

{#if option}
	<div class="select" class:stacked>
		<span class="label" id="{uid}-label">{label}</span>
		<button
			bind:this={trigger}
			type="button"
			class="trigger"
			role="combobox"
			aria-haspopup="listbox"
			aria-expanded={open}
			aria-controls="{uid}-list"
			aria-labelledby="{uid}-label"
			aria-activedescendant={open && active >= 0 ? `${uid}-${active}` : undefined}
			{disabled}
			onclick={() => (open ? hide() : show())}
			onkeydown={keydown}
		>
			<span class="value">{#if selected}{@render option(selected, false)}{/if}</span>
			<svg class="arrow" viewBox="0 0 24 24" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg>
		</button>
		{#if open}
			<ul
				bind:this={list}
				class="list"
				id="{uid}-list"
				role="listbox"
				aria-labelledby="{uid}-label"
				popover="manual"
				style:left="{at.left}px"
				style:top="{at.top}px"
				style:width="{at.width}px"
				style:max-height="{at.maxHeight}px"
			>
				{#each options as one, i (one.value)}
					<!-- svelte-ignore a11y_click_events_have_key_events: the keyboard is the trigger's, which keeps the focus -->
					<li
						id="{uid}-{i}"
						data-i={i}
						role="option"
						aria-selected={one.value === value}
						class:rule={one.rule}
						class:active={i === active}
						onclick={() => choose(i)}
						onpointermove={() => (active = i)}
					>
						{@render option(one, true)}
					</li>
				{/each}
			</ul>
		{/if}
		{#if stacked && hint}<span class="note">{hint}</span>{/if}
	</div>
{:else}
	<label class="select" class:stacked>
		<span class="label">{label}</span>
		<select {value} {disabled} onchange={changed}>
			{#each options as opt (opt.value)}
				{#if opt.rule}<hr />{/if}
				<option value={opt.value}>{opt.label}</option>
			{/each}
		</select>
		{#if stacked && hint}<span class="note">{hint}</span>{/if}
	</label>
{/if}

<style>
	.select {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--sp-3);
	}
	.stacked {
		flex-direction: column;
		align-items: stretch;
		gap: var(--sp-2);
	}
	.stacked .label {
		font-weight: var(--fw-medium);
		letter-spacing: var(--ls-caps);
		text-transform: uppercase;
		color: var(--text-muted);
	}
	.note {
		color: var(--text-muted);
	}
	select,
	.trigger {
		min-height: var(--control-h);
		padding: 0 var(--sp-3);
		border: var(--bw) solid var(--border-strong);
		border-radius: var(--radius);
		background: var(--bg);
		color: var(--text);
		font: inherit;
		cursor: pointer;
	}
	select:disabled,
	.trigger:disabled {
		color: var(--text-muted);
		cursor: not-allowed;
	}
	select:focus-visible,
	.trigger:focus-visible {
		outline: var(--bw) solid var(--focus);
		outline-offset: var(--sp-1);
	}
	.trigger {
		display: flex;
		align-items: center;
		gap: var(--sp-3);
		padding: var(--sp-2) var(--sp-3) var(--sp-2) var(--sp-2);
		text-align: left;
	}
	.value {
		flex: 1;
		min-width: 0;
	}
	.arrow {
		flex: none;
		width: 1.4em;
		height: 1.4em;
		fill: none;
		stroke: currentColor;
		stroke-width: 2;
		stroke-linecap: round;
		stroke-linejoin: round;
		transition: rotate var(--dur-fast) var(--ease-out);
	}
	.trigger[aria-expanded='true'] .arrow {
		rotate: 180deg;
	}
	/* Fixed and placed by `place()`; `inset: auto` and `margin: 0` undo the
	   centring a popover gets from the browser. */
	.list {
		position: fixed;
		inset: auto;
		margin: 0;
		padding: 0;
		overflow-y: auto;
		list-style: none;
		border: var(--bw) solid var(--border-strong);
		background: var(--bg);
		color: var(--text);
		font: inherit;
	}
	.list:global(.over) {
		translate: 0 -100%;
	}
	li {
		padding: var(--sp-2);
		cursor: pointer;
	}
	li.rule {
		border-top: var(--bw) solid var(--border);
	}
	/* The highlight follows aria-activedescendant, so the keyboard and the
	   pointer show the same option. */
	li.active {
		background: var(--action);
		color: var(--action-text);
	}
	li[aria-selected='true']:not(.active) {
		box-shadow: inset var(--sp-1) 0 0 var(--border-strong);
	}
</style>
