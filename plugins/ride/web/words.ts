import { words } from '$lib/i18n';

// Riding's own words, in each language the front end speaks.
// Copy rules: short, professional, no em dashes, no internal information.
const en = {
	name: 'Riding',
	board: 'Board',
	horse: 'Horse',
	ride: 'Ride',
	leave: 'Leave',
	land: 'Land',
	tame: 'Tame',
	release: 'Release',
	call: 'Call',
	let_go: 'Let go',
	'let_go.confirm': '{name} goes back to its herd and is yours no more. This cannot be undone.',
	on_its_way: 'on its way',
	speed: '{n} m/s',
	altitude: '{n} m up',
	'hint.board': 'ride the board, or step off',
	'hint.leap.keys': 'Space',
	'hint.leap': 'on a horse, hold to jump higher',
	'pace.walk': 'Walk',
	'pace.trot': 'Trot',
	'pace.gallop': 'Gallop',
	'pace.run': 'Run',
	'pace.back': 'Back',
	'pace.fly': 'Fly',
	'pace.ceiling': 'Highest it flies',
	'pace.no_room': 'No room',
	'pace.wingless': 'This one does not fly',
	'pace.no_ground': 'No ground'
};

const pt: typeof en = {
	name: 'Montaria',
	board: 'Prancha',
	horse: 'Cavalo',
	ride: 'Montar',
	leave: 'Descer',
	land: 'Pousar',
	tame: 'Domar',
	release: 'Soltar',
	call: 'Chamar',
	let_go: 'Libertar',
	'let_go.confirm': '{name} volta para a manada e deixa de ser seu. Não dá para desfazer.',
	on_its_way: 'a caminho',
	speed: '{n} m/s',
	altitude: '{n} m de altura',
	'hint.board': 'subir na prancha, ou descer',
	'hint.leap.keys': 'Espaço',
	'hint.leap': 'a cavalo, segure para pular mais alto',
	'pace.walk': 'Passo',
	'pace.trot': 'Trote',
	'pace.gallop': 'Galope',
	'pace.run': 'Carreira',
	'pace.back': 'Recuo',
	'pace.fly': 'Voo',
	'pace.ceiling': 'Altura máxima',
	'pace.no_room': 'Sem espaço',
	'pace.wingless': 'Este não voa',
	'pace.no_ground': 'Sem chão'
};

const zh: typeof en = {
	name: '骑乘',
	board: '滑板',
	horse: '马',
	ride: '骑',
	leave: '下来',
	land: '降落',
	tame: '驯服',
	release: '放生',
	call: '呼唤',
	let_go: '放走',
	'let_go.confirm': '{name} 会回到马群，不再属于你。此操作无法撤销。',
	on_its_way: '在路上',
	speed: '{n} 米/秒',
	altitude: '高 {n} 米',
	'hint.board': '踏上滑板，或下来',
	'hint.leap.keys': '空格',
	'hint.leap': '骑马时，按住跳得更高',
	'pace.walk': '慢步',
	'pace.trot': '快步',
	'pace.gallop': '奔跑',
	'pace.run': '疾驰',
	'pace.back': '后退',
	'pace.fly': '飞行',
	'pace.ceiling': '已到最高',
	'pace.no_room': '没有空间',
	'pace.wingless': '这匹不会飞',
	'pace.no_ground': '没有地面'
};

export const t = words({ en, pt, zh });

/** A key of these words as the prompt and the pet's marker name it: a board or a horse, riding it, leaving it, landing it, taming and releasing it; calling the pet and letting it go. */
export const said = (key: string): string => (key in en ? t(key as keyof typeof en) : key);
