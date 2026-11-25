// Builds the server's module: crates/module -> WASM -> the file the Go server
// embeds, and copies beside it the pinned emulator's runner, which the
// server judges a cartridge by. The Go server builds and tests only after
// this.
//   bun run module
import { copyFileSync } from 'node:fs';
import { $ } from 'bun';
import { copyJudge } from './emulator';
import { ROOT } from './lib';

const WASM = `${ROOT}/target/wasm32-unknown-unknown/release/module.wasm`;
const OUT = `${ROOT}/server/internal/module/world.wasm`;
const JUDGE = `${ROOT}/server/internal/arcade/emulator`;

export async function buildModule(): Promise<void> {
	$.cwd(ROOT);
	await $`cargo build --release --target wasm32-unknown-unknown -p module`;
	copyFileSync(WASM, OUT);
	console.log(`module -> ${OUT}`);
	copyJudge(JUDGE);
	console.log(`the emulator's runner -> ${JUDGE}`);
}

if (import.meta.main) await buildModule();
