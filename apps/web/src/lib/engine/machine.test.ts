import { describe, expect, test } from 'bun:test';
import { classify, gpuKey, weak, type Family, type Silent, type WebGpu } from './machine';

const gpu = (said: Partial<WebGpu> = {}): WebGpu => ({
	vendor: '',
	architecture: '',
	device: '',
	description: '',
	fallback: false,
	astc: false,
	texturePx: 8192,
	bufferMib: 256,
	...said
});

// What browsers say, as they say it: WebGL's renderer, and WebGPU's adapter or why it said none.
const said: Record<string, [string | null, WebGpu | Silent, Family]> = {
	'nothing at all': [null, 'absent', 'none'],
	'WebGPU with no adapter, and no WebGL2': [null, 'none', 'none'],
	'a fallback adapter': ['ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (Subzero)), SwiftShader driver)', gpu({ fallback: true }), 'software'],
	'SwiftShader through WebGL2 alone': ['ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (Subzero) (0x0000C0DE)), SwiftShader driver)', 'absent', 'software'],
	'llvmpipe on Linux': ['llvmpipe (LLVM 15.0.7, 256 bits)', 'none', 'software'],
	'Windows with no driver': ['ANGLE (Microsoft, Microsoft Basic Render Driver (0x0000008C) Direct3D11 vs_5_0 ps_5_0, D3D11)', 'none', 'software'],
	'an NVIDIA laptop card': [
		'ANGLE (NVIDIA, NVIDIA GeForce RTX 4050 Laptop GPU (0x000028A1) Direct3D11 vs_5_0 ps_5_0, D3D11)',
		gpu({ vendor: 'nvidia', architecture: 'lovelace' }),
		'nvidia'
	],
	'an Apple chip in Chrome': ['ANGLE (Apple, ANGLE Metal Renderer: Apple M1 Pro, Unspecified Version)', gpu({ vendor: 'apple', architecture: 'metal-3', astc: true }), 'apple'],
	'an Apple chip in Safari': ['Apple GPU', gpu({ astc: true }), 'apple'],
	'Safari on an Intel Mac': ['Apple GPU', gpu(), 'hidden'],
	'Safari with no WebGPU': ['Apple GPU', 'absent', 'hidden'],
	"a Ryzen's graphics": ['ANGLE (AMD, AMD Radeon(TM) Graphics (0x00001638) Direct3D11 vs_5_0 ps_5_0, D3D11)', gpu({ vendor: 'amd', architecture: 'gcn-5' }), 'amd-integrated'],
	"a Ryzen's RX Vega graphics": [
		'ANGLE (AMD, AMD Radeon(TM) RX Vega 10 Graphics (0x000015D8) Direct3D11 vs_5_0 ps_5_0, D3D11)',
		gpu({ vendor: 'amd', architecture: 'gcn-5' }),
		'amd-integrated'
	],
	"a Ryzen's RX Vega graphics, WebGL alone": ['ANGLE (AMD, AMD Radeon(TM) RX Vega 11 Graphics Direct3D11 vs_5_0 ps_5_0, D3D11)', 'absent', 'amd-integrated'],
	'an RX Vega card': ['ANGLE (AMD, Radeon RX Vega (0x0000687F) Direct3D11 vs_5_0 ps_5_0, D3D11)', gpu({ vendor: 'amd', architecture: 'gcn-5' }), 'amd'],
	"a Ryzen's newer graphics": [
		'ANGLE (AMD, AMD Radeon 780M Graphics (0x000015BF) Direct3D11 vs_5_0 ps_5_0, D3D11)',
		gpu({ vendor: 'amd', architecture: 'rdna-3' }),
		'amd-integrated'
	],
	'a Radeon card': ['ANGLE (AMD, AMD Radeon RX 6700 XT (0x000073DF) Direct3D11 vs_5_0 ps_5_0, D3D11)', gpu({ vendor: 'amd', architecture: 'rdna-2' }), 'amd'],
	"an Intel laptop's graphics": [
		'ANGLE (Intel, Intel(R) Iris(R) Xe Graphics (0x00009A49) Direct3D11 vs_5_0 ps_5_0, D3D11)',
		gpu({ vendor: 'intel', architecture: 'gen-12lp' }),
		'intel'
	],
	"Meteor Lake's graphics, named Arc": [
		'ANGLE (Intel, Intel(R) Arc(TM) Graphics (0x00007D55) Direct3D11 vs_5_0 ps_5_0, D3D11)',
		gpu({ vendor: 'intel', architecture: 'xe-lpg' }),
		'intel'
	],
	'an Arc card': ['ANGLE (Intel, Intel(R) Arc(TM) A770 Graphics (0x000056A0) Direct3D11 vs_5_0 ps_5_0, D3D11)', gpu({ vendor: 'intel', architecture: 'gen-12hp' }), 'intel-arc'],
	'a Snapdragon laptop': ['ANGLE (Qualcomm, Adreno (TM) 690 (0x00000690) Direct3D11 vs_5_0 ps_5_0, D3D11)', gpu({ vendor: 'qualcomm' }), 'mobile'],
	'a Raspberry Pi': ['V3D 4.2', 'none', 'mobile'],
	'WebGL masked, WebGPU naming NVIDIA': ['WebKit WebGL', gpu({ vendor: 'nvidia', architecture: 'ampere' }), 'nvidia'],
	'WebGL masked, WebGPU naming an Arc card': ['WebKit WebGL', gpu({ vendor: 'intel', architecture: 'xe-hpg' }), 'intel-arc'],
	'WebGL masked, WebGPU naming AMD alone': ['WebKit WebGL', gpu({ vendor: 'amd', architecture: 'rdna-2' }), 'hidden'],
	// Firefox says a bucket: the vendor and the tier kept, ", or similar" after.
	'Firefox on Windows, an NVIDIA card': ['ANGLE (NVIDIA, NVIDIA GeForce GTX 980 Direct3D11 vs_5_0 ps_5_0), or similar', 'absent', 'nvidia'],
	"Firefox on Windows, Intel's graphics": ['ANGLE (Intel, Intel(R) HD Graphics 400 Direct3D11 vs_5_0 ps_5_0), or similar', 'absent', 'intel'],
	'Firefox on a Mac, an Apple chip': ['Apple M1, or similar', 'absent', 'apple'],
	"Firefox's old Radeon bucket": ['ANGLE (AMD, Radeon HD 3200 Graphics Direct3D11 vs_5_0 ps_5_0), or similar', 'absent', 'amd-integrated'],
	"Firefox's Radeon bucket, cards and integrated alike": ['ANGLE (AMD, Radeon R9 200 Series Direct3D11 vs_5_0 ps_5_0), or similar', 'absent', 'hidden'],
	'Firefox with WebGPU, its bucket breaking AMD': ['ANGLE (AMD, Radeon R9 200 Series Direct3D11 vs_5_0 ps_5_0), or similar', gpu({ vendor: 'amd' }), 'hidden'],
	'Firefox on Android': ['Adreno (TM) 540, or similar', 'absent', 'mobile'],
	// Two GPUs: WebGL may name the one the engine is not given.
	'a laptop with two, the adapter the card': [
		'ANGLE (Intel, Intel(R) UHD Graphics 630 (0x00003E9B) Direct3D11 vs_5_0 ps_5_0, D3D11)',
		gpu({ vendor: 'nvidia', architecture: 'turing' }),
		'nvidia'
	],
	"AMD's adapter, WebGL naming the card": ['ANGLE (AMD, AMD Radeon Pro 5500M OpenGL Engine, OpenGL 4.1)', gpu({ vendor: 'amd', architecture: 'rdna-1' }), 'amd'],
	'WebGPU late, WebGL says': ['ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 (0x00002504) Direct3D11 vs_5_0 ps_5_0, D3D11)', 'late', 'nvidia'],
	'WebGL2 kept, WebGPU not asked': ['ANGLE (Intel, Intel(R) Iris(R) Xe Graphics (0x00009A49) Direct3D11 vs_5_0 ps_5_0, D3D11)', 'unasked', 'intel']
};

