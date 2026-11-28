import { describe, expect, test } from 'bun:test';
import { translate } from '../i18n/config';
import { plain, rows, type Facts, type Say } from './facts';
import type { Graphics, Machine } from './machine';
import type { Effects } from './protocol';

const english: Say = (key, params) => translate('en', key, params);

const graphics: Graphics = {
	webgpu: { vendor: 'amd', architecture: 'gcn-5', device: '', description: '', fallback: false, astc: false, texturePx: 8192, bufferMib: 256 },
	webgl: 'ANGLE (AMD, AMD Radeon(TM) Graphics (0x00001638) Direct3D11 vs_5_0 ps_5_0, D3D11)'
};

const machine: Machine = {
	browser: 'Google Chrome 131.0.6778.86',
	system: 'Windows 15.0.0',
	screen: { width: 1536, height: 864, ratio: 1.25 },
	cores: 16,
	memory: 8,
	touchPoints: 0,
	language: 'pt-BR',
	keyboard: 'qwerty'
};

const facts: Facts = {
	version: '0.42.0',
	host: 'world.example',
	graphics,
	machine,
	last: { stopped: 'drawn' },
	kept: { picture: 'light', effects: { trees: true, shrubs: false, flowers: false, animals: true } as Effects, gpu: { backend: 'webgl2' }, key: '1a2b3c4d' },
	said: ['safe'],
	touch: false
};

const said = (list: Facts) => Object.fromEntries(rows(list, english).map(({ label, value }) => [label, value]));

describe('rows', () => {
	test('before the start: the engine, the browser, the graphics, the screen, what is kept', () => {
		expect(said(facts)).toEqual({
			Engine: '0.42.0',
			World: 'world.example',
			Browser: 'Google Chrome 131.0.6778.86',
			System: 'Windows 15.0.0',
			WebGPU: 'amd · gcn-5',
			WebGL2: graphics.webgl as string,
			Family: 'AMD integrated, may be light',
			Limits: 'textures 8192 px, buffers 256 MiB',
			Screen: '1536 × 864 px at 1.25',
			Processor: '16 cores, 8 GiB',
			Touch: 'no, 0 touch points',
			Language: 'pt-BR, keyboard qwerty',
			'Last start': 'stalled at First frame drawn',
			Kept: 'Light; trees on, shrubs off, flowers off, animals on; adapter high performance, road WebGL2',
			'GPU key': '1a2b3c4d',
			Flags: 'safe'
		});
	});

	test('a start under way says each step reached and the next one waiting, and what the engine said', () => {
		const list = said({
			...facts,
			start: {
				reached: { module: 412, adapter: 655 },
				adapter: { step: 'adapter', name: '', backend: 'webgpu', texture_px: 16384, buffer_mib: 2048 },
				about: ['amd', 'gcn-5'],
				failure: null
			}
		});
		expect(list['Engine loaded']).toBe('0.41 s');
		expect(list['Adapter given']).toBe('0.66 s');
		expect(list['Device open']).toBe('…');
		expect(list['Pipelines made']).toBeUndefined();
		expect(list.Adapter).toBe('amd · gcn-5');
		expect(list.Road).toBe('WebGPU');
		expect(list.Limits).toBe('textures 16384 px, buffers 2048 MiB');
	});

	test('a running world adds the frame, the keys held and who has them', () => {
		const list = said({
			...facts,
			live: {
				frame: { mean: 16.71, longest: 33.4 },
				held: { codes: [], keys: ['Forward'], stick: [0, 0], away: false },
				points: 0,
				hidden: false,
				focused: true
			}
		});
		expect(list.Frame).toBe('16.7 ms, longest 33 ms');
		expect(list.Input).toBe('Forward');
		expect(list.Away).toBe('no, page focused');
	});

	test('what a browser does not say is said unknown, never left out', () => {
		const list = said({
			...facts,
			version: null,
			graphics: { webgpu: 'absent', webgl: null },
			machine: { ...machine, cores: null, memory: null, keyboard: null }
		});
		expect(list.Engine).toBe('unknown');
		expect(list.Processor).toBe('unknown cores, unknown GiB');
		expect(list.WebGPU).toBe('not offered');
		expect(list.WebGL2).toBe('not offered');
		expect(list.Family).toBe('no GPU, may be light');
	});

	test('why WebGPU said nothing is said', () => {
		const silent = (webgpu: Graphics['webgpu']) => said({ ...facts, graphics: { ...graphics, webgpu } }).WebGPU;
		expect(silent('none')).toBe('no adapter given');
		expect(silent('unasked')).toBe('not asked, WebGL2 kept');
		expect(silent('late')).toBe('no answer in time');
	});

	test('the rest of the machine is said only once asked', () => {
		const list = said({ ...facts, machine: null });
		expect(list.Browser).toBeUndefined();
		expect(list.Screen).toBeUndefined();
		expect(list.WebGPU).toBe('amd · gcn-5');
	});

	test('kept says only what is kept: nothing, the picture alone, or a GPU off the default', () => {
		const kept = (value: Partial<Facts['kept']>) => said({ ...facts, kept: { ...facts.kept, ...value } }).Kept;
		expect(kept({ picture: null, effects: null, gpu: null })).toBe('nothing');
		expect(kept({ gpu: null })).toBe('Light; trees on, shrubs off, flowers off, animals on');
		expect(kept({ picture: null, effects: null })).toBe('adapter high performance, road WebGL2');
	});
});

describe('plain', () => {
	test('a row a line, label and value', () => {
		expect(plain([{ label: 'Engine', value: '0.42.0' }, { label: 'World', value: 'here' }])).toBe('Engine: 0.42.0\nWorld: here');
	});
});
