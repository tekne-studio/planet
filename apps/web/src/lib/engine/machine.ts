import type { Gpu } from './start';

/**
 * What the browser says of the machine, and what the page reads from it:
 * the family of its graphics, whether that family may be too light for the
 * world, and a key that names this GPU. The graphics are asked before a
 * start; the rest only when the list of facts is shown.
 */
export interface Graphics {
	/** WebGPU's adapter, the one the engine will be given, or why WebGPU said nothing. */
	webgpu: WebGpu | Silent;
	/** WebGL2's renderer, unmasked where the browser lets it be; null where it gives no WebGL2. */
	webgl: string | null;
}

/**
 * Why WebGPU said nothing: the browser has no WebGPU (`absent`), it gave no
 * adapter (`none`), WebGL2 is the road kept, so it was left alone
 * (`unasked`), or it did not answer in time (`late`).
 */
export type Silent = 'absent' | 'none' | 'unasked' | 'late';

/** What WebGPU says of its adapter. Chrome gives vendor and architecture and leaves the rest empty. */
export interface WebGpu {
	vendor: string;
	architecture: string;
	device: string;
	description: string;
	/** A software adapter, which draws on the processor. */
	fallback: boolean;
	/** Whether it compresses textures as ASTC, as Apple's own chips do and an Intel Mac's GPU does not. */
	astc: boolean;
	/** The largest side of a 2D texture, texels. */
	texturePx: number;
	/** The largest buffer, MiB. */
	bufferMib: number;
}

/** What the browser says of the rest of the machine. */
export interface Machine {
	/** The browser and its version, as it names itself. */
	browser: string;
	/** The system and its version, as the browser gives them. */
	system: string;
	screen: { width: number; height: number; ratio: number };
	/** Logical processors; null where the browser does not say. */
	cores: number | null;
	/** Memory, GiB, rounded by the browser; null where it does not say. */
	memory: number | null;
	touchPoints: number;
	language: string;
	/** The top row of letters as the keyboard types them (`qwerty`, `azerty`); null where the browser does not say. */
	keyboard: string | null;
}

/**
 * The family of a machine's graphics, never its model: enough to tell a
 * card that holds the world from one that may not.
 */
export const families = ['none', 'software', 'hidden', 'nvidia', 'apple', 'amd', 'amd-integrated', 'intel', 'intel-arc', 'mobile'] as const;
export type Family = (typeof families)[number];

/** The families that may not hold the world at its fullest: the graphics dialog opens for them once. */
const weakFamilies: ReadonlySet<Family> = new Set(['none', 'software', 'hidden', 'amd-integrated', 'intel', 'mobile']);

export function weak(family: Family): boolean {
	return weakFamilies.has(family);
}

const SOFTWARE = /swiftshader|llvmpipe|softpipe|lavapipe|basic render|software/i;

/**
 * The family. The adapter the engine will be given speaks first, by its
 * name where it gives one, else by its vendor: on a laptop with two GPUs
 * WebGL may name the other. AMD's cards and its integrated graphics share
 * architectures, so WebGL's name tells them apart, when it names AMD.
 * Without an adapter, WebGL's name alone. Firefox blurs that name to a
 * bucket that keeps the vendor and the tier, ", or similar" after it.
 */
export function classify({ webgpu, webgl }: Graphics): Family {
	const adapter = typeof webgpu === 'object' ? webgpu : null;
	if ((webgpu === 'absent' || webgpu === 'none') && webgl === null) return 'none';
	if (adapter?.fallback) return 'software';
	const gl = (webgl ?? '').replace(/,\s*or similar$/i, '');
	const own = adapter ? `${adapter.description} ${adapter.device}` : '';
	if (SOFTWARE.test(`${gl} ${own}`)) return 'software';
	// Safari names every Mac's GPU so, an Intel Mac's too.
	if (/^apple gpu$/i.test(gl.trim()) && !adapter?.vendor) return adapter?.astc ? 'apple' : 'hidden';
	if (adapter) {
		const named = byName(own);
		if (named) return named;
		if (adapter.vendor.toLowerCase() === 'amd') {
			const said = byName(gl);
			return said === 'amd' || said === 'amd-integrated' ? said : 'hidden';
		}
		const vendor = byVendor(adapter.vendor, adapter.architecture);
		if (vendor) return vendor;
	}
	return byName(gl) ?? 'hidden';
}

