// Bakes a field: the ground of a real body, as a cube map our sectors read.
//   bun run field             out/fields/earth.field, its sidecar and a preview,
//                             and whether it is the committed Earth's ground
//   bun run field --replace   the same into assets/fields/, as the Earth every
//                             version carries. Another id is another world: a
//                             world founded on the committed Earth stops loading
// The source is ETOPO 2022 (NOAA NCEI), topography and bathymetry in one
// grid, a work of the United States government and so in the public domain.
// It is fetched once into `target/field/`, and only the bake is an asset.
//
// Our planet is 1/305 of Earth, so the elevations are not heights here: what
// survives the change of scale is the shape. See `crates/worldgen/src/v3.rs`.
import { $ } from 'bun';
import { mkdir, readdir } from 'node:fs/promises';
// The page checks a fetched field by this rule, so the bake writes by it.
import { contentId } from '../apps/web/src/lib/fields';
import { ROOT } from './lib';

// --- The source grid -------------------------------------------------------

/** ETOPO 2022 at 60 arc-seconds, as the OPeNDAP server names it. */
const DODS =
	'https://www.ngdc.noaa.gov/thredds/dodsC/global/ETOPO2022/60s/60s_surface_elev_netcdf/ETOPO_2022_v1_60s_N90W180_surface.nc.dods';
const SRC_ROWS = 10800;
const SRC_COLS = 21600;
/** Every other sample: 3.7 km on Earth, still far finer than a texel here. */
const STRIDE = 2;
/** Source rows per request. Whole grid in one is refused halfway through. */
const BAND = 900;

const ROWS = SRC_ROWS / STRIDE;
const COLS = SRC_COLS / STRIDE;

// --- The cube map ----------------------------------------------------------

/**
 * Texels per face side the source is scattered into: 32 m of planet per
 * texel, 9.8 km of Earth. Ruggedness is the spread inside one of these, and
 * v3's belt thresholds are tuned to that grain.
 */
const GRAIN = 1024;
/**
 * Texels per face side the file stores: 64 m of planet, 19.6 km of Earth.
 * It is the grain's pyramid with its finest level dropped, never a scatter at
 * this side: ruggedness measured over a wider texel grows a mountain belt on
 * every shelf break. What the drop loses is coastline under a texel, the
 * islands and straits narrower than about 20 km of Earth; belts and lift
 * read coarser levels, the same at either side.
 */
const SIDE = 512;
/** Metres of ruggedness in one step of the byte. Matches `field.rs`. */
const RUGGED_STEP = 16;
/**
 * Which way longitude runs on our sphere.
 *
 * A place on the source body has to be put at the direction where the engine
 * draws it, and that frame is a mirror of the body's: baked straight in, a map
 * comes out east for west, and the Italian boot points the wrong way. One sign,
 * in one place, used by everything here that turns a latitude and a longitude
 * into a direction. The engine reads places by the same sign
 * (`topology::at_lat_lon`), and `crates/worldgen/tests/earth.rs` holds the bake
 * to that reading, and says why a test cannot be what checks the sign.
 */
const EAST = -1;
const SECTOR_SIDE = 65536;
const HALF = SECTOR_SIDE / 2;
const QUARTER_PI = Math.PI / 4;

/** `[n, u, v]` per sector, in the order `crates/topology` fixes. */
const FRAMES: number[][][] = [
	[[1, 0, 0], [0, 1, 0], [0, 0, 1]],
	[[-1, 0, 0], [0, 0, 1], [0, 1, 0]],
	[[0, 1, 0], [0, 0, 1], [1, 0, 0]],
	[[0, -1, 0], [1, 0, 0], [0, 0, 1]],
	[[0, 0, 1], [1, 0, 0], [0, 1, 0]],
	[[0, 0, -1], [0, 1, 0], [1, 0, 0]]
];

const dot = (a: number[], b: number[]) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

