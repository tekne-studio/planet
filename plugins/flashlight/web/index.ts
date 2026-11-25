import type { WebPlugin } from '$lib/plugins/plugin';
import Button from './Button.svelte';
import { t } from './words';

/** The flashlight in the web front end: its key, and a button that does the same. */
const flashlight: WebPlugin = {
	name: 'flashlight',
	label: () => t('name'),
	Button,
	hints: [{ keys: 'L', does: () => t('hint.light') }]
};

export default flashlight;
