// The games this version ships, as its asset set lists them
// (`assets/carts/games.json`, written by `bun run wasm` from the built
// files in games/): each by the name a command gives it, the title a
// person reads, and its file, which every world holds without an upload.
// Read again each time it is asked for: a game shipped while the page is
// open is offered, and HTTP caching keeps the read cheap.

/** A game this version ships. */
export interface Game {
	name: string;
	title: string;
	/** `asset://<sha256>.wasm`. */
	cart: string;
}

const isGame = (game: unknown): game is { name: string; title: string; file: string } =>
	typeof game === 'object' &&
	game !== null &&
	['name', 'title', 'file'].every((key) => typeof (game as Record<string, unknown>)[key] === 'string');

/** The games, in the order offered; none when the set lists none or cannot be read. */
export function games(): Promise<Game[]> {
	return fetch('/assets/carts/games.json')
		.then((response) => (response.ok ? response.json() : []))
		.then((listed: unknown) =>
			Array.isArray(listed) ? listed.filter(isGame).map(({ name, title, file }) => ({ name, title, cart: `asset://${file}` })) : []
		)
		.catch(() => []);
}