function byName(name: string): Family | null {
	if (/tegra|qualcomm|adreno|\bmali\b|powervr|xclipse|immortalis|videocore|\bv3d\b|vivante/i.test(name)) return 'mobile';
	if (/nvidia|geforce|quadro/i.test(name)) return 'nvidia';
	if (/\bapple\b/i.test(name)) return 'apple';
	// Intel's integrated graphics; Arc's cards are named by a series and a number.
	if (/\bintel\b/i.test(name)) return /\barc(\(tm\))?\s+[ab]\d/i.test(name) ? 'intel-arc' : 'intel';
	if (/\bamd\b|radeon|firepro/i.test(name)) {
		// Firefox's bucket for Radeon cards and the newer integrated graphics alike.
		if (/radeon r9 200 series/i.test(name)) return null;
		// A card is RX, Pro or FirePro; a Ryzen's graphics are Radeon Graphics, RX Vega and a number
		// and Graphics, or a number and M, and Firefox's HD buckets hold those and the old cards, light alike.
		if (/rx vega \d+ graphics/i.test(name)) return 'amd-integrated';
		return /radeon(\(tm\))?\s+(rx|pro|vii)\b|firepro/i.test(name) ? 'amd' : 'amd-integrated';
	}
	return null;
}

/** Chrome's vendors and Intel's architectures: `-hp`, `-hpg` are Arc's cards, the rest integrated. AMD's is classify's. */
function byVendor(vendor: string, architecture: string): Family | null {
	switch (vendor.toLowerCase()) {
		case 'nvidia':
			return 'nvidia';
		case 'apple':
			return 'apple';
		case 'intel':
			return /hpg?$/i.test(architecture) ? 'intel-arc' : 'intel';
		case 'qualcomm':
		case 'arm':
		case 'img-tec':
			return 'mobile';
		default:
			return null;
	}
}

/**
 * The versions a renderer's name carries of its driver and kernel, which
 * an update changes on the same GPU: Mesa's, LLVM's, DRM's, an API's, a
 * driver's, and a kernel's release.
 */
const VERSIONS = /\b(?:mesa|llvm|drm|vulkan|opengl(?: es)?|driver)[\s-]+\d+(?:\.\d+)+(?:-[\w.~+-]+)?|\b\d+\.\d+\.\d+-[\w.~+-]+/gi;

/**
 * A short name for this GPU as this browser reaches it: a choice kept for
 * it is not asked again, and another GPU is asked anew. WebGL's renderer
 * where it names one, its versions left out, since it stays the same
 * whichever adapter and road are kept; else what WebGPU says of its
 * adapter.
 */
export function gpuKey({ webgpu, webgl }: Graphics): string {
	const adapter = typeof webgpu === 'object' ? webgpu : null;
	const named = webgl !== null && !/generic renderer|webkit webgl/i.test(webgl);
	const said = named ? webgl.replace(VERSIONS, '') : [adapter?.vendor, adapter?.architecture, adapter?.device, adapter?.description].join('|');
	// FNV-1a, 32 bits: a key to compare, never to keep secret.
	let hash = 0x811c9dc5;
	for (const char of said) {
		hash ^= char.codePointAt(0) ?? 0;
		hash = Math.imul(hash, 0x01000193) >>> 0;
	}
	return hash.toString(16).padStart(8, '0');
}

/** What the browser offers beyond the DOM's types: WebGPU, the user agent's data, memory, the keyboard. */
interface Offered {
	gpu?: { requestAdapter(options?: { powerPreference?: string }): Promise<RawAdapter | null> };
	userAgentData?: {
		platform: string;
		getHighEntropyValues(hints: string[]): Promise<{ fullVersionList?: { brand: string; version: string }[]; platformVersion?: string }>;
	};
	deviceMemory?: number;
	keyboard?: { getLayoutMap(): Promise<Map<string, string>> };
}
interface RawAdapter {
	info?: Partial<Record<'vendor' | 'architecture' | 'device' | 'description', string>> & { isFallbackAdapter?: boolean };
	isFallbackAdapter?: boolean;
	features?: { has(name: string): boolean };
	limits: { maxTextureDimension2D: number; maxBufferSize: number };
}

/** How long WebGPU is given to name its adapter, ms: a browser that hangs on it must not hold the start. */
const GRAPHICS_WAIT_MS = 2000;

