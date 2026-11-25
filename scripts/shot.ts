// Headless render to PNG with a fixed clock and seed. Arguments pass through:
//   bun run shot --out out/ground.png --seed 00000000deadbeef
//   bun run shot --out out/orbit.png --altitude 30000 --pitch -60 --boom 50
//   bun run shot --cabinet rocks --play --out out/played.png
// `--cabinet` takes a cart as `bun run cart` does (`carts.ts`): named by
// its hash and served as a world's file is (PLANET_FILES).
import { copyFileSync, mkdirSync } from 'node:fs';
import { $ } from 'bun';
import { sha256 } from './built';
import { cartFile } from './carts';
import { ROOT } from './lib';

$.cwd(ROOT);
const args = process.argv.slice(2);
if (!args.includes('--out')) args.push('--out', 'out/shot.png');
const env = { ...process.env };
const at = args.indexOf('--cabinet');
if (at >= 0 && args[at + 1]) {
	const file = cartFile(args[at + 1]);
	const hash = sha256(await Bun.file(file).bytes());
	const files = `${ROOT}/out/files`;
	mkdirSync(files, { recursive: true });
	copyFileSync(file, `${files}/${hash}.wasm`);
	args[at + 1] = hash;
	env.PLANET_FILES = files;
}
await $`cargo run --release -q -p shell-desktop -- shot ${args}`.env(env);