/** Cube face coordinate to block coordinate `0..=SECTOR_SIDE`, unwarped. */
const unwarp = (face: number) => (Math.atan(face) / QUARTER_PI) * HALF + HALF;

/** The sector and surface coordinates a direction passes through. */
function fromDirection(d: number[]): [number, number, number] {
	let axis = 0;
	for (let a = 1; a < 3; a++) if (Math.abs(d[a]) > Math.abs(d[axis])) axis = a;
	const sector = axis * 2 + (d[axis] < 0 ? 1 : 0);
	const [n, ua, va] = FRAMES[sector];
	const depth = dot(d, n);
	return [sector, unwarp(dot(d, ua) / depth), unwarp(dot(d, va) / depth)];
}

// --- Fetching --------------------------------------------------------------

const CACHE = `${ROOT}/target/field`;

/** One latitude band of the source, as metres of elevation. */
async function band(index: number): Promise<Float32Array> {
	const file = `${CACHE}/band-${String(index).padStart(2, '0')}.dods`;
	const cached = Bun.file(file);
	if (!(await cached.exists())) {
		const first = index * BAND;
		const query = `?z.z[${first}:${STRIDE}:${first + BAND - 1}][0:${STRIDE}:${SRC_COLS - 1}]`;
		process.stdout.write(`  band ${index + 1}/${SRC_ROWS / BAND}\r`);
		const res = await fetch(DODS + encodeURI(query));
		if (!res.ok) throw new Error(`ETOPO band ${index}: HTTP ${res.status}`);
		await Bun.write(file, await res.arrayBuffer());
	}
	const bytes = new Uint8Array(await Bun.file(file).arrayBuffer());
	const mark = new TextEncoder().encode('Data:\n');
	let at = 0;
	outer: for (; at < bytes.length - mark.length; at++) {
		for (let k = 0; k < mark.length; k++) if (bytes[at + k] !== mark[k]) continue outer;
		break;
	}
	// `Data:` then the array length, twice, then big endian floats.
	const start = at + mark.length + 8;
	const wanted = (BAND / STRIDE) * COLS;
	if (bytes.length - start < wanted * 4) throw new Error(`ETOPO band ${index} is short; delete ${file}`);
	const view = new DataView(bytes.buffer, bytes.byteOffset + start);
	const out = new Float32Array(wanted);
	for (let i = 0; i < wanted; i++) out[i] = view.getFloat32(i * 4, false);
	return out;
}

// --- Baking ----------------------------------------------------------------

/** Six faces, face after face and row after row, as `field.rs` stores them. */
type Level = { side: number; elevation: Int16Array; ruggedness: Uint8Array };

const level = (side: number): Level => ({
	side,
	elevation: new Int16Array(6 * side * side),
	ruggedness: new Uint8Array(6 * side * side)
});

const clamp = (value: number, low: number, high: number) => Math.max(low, Math.min(high, value));

/** One pass over the source: every sample lands in the texel that holds it,
 *  which is the only way to cover a sphere whose texels are not all equal. */