/**
 * Asks the browser what it says of the graphics, as the engine will reach
 * them: WebGPU for the adapter of the power kept, unless WebGL2 is the road
 * kept, and WebGL2's renderer.
 */
export async function askGraphics(gpu: Gpu, waitMs = GRAPHICS_WAIT_MS): Promise<Graphics> {
	return { webgpu: await adapter(gpu, waitMs), webgl: renderer(gpu) };
}

async function adapter(gpu: Gpu, waitMs: number): Promise<WebGpu | Silent> {
	const offered = navigator as Navigator & Offered;
	if (!offered.gpu) return 'absent';
	if (gpu.backend === 'webgl2') return 'unasked';
	const asked = offered.gpu.requestAdapter({ powerPreference: gpu.power === 'low' ? 'low-power' : 'high-performance' }).then(
		(raw): WebGpu | Silent => (raw ? read(raw) : 'none'),
		(): Silent => 'none'
	);
	const late = new Promise<Silent>((done) => setTimeout(() => done('late'), waitMs));
	return Promise.race([asked, late]);
}

function read(raw: RawAdapter): WebGpu {
	const info = raw.info ?? {};
	return {
		vendor: info.vendor ?? '',
		architecture: info.architecture ?? '',
		device: info.device ?? '',
		description: info.description ?? '',
		fallback: info.isFallbackAdapter ?? raw.isFallbackAdapter ?? false,
		astc: raw.features?.has('texture-compression-astc') ?? false,
		texturePx: raw.limits.maxTextureDimension2D,
		bufferMib: Math.floor(raw.limits.maxBufferSize / 2 ** 20)
	};
}

/**
 * WebGL2's renderer, from a canvas made for the asking and let go at once,
 * asked for the adapter the engine will be: on a machine with two GPUs,
 * WebGL names the one asked for.
 */
function renderer(gpu: Gpu = {}): string | null {
	try {
		const gl = document.createElement('canvas').getContext('webgl2', { powerPreference: gpu.power === 'low' ? 'low-power' : 'high-performance' });
		if (!gl) return null;
		const debug = gl.getExtension('WEBGL_debug_renderer_info');
		const said = String(gl.getParameter(debug ? debug.UNMASKED_RENDERER_WEBGL : gl.RENDERER));
		gl.getExtension('WEBGL_lose_context')?.loseContext();
		return said;
	} catch {
		return null;
	}
}

/** Whether the browser offers a road to a GPU at all: WebGPU, or WebGL2. */
export function offersARoad(): boolean {
	return 'gpu' in navigator || renderer() !== null;
}

/**
 * Asks the browser what it says of the rest of the machine. Each answer
 * stands alone: one the browser refuses is left unknown, never the whole.
 */
export async function askMachine(): Promise<Machine> {
	const offered = navigator as Navigator & Offered;
	const [named, keyboard] = await Promise.all([naming(offered), layout(offered)]);
	return {
		...named,
		screen: { width: screen.width, height: screen.height, ratio: devicePixelRatio },
		cores: navigator.hardwareConcurrency || null,
		memory: offered.deviceMemory ?? null,
		touchPoints: navigator.maxTouchPoints,
		language: navigator.language,
		keyboard
	};
}

/** The browser's and the system's names: Chromium's own data where it gives them, else the user agent. */
async function naming(offered: Offered): Promise<Pick<Machine, 'browser' | 'system'>> {
	const data = offered.userAgentData;
	if (data) {
		try {
			const high = await data.getHighEntropyValues(['fullVersionList', 'platformVersion']);
			const brands = (high.fullVersionList ?? []).filter((brand) => !/not.a.brand/i.test(brand.brand));
			const brand = brands.find((one) => one.brand !== 'Chromium') ?? brands[0];
			if (brand) {
				return {
					browser: `${brand.brand} ${brand.version}`,
					system: [data.platform, high.platformVersion].filter(Boolean).join(' ')
				};
			}
		} catch {
			// the user agent below says it
		}
	}
	return { browser: navigator.userAgent, system: navigator.platform };
}

/** The top row of letters, as the keyboard types them. */
async function layout(offered: Offered): Promise<string | null> {
	try {
		const map = await offered.keyboard?.getLayoutMap();
		if (!map) return null;
		const row = ['KeyQ', 'KeyW', 'KeyE', 'KeyR', 'KeyT', 'KeyY'].map((code) => map.get(code) ?? '?').join('');
		return row === '??????' ? null : row;
	} catch {
		return null;
	}
}
