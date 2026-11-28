<script lang="ts">
	import type { Snippet } from 'svelte';
	import { enhance } from '$app/forms';
	import type { ActionResult } from '@sveltejs/kit';
	import Button from './Button.svelte';
	import { t } from '$lib/i18n';

	// Modal over the native <dialog>: focus trapped, Esc closes, page dims.
	// Project rule: EVERY destructive action passes through a confirming Dialog.
	// The box is a form. With `action` it posts to a SvelteKit form action
	// (children carry the fields); without, it calls `onconfirm`. A dialog
	// that walks a person through steps (sign in: the email, then the code)
	// reads each action's result with `onresult` and says whether it is done.
	// A dialog with one way forward, `oneWay`, has no cancel: Esc and a click
	// outside are its button, so it is answered however it is left.
	interface Props {
		open?: boolean;
		title: string;
		confirmLabel: string;
		/** Destructive: the confirm button takes the danger variant. */
		danger?: boolean;
		/** Form action to post to, for example `?/delete`. */
		action?: string;
		/**
		 * With `action`: reads the result instead of applying it to the page,
		 * and returns whether the dialog closes. Absent, the page updates and
		 * the dialog closes.
		 */
		onresult?: (result: ActionResult) => boolean | Promise<boolean>;
		onconfirm?: () => void | Promise<void>;
		/** One way forward: no cancel, and Esc or a click outside confirms. */
		oneWay?: boolean;
		children: Snippet;
	}

	let { open = $bindable(false), title, confirmLabel, danger = false, action, onresult, onconfirm, oneWay = false, children }: Props = $props();

	let el: HTMLDialogElement | undefined = $state();
	let form: HTMLFormElement | undefined = $state();

	/** Esc or a click outside: the cancel, or the one way forward. */
	function leave() {
		if (oneWay) form?.requestSubmit();
		else open = false;
	}
	let busy = $state(false);

	$effect(() => {
		if (!el) return;
		if (open && !el.open) {
			el.showModal();
			// One way forward opens on it, so Enter answers, never on a choice.
			if (oneWay) form?.querySelector<HTMLButtonElement>('button[type=submit]')?.focus();
		}
		if (!open && el.open) el.close();
	});

	async function confirm(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		try {
			await onconfirm?.();
		} finally {
			busy = false;
			open = false;
		}
	}
</script>

{#snippet content()}
	<h2>{title}</h2>
	<div class="body">{@render children()}</div>
	<footer>
		{#if !oneWay}
			<Button type="button" variant="ghost" onclick={() => (open = false)}>{t('common.cancel')}</Button>
		{/if}
		<Button type="submit" variant={danger ? 'danger' : 'primary'} loading={busy}>{confirmLabel}</Button>
	</footer>
{/snippet}

<dialog
	bind:this={el}
	onclose={() => (open = false)}
	oncancel={(e) => {
		e.preventDefault();
		if (!busy) leave();
	}}
	onclick={(e) => {
		if (e.target === el && !busy) leave();
	}}
>
	{#if action}
		<form
			bind:this={form}
			class="box"
			method="POST"
			{action}
			use:enhance={() => {
				busy = true;
				return async ({ result, update }) => {
					if (onresult) {
						const done = await onresult(result);
						busy = false;
						if (done) open = false;
						return;
					}
					await update();
					busy = false;
					open = false;
				};
			}}
		>
			{@render content()}
		</form>
	{:else}
		<form bind:this={form} class="box" onsubmit={confirm}>
			{@render content()}
		</form>
	{/if}
</dialog>

<style>
	dialog {
		/* the reset zeroes margins; <dialog> centres through margin: auto */
		margin: auto;
		padding: 0;
		border: none;
		background: transparent;
		color: var(--text);
		width: min(var(--page-narrow), calc(100vw - 2 * var(--sp-5)));
	}
	dialog::backdrop {
		background: var(--backdrop);
	}
	.box {
		display: flex;
		flex-direction: column;
		gap: var(--sp-4);
		padding: var(--sp-5);
		border: var(--bw) solid var(--border-strong);
		background: var(--bg);
	}
	h2 {
		letter-spacing: var(--ls-caps);
		text-transform: uppercase;
	}
	.body {
		display: flex;
		flex-direction: column;
		gap: var(--sp-4);
		color: var(--text-muted);
		overflow-wrap: anywhere;
	}
	footer {
		display: flex;
		justify-content: flex-end;
		gap: var(--sp-3);
	}
</style>