async function scatter(): Promise<Level> {
	const texels = GRAIN * GRAIN;
	const sum = Array.from({ length: 6 }, () => new Float64Array(texels));
	const count = Array.from({ length: 6 }, () => new Uint32Array(texels));
	const low = Array.from({ length: 6 }, () => new Float32Array(texels).fill(Infinity));
	const high = Array.from({ length: 6 }, () => new Float32Array(texels).fill(-Infinity));

	// Longitude is the same for every row: pay for it once.
	const cosLon = new Float64Array(COLS);
	const sinLon = new Float64Array(COLS);
	for (let c = 0; c < COLS; c++) {
		const lon = ((-180 + (c * STRIDE + 0.5) / 60) * Math.PI) / 180;
		cosLon[c] = Math.cos(lon);
		sinLon[c] = EAST * Math.sin(lon);
	}

	for (let b = 0; b < SRC_ROWS / BAND; b++) {
		const rows = await band(b);
		const height = BAND / STRIDE;
		for (let r = 0; r < height; r++) {
			// ETOPO counts rows from the south pole up, whatever its name says.
			const lat = ((-90 + (b * BAND + r * STRIDE + 0.5) / 60) * Math.PI) / 180;
			// Earth's pole is our pole: `material` reads latitude off `d[1]`.
			const [y, ring] = [Math.sin(lat), Math.cos(lat)];
			for (let c = 0; c < COLS; c++) {
				const elevation = rows[r * COLS + c];
				const [sector, u, v] = fromDirection([ring * cosLon[c], y, ring * sinLon[c]]);
				const tx = Math.min(GRAIN - 1, Math.max(0, Math.floor((u / SECTOR_SIDE) * GRAIN)));
				const ty = Math.min(GRAIN - 1, Math.max(0, Math.floor((v / SECTOR_SIDE) * GRAIN)));
				const at = ty * GRAIN + tx;
				sum[sector][at] += elevation;
				count[sector][at]++;
				if (elevation < low[sector][at]) low[sector][at] = elevation;
				if (elevation > high[sector][at]) high[sector][at] = elevation;
			}
		}
		process.stdout.write(`  scattered ${(((b + 1) / (SRC_ROWS / BAND)) * 100).toFixed(0)}%\r`);
	}

	const grain = level(GRAIN);
	for (let sector = 0; sector < 6; sector++) {
		for (let at = 0; at < texels; at++) {
			// A corner texel of a warped face is the smallest there is; none
			// come out under a source sample, but say so if one ever does.
			if (count[sector][at] === 0) throw new Error(`texel ${at} of face ${sector} saw no source`);
			const spread = high[sector][at] - low[sector][at];
			grain.elevation[sector * texels + at] = clamp(Math.round(sum[sector][at] / count[sector][at]), -32768, 32767);
			grain.ruggedness[sector * texels + at] = clamp(Math.round(spread / RUGGED_STEP), 0, 255);
		}
	}
	return grain;
}

/**
 * Half the texels, the same ground: each texel the nearest mean of the four
 * it covers, a half rounded up, both channels. `Field::parse` builds its
 * coarser levels by the same rule, so the stored side is the grain's pyramid
 * with its finest levels dropped, to the bit. Ruggedness averages rather than
 * accumulates: it stays the spread inside one grain texel, so a silhouette
 * cannot change because the camera moved away.
 */
function halve(fine: Level): Level {
	const side = fine.side / 2;
	const coarse = level(side);
	const mean = (plane: Int16Array | Uint8Array, at: number) =>
		(plane[at] + plane[at + 1] + plane[at + fine.side] + plane[at + fine.side + 1] + 2) >> 2;
	for (let sector = 0; sector < 6; sector++) {
		for (let y = 0; y < side; y++) {
			for (let x = 0; x < side; x++) {
				const at = (sector * fine.side + 2 * y) * fine.side + 2 * x;
				const to = (sector * side + y) * side + x;
				coarse.elevation[to] = mean(fine.elevation, at);
				coarse.ruggedness[to] = mean(fine.ruggedness, at);
			}
		}
	}
	return coarse;
}

/** LOCO-I's median edge detector: the median of left, up and the plane
 *  through them and up-left. `field.rs` says why, and undoes it. */
const med = (left: number, up: number, upLeft: number) =>
	clamp(left + up - upLeft, Math.min(left, up), Math.max(left, up));

/** What a field file starts with: `Field::parse` refuses any other. */
const MAGIC = 'PLFIELD2';
/** Magic, id, side: what precedes the planes. Matches `field.rs`. */
const HEADER = 8 + 32 + 4;