describe('classify', () => {
	test.each(Object.entries(said))('%s', (_, [webgl, webgpu, family]) => {
		expect(classify({ webgl, webgpu })).toBe(family);
	});

	test('a card holds the world; integrated, mobile, software and unknown graphics may not', () => {
		const holds: Family[] = ['nvidia', 'apple', 'amd', 'intel-arc'];
		const mayNot: Family[] = ['none', 'software', 'hidden', 'amd-integrated', 'intel', 'mobile'];
		expect(holds.filter(weak)).toEqual([]);
		expect(mayNot.filter((family) => !weak(family))).toEqual([]);
	});
});

describe('gpuKey', () => {
	const graphics = (name: string) => {
		const [webgl, webgpu] = said[name];
		return { webgl, webgpu };
	};

	test('the same GPU is the same key, another GPU another', () => {
		const nvidia = graphics('an NVIDIA laptop card');
		expect(gpuKey(nvidia)).toBe(gpuKey({ ...nvidia }));
		expect(gpuKey(nvidia)).not.toBe(gpuKey(graphics("a Ryzen's graphics")));
		expect(gpuKey(nvidia)).toMatch(/^[0-9a-f]{8}$/);
	});

	test('the road kept does not change the key: WebGL names the GPU either way', () => {
		const nvidia = graphics('an NVIDIA laptop card');
		expect(gpuKey({ ...nvidia, webgpu: 'unasked' })).toBe(gpuKey(nvidia));
		expect(gpuKey({ ...nvidia, webgpu: 'late' })).toBe(gpuKey(nvidia));
	});

	test("a driver's and a kernel's versions are no part of the GPU", () => {
		const mesa = (versions: string) => gpuKey({ webgl: `AMD Radeon Graphics (renoir, LLVM ${versions})`, webgpu: 'absent' });
		expect(mesa('15.0.7, DRM 3.54, 6.5.0-14-generic')).toBe(mesa('17.0.6, DRM 3.57, 6.8.0-31-generic'));
		const vulkan = (driver: string) =>
			gpuKey({
				webgl: `ANGLE (Intel, Vulkan ${driver} (Intel(R) UHD Graphics 620 (KBL GT2) (0x00005917)), Intel open-source Mesa driver-${driver})`,
				webgpu: 'absent'
			});
		expect(vulkan('1.3.255')).toBe(vulkan('1.3.274'));
		expect(gpuKey({ webgl: 'Mesa Intel(R) UHD Graphics 620 (KBL GT2), Mesa 23.2.1', webgpu: 'absent' })).not.toBe(
			gpuKey({ webgl: 'Mesa Intel(R) UHD Graphics 630 (CFL GT2), Mesa 23.2.1', webgpu: 'absent' })
		);
	});

	test('WebGL masked, the adapter names the GPU', () => {
		const masked = graphics('WebGL masked, WebGPU naming NVIDIA');
		expect(gpuKey(masked)).not.toBe(gpuKey(graphics('WebGL masked, WebGPU naming an Arc card')));
	});
});
