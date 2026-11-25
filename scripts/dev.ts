// Everything at once: the world server and the web front end. Ctrl+C stops both.
// The engine is rebuilt first, every time: cargo is incremental, and a stale
// WASM package silently shows an older world than the desktop does.
// `--cart <file.wasm>`, again for more, offers built carts after the ROMs
// this version ships, for this run: one named as a ROM takes its place, so
// a game is heard in a cabinet before its ROM is committed. A
// `bun run wasm` beside the run offers them again (`wasm.ts`).
//   bun run dev --cart ../mygame/target/wasm32-unknown-unknown/release/mygame.wasm
import { HOST, ROOT, freePort } from './lib';
import { build, serve } from './server';
import { buildWasm, holdCarts, releaseCarts } from './wasm';

const args = process.argv.slice(2);
const extra = args.flatMap((arg, at) => (args[at - 1] === '--cart' ? [arg] : []));
if (args.length !== extra.length * 2 || args.some((arg, at) => at % 2 === 0 && arg !== '--cart')) {
	throw new Error('bun run dev [--cart <file.wasm>]...');
}
await buildWasm(extra);
holdCarts(extra);

await build();
// Several checkouts run at once, each with its own world: the server takes
// the first free port from 8190, and Vite does the same for the page on its
// own. 8190, not the server's own 8090: mundos's PocketBase holds 8090 on a
// machine that runs both.
const port = freePort(Number(process.env.PORT ?? 8190));
const server = serve(port);

// Vite holds the page and hands the world's routes and socket to the server,
// so the browser sees one origin, as it does under mundos.
const web = Bun.spawn(['bun', 'run', 'dev'], {
	cwd: `${ROOT}/apps/web`,
	env: { ...process.env, WORLD_ORIGIN: `http://${HOST}:${port}` },
	stdout: 'inherit',
	stderr: 'inherit'
});

const stop = () => {
	releaseCarts();
	server.kill();
	web.kill();
	process.exit(0);
};
process.on('SIGINT', stop);
process.on('SIGTERM', stop);

// Either one going takes the other down.
await Promise.race([server.exited, web.exited]);
stop();
