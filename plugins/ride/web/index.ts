import type { WebPlugin } from '$lib/plugins/plugin';
import Button from './Button.svelte';
import Layer from './Layer.svelte';
import { t, said } from './words';

/** Riding in the web front end: its key, the board's button, the words its offers are said in, and the speed and the pace while riding. */
const ride: WebPlugin = {
	name: 'ride',
	label: () => t('name'),
	Layer,
	Button,
	hints: [{ keys: 'H', does: () => t('hint.board') }],
	words: said
};

export default ride;
