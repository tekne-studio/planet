import { words } from '$lib/i18n';

// The flashlight's own words, in each language the front end speaks.
// Copy rules: short, professional, no em dashes, no internal information.
const en = {
	name: 'Flashlight',
	'hint.light': 'flashlight on or off'
};

const pt: typeof en = {
	name: 'Lanterna',
	'hint.light': 'acender ou apagar a lanterna'
};

const zh: typeof en = {
	name: '手电筒',
	'hint.light': '打开或关闭手电筒'
};

export const t = words({ en, pt, zh });
