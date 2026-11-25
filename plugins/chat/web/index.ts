import type { WebPlugin } from '$lib/plugins/plugin';
import Button from './Button.svelte';
import Layer from './Layer.svelte';
import { t } from './words';

/** Chat in the web front end: a bar at the foot of the world, behind a button on a touch screen, and balloons over heads. */
const chat: WebPlugin = {
	name: 'chat',
	label: () => t('name'),
	Layer,
	Button,
	hints: [{ keys: 'Enter', does: () => t('hint.chat') }]
};

export default chat;