/**
 * The file `Field::parse` reads: `PLFIELD2`, the id, the side, then the level
 * in three planes, each face after face and row after row, with no gutter.
 * Elevation goes as its residual from a prediction by the texels before it,
 * zigzagged into a u16 whose high bytes are the first plane and low bytes the
 * second: smooth ground leaves small residuals, so brotli packs the planes.
 * Ruggedness is the third plane, as it is: a prediction packs it 0.1% smaller.
 * The parser builds the coarser levels and lays the gutters, folding each
 * seam in address space with `topology`. The id goes in last: the file's
 * `contentId`, the rule the page checks fetched bytes by.
 */
async function encode(stored: Level): Promise<{ file: Uint8Array; id: string }> {
	const { side, elevation } = stored;
	const plane = 6 * side * side;
	const out = new Uint8Array(HEADER + 3 * plane);
	out.set(new TextEncoder().encode(MAGIC));
	new DataView(out.buffer).setUint32(40, side, true);
	const [high, low] = [out.subarray(HEADER, HEADER + plane), out.subarray(HEADER + plane, HEADER + 2 * plane)];
	for (let sector = 0; sector < 6; sector++) {
		for (let y = 0; y < side; y++) {
			for (let x = 0; x < side; x++) {
				const at = (sector * side + y) * side + x;
				const predicted =
					y === 0 ? (x === 0 ? 0 : elevation[at - 1])
					: x === 0 ? elevation[at - side]
					: med(elevation[at - 1], elevation[at - side], elevation[at - side - 1]);
				// Wrapped to an i16, as the parser's add wraps it back.
				const residual = ((elevation[at] - predicted) << 16) >> 16;
				const zigzag = ((residual << 1) ^ (residual >> 15)) & 0xffff;
				high[at] = zigzag >> 8;
				low[at] = zigzag & 0xff;
			}
		}
	}
	out.set(stored.ruggedness, HEADER + 2 * plane);
	const id = await contentId(out);
	out.set(Buffer.from(id, 'hex'), 8);
	return { file: out, id };
}

/** An equirectangular look at what was baked, so the bake can be seen and not
 *  only believed. */
function preview({ side, elevation }: Level, width: number): Uint8Array {
	const height = width / 2;
	const rgb = new Uint8Array(width * height * 3);
	for (let y = 0; y < height; y++) {
		const lat = ((90 - ((y + 0.5) / height) * 180) * Math.PI) / 180;
		for (let x = 0; x < width; x++) {
			const lon = ((-180 + ((x + 0.5) / width) * 360) * Math.PI) / 180;
			const d = [Math.cos(lat) * Math.cos(lon), Math.sin(lat), EAST * Math.cos(lat) * Math.sin(lon)];
			const [sector, u, v] = fromDirection(d);
			const tx = Math.min(side - 1, Math.floor((u / SECTOR_SIDE) * side));
			const ty = Math.min(side - 1, Math.floor((v / SECTOR_SIDE) * side));
			const e = elevation[(sector * side + ty) * side + tx];
			const at = (y * width + x) * 3;
			if (e < 0) {
				const deep = Math.min(1, -e / 6000);
				[rgb[at], rgb[at + 1], rgb[at + 2]] = [10, 30 + 40 * (1 - deep), 70 + 120 * (1 - deep)];
			} else {
				const up = Math.min(1, e / 4000);
				[rgb[at], rgb[at + 1], rgb[at + 2]] = [60 + 190 * up, 110 + 120 * up, 50 + 150 * up];
			}
		}
	}
	return png(rgb, width, height);
}

/** Adler-32 over the raw scanlines: what closes a zlib stream. */
function adler32(bytes: Uint8Array): number {
	let [a, b] = [1, 0];
	for (const byte of bytes) {
		a = (a + byte) % 65521;
		b = (b + a) % 65521;
	}
	return ((b << 16) | a) >>> 0;
}

