// A cartridge run with --window in the desktop shell, the keys its buttons:
// a game this version ships, by its name, or a built cart, a .wasm
// (`carts.ts`). The rest of the arguments are the shell's: --seed, --scale.
// A run with no window, a strip of PNGs or its tones as a WAV, is the
// console's runner's (`bun run cart` in TKN-32), on the same file: the
// command means one thing wherever it is run, and here does half of it.
//   bun run cart rocks --window
//   bun run cart path/to/game.wasm --window --scale 6
import { relative } from 'node:path';
import { $ } from 'bun';
import { sha256 } from './built';
import { cartFile } from './carts';
import { ROOT } from './lib';

$.cwd(ROOT);
const [cart, ...rest] = process.argv.slice(2);
const usage = 'bun run cart <rom | file.wasm> --window [--seed N] [--scale N]';
if (!cart) throw new Error(usage);
if (!rest.includes('--window')) {
	console.error(`${usage}: a run with no window is the console's runner's, \`bun run cart\` in TKN-32`);
	process.exit(1);
}
const args = rest.filter((arg) => arg !== '--window');
// One line says why no file came: a ROM whose kept bytes changed, a list
// that does not parse, or a name nothing answers to.
let file: string;
try {
	file = relative(ROOT, cartFile(cart));
} catch (error) {
	console.error(`bun run cart ${cart}: ${(error as Error).message}`);
	process.exit(1);
}
// A cartridge is named by the hash of its content, as every heavy file is.
const bytes = await Bun.file(`${ROOT}/${file}`).bytes();
console.log(`${file}: ${bytes.length} bytes, named ${sha256(bytes)}.wasm`);
process.exit((await $`cargo run --release -q -p shell-desktop -- cart ${file} ${args}`.nothrow()).exitCode);
