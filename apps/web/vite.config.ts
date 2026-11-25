import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

/**
 * The world server this page is served over: the one `bun run dev` started,
 * or else one on the usual port (`scripts/server.ts`).
 */
const WORLD = process.env.WORLD_ORIGIN ?? 'http://127.0.0.1:8090';

export default defineConfig({
	plugins: [
		sveltekit({
			compilerOptions: {
				// Runes mode for every project file; libraries keep their own mode.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			// The page is only files, and the world server sends them. An
			// address that is no route gets `404.html`, which draws the error
			// page.
			adapter: adapter({ fallback: '404.html' })
		})
	],
	// The cartridges' worker loads its engine as a module of its own.
	worker: { format: 'es' },
	server: {
		// One origin in development too: Vite holds the page and hands the
		// world's routes, its socket and its files to the Go server.
		proxy: { '/api': { target: WORLD, ws: true }, '/files': { target: WORLD } },
		// A plugin's panel lives in its own folder, outside this app.
		fs: { allow: ['../../plugins'] }
	}
});