/** Minimal PNG: one zlib stream, three chunks. */
function png(rgb: Uint8Array, width: number, height: number): Uint8Array {
	const raw = new Uint8Array((width * 3 + 1) * height);
	for (let y = 0; y < height; y++) {
		raw[y * (width * 3 + 1)] = 0;
		raw.set(rgb.subarray(y * width * 3, (y + 1) * width * 3), y * (width * 3 + 1) + 1);
	}
	const table = Array.from({ length: 256 }, (_, n) => {
		let c = n;
		for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
		return c >>> 0;
	});
	const crc = (bytes: Uint8Array) => {
		let c = 0xffffffff;
		for (const byte of bytes) c = table[(c ^ byte) & 0xff] ^ (c >>> 8);
		return (c ^ 0xffffffff) >>> 0;
	};
	const chunk = (kind: string, body: Uint8Array) => {
		const head = new Uint8Array(4 + body.length);
		head.set(new TextEncoder().encode(kind));
		head.set(body, 4);
		const out = new Uint8Array(8 + body.length + 4);
		new DataView(out.buffer).setUint32(0, body.length, false);
		out.set(head, 4);
		new DataView(out.buffer).setUint32(out.length - 4, crc(head), false);
		return out;
	};
	// `Bun.deflateSync` gives bare deflate; PNG wants it wrapped in zlib.
	const zlib = (body: Uint8Array) => {
		const packed = Bun.deflateSync(body);
		const out = new Uint8Array(2 + packed.length + 4);
		out.set([0x78, 0x01]);
		out.set(packed, 2);
		new DataView(out.buffer).setUint32(out.length - 4, adler32(body), false);
		return out;
	};
	const ihdr = new Uint8Array(13);
	const view = new DataView(ihdr.buffer);
	view.setUint32(0, width, false);
	view.setUint32(4, height, false);
	ihdr.set([8, 2, 0, 0, 0], 8);
	const parts = [
		new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
		chunk('IHDR', ihdr),
		chunk('IDAT', zlib(raw)),
		chunk('IEND', new Uint8Array(0))
	];
	const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
	let at = 0;
	for (const part of parts) {
		out.set(part, at);
		at += part.length;
	}
	return out;
}

// --- Run -------------------------------------------------------------------

/** Where the Earth every version carries lives. A world founded on it names
 *  its id, so only `--replace` writes here. */
const COMMITTED = `${ROOT}/assets/fields`;
const into = process.argv.includes('--replace') ? COMMITTED : `${ROOT}/out/fields`;
await mkdir(CACHE, { recursive: true });
await mkdir(into, { recursive: true });

console.log(`ETOPO 2022, ${COLS} x ${ROWS} samples`);
let stored = await scatter();
while (stored.side > SIDE) stored = halve(stored);
console.log(`\nbaking ${SIDE} texels per face side, from a grain of ${GRAIN}`);

const { file, id } = await encode(stored);
const sidecar = Bun.file(`${COMMITTED}/earth.json`);
const committed: string | null = (await sidecar.exists()) ? (await sidecar.json()).id : null;
const path = `${into}/earth.field`;
await Bun.write(path, file);
// The sidecar is how a page names the ground without downloading it first.
await Bun.write(
	`${into}/earth.json`,
	JSON.stringify({ id, side: SIDE, texel_m: (SECTOR_SIDE * 0.5) / SIDE, source: 'ETOPO 2022 (NOAA NCEI), public domain' }, null, '\t') + '\n'
);
await Bun.write(`${into}/earth.png`, preview(stored, 2048));
console.log(`\n${path}`);
console.log(`  ${(file.length / 1e6).toFixed(1)} MB, ${(SECTOR_SIDE * 0.5) / SIDE} m per texel`);
console.log(`  id ${id}`);
if (into !== COMMITTED) {
	console.log(
		id === committed
			? '  the committed Earth, byte for byte'
			: `  other ground than the committed Earth (${committed}): \`bun run field --replace\` makes it the default, and a world founded on the committed one stops loading`
	);
}
