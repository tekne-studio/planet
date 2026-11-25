// Builds the browser client: shell-web -> WASM -> apps/web/src/lib/engine/pkg,
// and in its `emulator/` the parts of the emulator the engine pins
// (`emulator.ts`): the runner and its host the worker that runs cartridges
// loads, and the chip with the processor a cabinet's game sounds through.
// And ships the ROMs this version ships into the asset set, with any extra
// built carts after them (`roms.ts`). Run beside a `bun run dev
// --cart`, it offers that run's extra carts again, read afresh from their
// paths: a rebuild never drops what the running world offers.
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { $ } from 'bun';
import { copyEmulator } from './emulator';
import { shipRoms } from './roms';
import { ROOT } from './lib';

const OUT = `${ROOT}/apps/web/src/lib/engine/pkg`;

/** The extra carts of this checkout's running dev, `{ pid, carts }`, while it runs. */
const HELD = `${ROOT}/target/dev-carts.json`;

/** Holds `carts`, by path, as the extra carts of this process's run; none lets them go. */
export function holdCarts(carts: string[]) {
	if (carts.length === 0) return releaseCarts();
	mkdirSync(dirname(HELD), { recursive: true });
	writeFileSync(HELD, JSON.stringify({ pid: process.pid, carts: carts.map((cart) => resolve(cart)) }));
}

/** Lets the run's extra carts go, as it ends. */
export const releaseCarts = () => rmSync(HELD, { force: true });

/** The extra carts a dev still running holds; none when no run holds any or its process is gone. */
function heldCarts(): string[] {
	let held: { pid: number; carts: string[] };
	try {
		held = JSON.parse(readFileSync(HELD, 'utf8'));
	} catch {
		return [];
	}
	try {
		process.kill(held.pid, 0);
	} catch {
		releaseCarts();
		return [];
	}
	return held.carts;
}

export async function buildWasm(extra: string[] = []): Promise<void> {
	if (!Bun.which('wasm-bindgen')) throw new Error('wasm-bindgen not found: run `bun run setup`');
	$.cwd(ROOT);
	await $`cargo build --release --target wasm32-unknown-unknown -p shell-web`;
	await $`wasm-bindgen --target web --out-dir ${OUT} target/wasm32-unknown-unknown/release/shell_web.wasm`;
	copyEmulator(`${OUT}/emulator`);
	const titles = await shipRoms(extra);
	console.log(`engine and emulator -> ${OUT}; ${titles.join(', ')} -> assets/carts`);
}

if (import.meta.main) {
	const held = heldCarts();
	if (held.length) console.log(`the running dev's extra carts, offered again: ${held.join(', ')}`);
	await buildWasm(held);
}
